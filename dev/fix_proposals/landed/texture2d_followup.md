# Texture2D follow-up — post-Basic landing residuals (windows + mac)

> **Status: landed.** Bilinear upscale resize (commit
> `fc5e883`, −8.5% Texture2D class) and alpha-bleed dilation pass
> (commit `b3bea9c`, −30.9% Texture2D class; rooted in the
> re-diagnosis in `landed/bc7_tiebreak_v2.md`) both landed.

## TL;DR

After the Basic-preset switch landed (`tex_close_60` → `texture2d_windows.md`),
56 Texture2D residuals remain on both the windows and mac 280-bundle corpora.
All 56 are single-signature `image data` differences on **standalone PNG**
textures (zero in-glb residuals). The work below classifies them and lands one
mechanical fix:

**Switch upscale resize from nearest-neighbor to float-bilinear**
(`src/resize.rs::bilinear_upscale`). Closes 8.5% of remaining bits-diff
on both targets, no regression in any other class, parity_bytes ceiling
unchanged.

| metric | post-Basic (baseline) | post-bilinear-upscale | Δ |
|---|---:|---:|---:|
| Texture2D bits-different (windows) | 18,497,519 | **16,920,515** | **−1,577,004 (−8.5 %)** |
| Texture2D bits-different (mac) | 18,497,519 | **16,920,515** | **−1,577,004 (−8.5 %)** |
| Texture2D ppm-bits | 14,442.2 | **13,211.0** | −1,231 ppm |
| Image-data ppm-within | 92,182.0 | **84,323.0** | −7,859 ppm |
| Texture2D byte-id objects | 948 / 1004 | 948 / 1004 | 0 (same set) |
| Texture2D residual count | 56 | 56 | 0 (same CIDs) |
| `cargo test --release --lib` | 107 / 107 | **109 / 109** (+2 new) | OK |
| `cargo test --release --test parity_bytes` | 1,978,445 / 1,978,445 | 1,978,445 / 1,978,445 | unchanged (no upscale fixtures) |

## Subclass breakdown (n=56 per platform, identical between windows + mac)

`dev/classify_texture2d_residuals.py` enriches each residual with source
dims (read from the PNG header), target dims, format, color space, mip
count, source-kind (png-standalone / glb-embedded / external-glb / ktx2),
POT-ness on both ends, and resize direction (identity / upscale / downscale
/ mixed). Pre-landing aggregates (one platform shown; mac matches windows
exactly):

```
--- by source ---
 png-standalone n=56 bits=18,497,519 ppm-img= 92,182.0

--- by scale ---
 upscale n=14 bits= 6,726,306 ppm-img=149,167.3 <-- worst ppm
 identity n=22 bits= 5,957,045 ppm-img= 61,524.8
 downscale n=18 bits= 5,289,563 ppm-img= 94,577.1
 mixed n= 2 bits= 524,605 ppm-img=186,125.2

--- by color space ---
 csp=1 (sRGB) n=56 bits=18,497,519
 csp=0 (linear) n= 0
--- by m_LightmapFormat ---
 (all 0 — no normal-maps in the standalone path)
```

Every residual is sRGB, BC7 (fmt=25), `png-standalone`. No KTX2, no
glb-embedded, no normal-maps. The two open question marks
(in-glb path preset choice; KTX2-embedded) are settled negatively: there
are no residuals on those paths.

## Highest-leverage subclass — upscale

Upscale cases (sw < dw on either axis) carry **36.4 % of remaining bits**
across 14 of 56 cases, at the **worst per-byte ratio** (149,167 ppm-img,
vs 94,577 for downscale and 61,525 for identity). The mixed cases (2 of 56)
are tiny in absolute terms (525k bits) but the worst per-byte ratio of all
subclasses (186,125 ppm-img), confirming the same root cause.

Source dim distribution within the upscale subclass:

| (sw, sh) → (dw, dh) | count | bits-diff |
|---|---:|---:|
| 500×500 → 512×512 | 7 | 2,386,820 |
| 503×503 → 512×512 | 1 |   888,854 |
| 1000×1000 → 1024×1024 | 1 | 1,052,620 |
| 841×493 → 1024×512 | 1 | 1,612,340 |
| 400×400 → 512×512 | 1 |   453,596 |
| 202×62 → 256×64 | 2 |    41,637 |
| 740×486 → 512×512 (mixed) | 1 |   519,065 |
| 34×54 → 32×64 (mixed) | 1 |     5,540 |

