# Texture2D residual v3 — standalone-streaming size gate lift

> **Status: landed.** Single one-line change in
> `src/builder.rs::StandaloneTextureBuilder::build` drops the artificial
> `target_w == 512 && target_h == 512` restriction on standalone BC7
> `.resS` streaming. Closes 77.3 % of the Texture2D bits-diff on the
> 22-entity / 2174-bundle windows corpus.

## TL;DR

| metric | pre (v10 windows) | post | Δ |
|---|---:|---:|---:|
| Texture2D bits-different (windows) | 600,522,410 | **135,514,929** | **−464,007,481 (−77.3 %)** |
| Texture2D ppm-bits (windows) | 187,924.1 | **49,626.2** | −138,298 ppm |
| `m_IsReadable` mismatches | 50 | **1** | −49 |
| `m_StreamData.path` mismatches | 134 | 90 | −44 |
| `m_StreamData.size` mismatches | 72 | 23 | −49 |
| Texture2D byte-identical (windows) | 3,481 / 4,656 | 3,569+ / 4,656 | +88+ |
| Texture2D bits-different (mac) | 11,681,508 | 11,681,508 | 0 (no regression) |
| `cargo test --release --lib` | 116 / 116 | **116 / 116** | OK |
| `cargo test --release --test parity_bytes` | 773,032 ≤ 773,032 | 773,032 ≤ 773,032 | unchanged ceiling |

## Root cause

`StandaloneTextureBuilder::build` (`src/builder.rs:1977-1980`) gated `.resS`
streaming of standalone BC7 textures behind a **size predicate** in addition
to the `model_referenced` flag:

```rust
let do_stream = self.model_referenced
    && prof.texture_format == 25
    && prof.target_w == 512
    && prof.target_h == 512;     // ← linux artifact
```

Linux capped `max_texture_size_for("linux") = 512`, so for the original
linux-target corpus every standalone BC7 was either 512×512 (streamable) or
<256 (sub-block fallback). Windows + mac use 1024, so prod produces 256×256,
512×512, 512×256, **and 1024×1024** standalone BC7s — but ours streamed only
the 512×512 subset, inlining the rest with `m_IsReadable=true` and an empty
`m_StreamData`.

## Evidence

`dev/measure_bits_texture2d_windows.py` on a clean build (commit `438a255`):

```
total tex bits compared : 3,195,558,040
total tex bits differ : 600,522,410
TEX PPM-BITS DIFFER : 187924.1

differing field paths (top 5):
 832 image data
 238 m_StreamData.offset
 134 m_StreamData.path
    72  m_StreamData.size
    50  m_IsReadable
```

The top 15 Texture2D residuals were all **~11.2 M bits each** with the
identical 4-field signature (`m_StreamData.path` empty vs `archive:/…`,
`m_StreamData.size` 0 vs 1,398,128, `image data` 1,398,128 bytes vs 0,
`m_IsReadable` True vs False) — the signature of a 1024×1024 BC7 that prod
streamed but ours inlined.

`/tmp/dump_container.py` confirmed all top-3 are **standalone** PNG/JPEG
bundles (2-entry `m_Container`, no glb), not in-glb textures. The prior
"in-glb" hypothesis was wrong — the actual residual driver was standalone
size > 512.

### Partition validation — `prod_streams ⇔ glb_references_uri`

`/tmp/standalone_partition.py` walked all 2174 windows bundles, partitioned
the 930 standalone BC7 textures by `(prod_stream, ref_by_glb_in_entity)`:

```
prod stream + ref-by-glb : 137 ← all streamed are referenced
prod stream + NOT ref-by-glb : 0
prod inline + ref-by-glb : 0 ← no referenced inline
prod inline + NOT ref-by-glb : 793
```

**100 % clean partition.** The `model_referenced` flag is already the
correct streaming gate (1,176 measure_script-passes set it on the same
137 CIDs); the size predicate was redundant and wrong.

Streamed-size distribution (ref'd subset):

| (w, h) | count | per-tex bits-diff (pre-fix) |
|---|---:|---:|
| 1024×1024 | 41 | ~11.2 M |
| 512×512 | 88 | 0 (already streamed) |
| 256×256 | 7 | ~700 K |
| 512×256 | 1 | ~1.4 M |

The fix recovers the 41 + 7 + 1 = 49 large mismatches × ~3-11 M bits =
~465 M bits, matching the observed delta.

## Implemented change

`src/builder.rs::StandaloneTextureBuilder::build` (single statement):

```rust
// Stream the BC7 standalone texture into.resS when a sibling glb in
// the entity references this image URI. Prod's `ImportTextures` path
// streams every cross-bundle-referenced BC7 standalone regardless of
// resolved size (256x256.. 1024x1024 all observed on the windows
// corpus: 137/137 ref'd BC7 streamed, 0 inline; 0/793 unref'd
// streamed). The earlier 512x512 gate was a linux-only artifact
// from when ABGEN_TARGET=linux capped max_texture_size at 512.
let do_stream = self.model_referenced && prof.texture_format == 25;
```

No other code touched. The `model_referenced` flag is already plumbed
through `ab-generate` Phase 2 from the glb-bundle URI collection pass,
and `ab-build-local --model-referenced` is the existing CLI surface.

## What's left

Post-landing top-15 windows residuals are now dominated by:

1. **Per-block BC7 encoder tie-break** (`image data`-only diffs, the
 ~2-4 M-bit residuals on `bafybeibzo7npkks`, `bafybeiasfwfzyf4`,
 `bafybeidtd4f7ixh`). Same bc7e-ISPC vs pure-Rust refinement-pass
 ordering issue documented in `texture2d_followup.md` §3 — bound by
 the existing analysis.

2. **NPOT classification mismatch** (`m_MipCount` + `m_Width/Height` +
 `m_CompleteImageSize` + `image data` all diff together — the
 `bafkreicfprgk7s6`-style residuals). 17 cases with `m_MipCount`
 differing, 13 each `m_Width/m_Height` differing. These are textures
 where our `bc7_target_size` snap picks a different POT than prod
 does; needs separate investigation.

3. **`m_ColorSpace=0` + `m_LightmapFormat=3` standalone variants**
 (32 cs=0 + 15 lm=3 cases in the corpus). Our
 `standalone_texture_profile` hardcodes `cs=1, lm=0`. For 47 cases
 this is wrong but the bits-diff per case is small (typetree scalar,
 not image data).

4. **mac unchanged** at 11.68 M bits / 9120 ppm — mac corpus is only 2
 entities, neither of which exercises the large-standalone codepath.

## Reproduce

```bash
cd <repo>
<fhs-shell> -c "cargo build --release --bin ab-build-local"
cd <repo-root>
ABGEN_AB_BIN=<wt>/.../target/release/ab-build-local \
 nix-shell --run "python -u <wt>/.../dev/measure_bits_texture2d_windows.py"
```
