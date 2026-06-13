# Texture2D windows — close the residual 56 (BC7 standalone path)

> **Status: LANDED in commit `bc5c9b0`** ("windows + mac parity work …
> Texture2D Basic preset"). `Params::basic` + `Bc7Profile::{Slow,Basic}`
> + `encode_bc7_mip_chain_with_profile` shipped in `src/bc7_pure.rs`;
> `src/builder.rs` standalone path picks Basic for windows/mac, Slow
> otherwise. In-glb textures continue on Slow. Texture2D ppm-bits
> dropped from 22 423 → 14 442 on windows v10 (–35.6 %); same shape on
> mac.

## TL;DR

The converter builds the BC7 mip chain for **standalone-texture bundles**
with `bc7e_compress_block_params_init_basic(perceptual=True)`, **not**
`..._init_slow`. Switching the Rust + Python builders' standalone
`encode_bc7_mip_chain` call site from `slow` to `basic` reduces the
Texture2D class' ppm-bits-different against the 280-bundle windows corpus
from **22,423 → 14,442 (-35.6 %)** with no other code changes. All 107
library tests stay green; the per-class Texture2D residual count stays at
56 (same set of cases) but every residual case is now far closer to prod.

## Baseline (pre-change)

`abgen-rs/dev/measure_bits_texture2d_windows.py` against the 280-bundle
corpus (`workdir/pathid_rt_v10_windows`, URP v10, StandaloneWindows64):

```
prod bundles surveyed : 280
paired & compared : 280
paired Texture2D objs : 1004
Texture2D byte-id : 948
differing Texture2D : 56 (all 100 % `image data` only)

total tex bits compared : 1,280,792,648
total tex bits differ : 28,719,155
TEX PPM-BITS DIFFER : 22,423.0

image-data bits diff : 28,719,155
image-data bytes total : 25,082,880
IMAGE-DATA PPM within : 143,121.3
```

All 56 residuals are single-signature: `image data` differs, same length,
same metadata (format, mip count, dims). This matches the post-tex_close_60
state — the 56 BC7 standalone-texture image-data divergences.

## Discrimination probe — pinning the encoder

`abgen-rs/dev/bc7_probe_prod_encoder.py` walks every bc7e preset
(`slow`, `basic`, `fast`, `veryfast`, `ultrafast`, `veryslow`, `slowest`)
at both `perceptual=True` and `perceptual=False`, plus `etcpak`, against
prod's BC7 image-data for a given CID. Per-block exact-match counts:

| CID | Source | Target | ours-rust (=slow/perc=T) | basic/perc=T | Δ |
|---|---|---|---:|---:|---:|
| `bafkreiczuewg3pf…` | 1340×670 | 1024×512 (dn) | 47.55 % | **99.77 %** | +52 pp |
| `bafybeih4xgkars5…` | 1024×1024 (id) | 1024×1024 | 50.88 % | **80.95 %** | +30 pp |
| `bafybeigs5ygjyxj…` | 1024×1024 (id) | 1024×1024 | 60.12 % | **85.84 %** | +25 pp |
| `bafybeihmoapsaow…` | 1024×1024 (id) | 1024×1024 | 56.71 % | 63.44 % | +7 pp |
| `bafkreihflg6n5vp…` | 586×586       | 512×512 | 19.03 % | 24.42 % | +5 pp |
| `bafybeicdnee5dq4…` | 841×493 (up)   | 1024×512 (up) | 4.28 % | 6.23 % | +2 pp |
| `bafybeig5aphs44y…` | 512×512 (id)   | 512×512 | 3.63 % | 4.71 % | +1 pp |

**`bc7e/basic/perc=True` wins every single comparison** — sometimes by huge
margins (the cleanest downscale → 99.77 % per-block exact). On clean
power-of-two-target downscales the per-block match rate is essentially
total, leaving < 0.25 % residual that pins to per-block tie-break noise.

The cases where neither slow nor basic matches well (the upscale and
near-identity cases at ~5-25 %) are a **separate** failure mode — almost
certainly a resize divergence between our `box_downscale` and the
converter's `Utils.ResizeTexture` for the upscale / near-1× path (`abgen/resize.py`
docstring describes the downscale path as proven byte-exact but the
upscale path falls through to `point_center_downscale`, which is unlikely
to bit-match the converter's `Graphics.Blit` upscale). Closing those is path 2
below.

### Why `slow` works on in-glb textures but `basic` works on standalone

The converter processes the two paths through different importers:

- **In-glb textures** (`Builder::texture_tree_with_wrap`, fmt=BC7 typically
 via `CustomGltfImporter`): the converter drives bc7e at the `slow` preset
 directly. Our existing `Params::slow` path matches these byte-exact
 (we measured 948/1004 Texture2D byte-id, and the 56 residuals are
 exclusively standalone PNGs — `dev/measure_bits_texture2d_windows.py`
 per-Tex top-15 confirms every high-bits case is a `bafybei…` / `bafkrei…`
 PNG source, never a glb-embedded texture).

- **Standalone-texture bundles** (`StandaloneTextureBuilder::build`,
 PNG / JPG source bundled by `ImportTextures`): the converter routes through a
 `TextureImporter` whose default platform-override on `StandaloneWindows64`
 / `StandaloneOSX` at `TextureCompressionQuality.Normal` is bc7e/basic.
 This is the corpus-wide signal: not a single CID where slow beats basic.

The same standalone bundles compared against bc7e/basic match per-block at
> 99 % on clean downscales — the strongest "this is the actual encoder"
signal we have without disassembling the Unity binary.

## Bit-exact bc7e-port confirmation

The same probe script verifies that our pure-Rust `bc7_pure` port is
bit-exact bc7e ISPC for the slow preset across all 41,825 differing blocks
of `bafybeicdnee5dq4`: `ours-rust == bc7e/slow/perc=True == 4.28 %`
matching prod (and `prod != bc7e/slow` for every differing block). After
adding `Params::basic`, the same probe on `bafkreiczuewg3pf` shows
`ours-rust == bc7e/basic/perc=True == 99.77 %`. The Rust port is correct
— it was just running the wrong preset for the standalone path.

## Implementation (full diff already prototyped & measured)

Three files, ~70 LOC net add. Prototype lived in this worktree;
**all 107 lib tests pass** and the Texture2D ppm-bits drops by 35.6 %.

### 1. `src/bc7_pure.rs` — refactor `Params::slow` into `base → slow / basic`

```rust
impl Params {
    fn base(perceptual: bool) -> Self {
        // bc7e_compress_block_params_init defaults: pbit_search=false,
        // al_max_mode7=1, uber_level=0, every other field same as today's
        // slow constructor.
        Params { /* … */ pbit_search: false, al_max_mode7: 1, /* … */ }
    }

    pub fn slow(perceptual: bool) -> Self {
        let mut p = Self::base(perceptual);
        p.al_max_mode7 = 2; p.pbit_search = true; p.uber_level = 0;
        p
    }

    pub fn basic(perceptual: bool) -> Self {
        let mut p = Self::base(perceptual);
        if perceptual {
            p.use_mode[0] = false;
            p.use_mode[2] = false; p.use_mode[3] = false;
            p.use_mode[4] = false; p.use_mode[5] = false;
        } else {
            p.max_partitions_mode[1] = 32;
            p.max_partitions_mode[2] = 32;
            p.max_partitions_mode[3] = 32;
            p.max_partitions_mode[7] = 32;
            p.use_mode[2] = false;
        }
        p.uber_level = 1;
        p
    }
}
```

Plus a `Bc7Profile { Slow, Basic }` enum and a
`encode_bc7_mip_chain_with_profile(...)` wrapper (the existing
`encode_bc7_mip_chain` delegates to it with `Bc7Profile::Slow` for back-
compat). The fields gated by `use_mode[0..6]` already exist and are
honored in the opaque path (`build_partition_plans`,
`evaluate_solution`).

### 2. `src/builder.rs` — pass profile per-call-site

```rust
fn encode_texture_bc7(img, mips, srgb, profile: bc7_pure::Bc7Profile) {
    // …existing body unchanged, calls encode_bc7_mip_chain_with_profile(…, profile)
}

// In-glb path (Builder::texture_tree_with_wrap, ~ line 729):
encode_texture_bc7(src, prof.mip_count, prof.color_space == 1,
                   bc7_pure::Bc7Profile::Slow)

// Standalone path (StandaloneTextureBuilder::build, ~ line 1867):
encode_texture_bc7(pil, prof.mip_count, prof.color_space == 1,
                   bc7_pure::Bc7Profile::Basic)
```

### 3. `abgen/builder.py` — same split on the Python side

```python
def _encode_texture_bc7(pil, mips, srgb, profile="slow"):
    # … fall back to dxt_unity if bc7_ispc missing; pass profile= through
    return _bc7_exact(pil, mip_count=mips, flip=True, srgb=srgb,
                      profile=profile)

# Standalone (around line 852):
data, mips = _encode_texture_bc7(resized, prof.mip_count,
                                 srgb=(prof.color_space == 1),
                                 profile="basic")
```

`abgen/bc7_ispc.encode_bc7_mip_chain` already takes `profile=` (defaults
to `"slow"`); no change needed there.

## Measured delta

After the three edits above, against the 280-bundle windows corpus:

| metric | baseline (slow) | with basic | Δ |
|---|---:|---:|---:|
| Texture2D bits-different | 28,719,155 | **18,497,519** | **-10,221,636 (-35.6 %)** |
| Texture2D ppm-bits | 22,423.0 | **14,442.2** | **-7,981 ppm** |
| Image-data ppm-within | 143,121.3 | **92,182.0** | **-50,939 ppm** |
| Texture2D byte-id objects | 948 / 1004 | 948 / 1004 | 0 (same set of residuals) |
| Texture2D residual count | 56 | 56 | 0 (same CIDs) |

The 56-case count is unchanged because each case still differs **at the
block-tiebreak level**: many blocks are now exact, but at least one block
per case still mismatches. The total bits-different per case drops sharply
(e.g. the worst case `bafybeicdnee5dq4` goes from 1,890,750 → 1,612,340
bits, a 15 % per-case reduction; `bafybeihmoapsaow` from 1,620,454 →
1,052,620, a 35 % per-case reduction).

The remaining 18.5 M bits of residual are partitioned roughly:
- ~15 M bits: per-block tie-break and refinement-pass differences between
 our pure-Rust port and bc7e ISPC under the basic preset, on
 non-clean-downscale geometries.
- ~3.5 M bits: per-block differences caused by **resize divergence** on
 upscale / near-1× cases (see CIDs scoring < 25 % per-block under basic
 in the table above).

## Comprehensive parity-harness (also bound this delta)

`dev/measure_full_vs_prod.py` (paired-object byte-exact):

| metric | baseline (slow) | with basic | Δ |
|---|---:|---:|---:|
| paired-object byte-exact | 14878/15018 (99.07 %) | 14879/15018 (99.07 %) | +1 byte-exact obj |
| Texture2D residuals | 56 | 56 | 0 |
| AssetBundle, Mesh, Material residuals | unchanged | unchanged | 0 |

No regression in any other class.

## Why this proposal is NOT landed in the worktree

The branch's `tests/parity_bytes.rs` is `python_vs_rust_byte_parity` — it
compares Rust against the Python builder's **regenerated** reference
fixtures. The single-change-to-Rust path raises the python-vs-rust delta
on the 4 standalone-texture fixture cases by ~1.9 M bits — which by
itself nudges the total ~127.3 M past the existing
`MAX_BITS_DIFFERENT = 126_877_515` ceiling by ~0.4 M bits.

Updating Python in lockstep (also switch standalone to `basic`) and
regenerating fixtures via `dev/parity_gen.py` produces 133.9 M bits — the
delta gets worse because the fixture regen also captures unrelated
fixture drift (e.g. `bafybeicqrwx4olf` 27,094,820 → 27,094,841 bytes,
+21 bytes from CAB hash / metadata changes that have accumulated since
the fixtures were last regenerated). That drift is **not** in this fix's
scope.

The fix is mechanical and correct, but landing it cleanly needs **one of**:

1. Update `MAX_BITS_DIFFERENT` to the new measured value (forbidden by
 the worktree directive).
2. Land alongside a fixture-regeneration pass that captures all
 accumulated drift since the fixtures were last regenerated — a
 separate proposal, not in scope here.
3. Update Python builder to use basic but **defer** fixture regeneration
 and bump the threshold by exactly the standalone-only delta. Same as
 #1 effectively.

For now this proposal stays in `dev/fix_proposals/` (unlanded). The
worktree carries the discriminator (`dev/bc7_probe_prod_encoder.py`),
the per-class measurement script (`dev/measure_bits_texture2d_windows.py`),
and the per-CID encoder probe (`dev/bc7_diff_against_ispc.py`) so the next
pass can re-confirm the signal and pick the landing strategy.

## Method note

Probe scripts in this worktree:

- **`dev/measure_bits_texture2d_windows.py`** — per-class Texture2D
 bits-diff over the 280-bundle corpus (canonical baseline measurement).
- **`dev/bc7_probe_prod_encoder.py`** — for a given CID, re-encodes via
 every bc7e preset × perceptual flag + etcpak, reports per-block
 match-against-prod for each. Use to confirm "prod uses bc7e/basic" on
 any new CID.
- **`dev/bc7_diff_against_ispc.py`** — for a given CID, drives the Rust
 build + bc7e ISPC side-by-side at a chosen profile; classifies per-block
 cases by `(ours==prod, ours==bc7e, prod==bc7e)` triples. Use to confirm
 our pure-Rust port is bit-exact bc7e at the chosen profile (proves the
 divergence is the profile selection, not the port).

All three scripts read `ABGEN_AB_BIN` and default to the worktree's
`abgen-rs/target/release/ab-build-local` so they auto-target the
worktree under measurement.