All of these are PNG → NPOT-rounded-up BC7 target, where `texprofile::npot`
snaps source up to the next POT (e.g. 500 → 512, 1000 → 1024). Prod
processes them by calling `Utils.ResizeTexture` which invokes
`Graphics.Blit` with `FilterMode.Bilinear` for the upscale step (vs the
`FilterMode.Point` mip-pick used on downscale, documented in
`abgen/resize.py` module docstring).

## Root cause hypothesis confirmed

`dev/upscale_sweep.py` (new) reproduces the post-resize bytes via several
candidate resamplers, encodes mip0 via bc7e/basic/perc=True, and counts
bits-diff vs prod. Pre-landing `box_downscale_rgba`'s upscale path falls
through to `point_center_downscale` (nearest-neighbor with half-pixel
center) — the same model bc7e was already seeing. Across all 16 upscale +
mixed CIDs (sum-of-cases, one platform):

| resampler | mip0 bits-diff (sum) | Δ vs NN |
|---|---:|---:|
| `nn_half_center` (status quo) | 5,486,347 |   0     |
| `pil_bilinear` | 5,334,395 |  −2.77 % |
| `bilin_float` (half-pixel-center 4-tap) | **4,331,741** | **−21.05 %** |

Every single one of the 16 cases improves under `bilin_float`; none
regresses. The `bilin_float` model is a deterministic float-bilinear with
half-pixel-centered taps and round-to-nearest output — closer to what a
`Graphics.Blit FilterMode.Bilinear` actually computes than the
`mesa_8bit_bilin` model used for the *downscale*-NPOT mip step
(`_bilinear_halve_floor` in `abgen/resize.py`).

`mesa_8bit_bilin` was tried as a candidate (it's the model proven byte-
exact for the *downscale* NPOT step). It *hurts* on upscale by an average
−13.4 % vs status quo. The downscale model uses a low-precision Mesa
lerp tied to a `FilterMode.Point` source, which is wrong for the upscale
path's `FilterMode.Bilinear` source.

## Implemented change (in this commit)

`src/resize.rs`:

1. New `bilinear_upscale` — float-bilinear, half-pixel-centered taps,
 round-to-nearest, edge-clamp. ~24 LOC, no dependencies.
2. `box_downscale_rgba` upscale branch routed through `bilinear_upscale`
 instead of `point_center_downscale`. The `ratio > 1.0` (downscale)
 branch now also dispatches to `bilinear_upscale` when the *final* step
 from the bilinear-halve loop is an upscale on either axis (handles the
 2 mixed cases like 740×486 → 512×512 where w is downscale but h is
 upscale; the `_bilinear_halve_floor` chain reduces 740 to 370 then
 stops, leaving cur=(370, 486) which must be resampled to (512, 512) —
 an upscale on w).
3. Two new unit tests: `bilinear_upscale_smoke` (2×2 → 4×4 produces
 interpolated values, not source-pixel repeats); `bilinear_upscale_mixed_axes`
 (4×2 → 2×4 doesn't panic).

No other files touched. The existing `box_downscale_matches_golden_vectors`
test passes because every fixture case lives entirely inside the
downscale branch — the upscale codepath was previously untested.

## Why this matters

Mip0 dominates the BC7 mip-chain bits-diff because:

1. Mip0 is 4× the size of mip1, 16× mip2, etc.
2. Mips 1..N are box-halved off mip0 — better mip0 → better mips downstream.

So a 21% mip0 reduction across the upscale subclass translates to an
~23% reduction in total upscale bits-diff (matches the observed
1,612,340 → 1,237,912 = −23% for `bafybeicdnee5dq4`, 1,052,620 → 697,448
= −34% for `bafybeihmoapsaow`).

## What's left

After this landing, 16.92M bits remain (vs 18.50M baseline). The remaining
gap is dominated by **per-block encoder tie-break noise** between our
pure-Rust `bc7_pure::encode_blocks(... Basic)` and bc7e ISPC. The encoder-
discrimination probe (`dev/bc7_probe_prod_encoder.py`) verifies this:

- On **clean POT-target downscale** (`bafkreiczuewg3pf`, 1340×670 → 1024×512):
 `ours-rust == bc7e/basic/perc=True == 99.77 %` match-vs-prod. The remaining
 0.23 % is bc7e ISPC's refinement-pass scheduling, which our pure-Rust port
 doesn't bit-exactly reproduce.
- On **identity 1024×1024** (`bafybeih4xgkars5`): `ours-rust == bc7e/basic == 80.95 %`,
 same as bc7e — confirming the encoder choice is correct (basic beats every
 other preset by ≥20 pp), the residual is per-block tie-break noise.
- On **identity 512×512** (`bafkreiffskc6wyx`, `bafkreibbuyvqmb6`, etc.):
 similar pattern — bc7e/basic is the right preset, the residual is encoder
 noise on the remaining ~5-8 % of blocks per case.

Even on **upscale post-fix**, `ours-rust == bc7e/basic` on mip0 — meaning
the encoder side is correct and any further improvement on upscale has to
come from an *even better* resize model (or from disassembling Unity's
URP Blit shader, explicitly out of scope).

## Open paths (none mechanical)

1. **bc7e ISPC refinement-pass parity.** The pure-Rust port replicates
 bc7e/basic's mode selection and partition search exactly but the
 per-block refinement loop (`evaluate_solution` + endpoint snap) has
 small ordering differences vs ISPC's `cmov`-heavy reference. Closing
 this is the `tex_close_60.md` Path-3 multi-day investigation. Bound by
 `~15M bits` (= 16.9M total − 1.5-2M residual upscale-still-imperfect).

2. **GPU-correct upscale model.** Even with `bilin_float`, our upscale
 doesn't reach the >99 % match seen on clean downscale. The Unity URP
 `Graphics.Blit` with `FilterMode.Bilinear` runs through the lavapipe/
 Mesa software rasterizer (same as the downscale path) but the
 instruction stream is different (different shader, different sampler
 state). An instrumented capture (analogous to the harvested
 `workdir/research/resize_dump/` for downscale) would pin the exact
 per-tap rounding. Estimated upper bound ≤ 4M bits.

3. **PNG-decoder precision.** All 56 residuals are PNG sources decoded
 via the `image` crate (Rust) → RGBA. Prod uses Unity's
 `Texture2D.LoadImage` which goes through `libpng` under
 `UnityEditor.Texture2D` import. Edge cases (alpha straight vs
 premultiplied, gAMA/cHRM handling, indexed-palette conversion) could
 produce single-bit differences in the source RGBA that propagate
 through resize → BC7. Not yet measured; bound unknown.

## Method note

New scripts in this worktree:

- `dev/measure_bits_texture2d_mac.py` — mac equivalent of the windows
 measurement (identical numbers, same 56 CIDs).
- `dev/classify_texture2d_residuals.py` — per-class enrichment +
 aggregation tool. Reports source-kind, scale direction, POT-ness, sRGB
 flag, mip count, format for every residual, then bins by
 `(scale, source, platform)` for actionable subclasses.
- `dev/upscale_sweep.py` — for the 16 upscale + mixed CIDs, encodes mip0
 under each candidate resampler via bc7e/basic and prints per-case +
 totalled bits-diff. Use this to vet any future resize model change
 before code change.

All scripts read `ABGEN_AB_BIN` and default to the worktree
`abgen-rs/target/release/ab-build-local`.

## Reproduce

```bash
cd /home/dcl/umbrella/.claude/worktrees/<wt>/ab-generator/abgen-rs
/home/dcl/linux-rigging/dcl-shell -c "cargo build --release --bin ab-build-local"
cd /home/dcl/umbrella/ab-generator
ABGEN_AB_BIN=<wt>/.../target/release/ab-build-local \
 nix-shell --run "python <wt>/.../dev/measure_bits_texture2d_windows.py"
ABGEN_AB_BIN=… nix-shell --run "python <wt>/.../dev/measure_bits_texture2d_mac.py"
ABGEN_AB_BIN=… nix-shell --run "python <wt>/.../dev/classify_texture2d_residuals.py"
ABGEN_AB_BIN=… nix-shell --run "python <wt>/.../dev/upscale_sweep.py"
```
