# BC7 "tiebreak" residual — re-classified as alpha-bleed, not encoder ambiguity

> **Status: landed.** The re-diagnosis in this proposal drove
> the alpha-bleed dilation pass landed in commit `b3bea9c`. See
> `landed/alpha_bleed_standalone.md` for the implemented algorithm.
>
> *Original investigation note follows.* The previous follow-up
> (`texture2d_followup.md`) labelled the remaining 16.9M-bit Texture2D
> residual as "bc7e ISPC refinement-pass parity" / per-block tie-breaks
> between our pure-Rust port and bc7e ISPC's refinement-loop scheduling.
> **This deep-dive shows that diagnosis was wrong.** The remaining
> residual is dominated by Unity's `TextureImporter.alphaIsTransparency`
> RGB-dilation/bleed preprocessing — a *source-pixel* divergence
> upstream of BC7, not a BC7 encoder tie-break.

## TL;DR

For all 22 identity-class residual CIDs (and likely the 18 downscale + 14
upscale + 2 mixed CIDs too), every differing 4×4 BC7 block in mip 0 contains
at least one `alpha == 0` pixel. At those transparent pixels:

- **Source PNG**: RGB = (0, 0, 0) (canonical transparent-black)
- **Our build's BC7 (decoded)**: RGB ≈ (0, 0, 0) (we feed source bytes
 through unchanged, encoder dutifully reproduces)
- **Unity's BC7 (decoded)**: RGB ≈ (100–255, 100–255, 100–255) —
 Unity has **bled the RGB channel from nearby opaque pixels into the
 alpha=0 regions** before encoding, the standard
 `alphaIsTransparency` preprocessing.

Because the input pixels to BC7 differ (transparent-black vs
transparent-bleed-gray), the resulting BC7 blocks are nowhere near each
other in mode/partition/endpoint/index space. **No bc7e preset, no
`Params::*` tweak, no refinement-pass change can close this — the
divergence is upstream of BC7.**

## Methodology

`dev/bc7_probe_prod_encoder.py` already wraps every bc7e preset
(`slow`, `basic`, `fast`, `veryfast`, `ultrafast`, `veryslow`,
`slowest`) × `perceptual ∈ {True, False}` + `etcpak`, against prod.

Ran 5 representative identity-class CIDs (windows + mac platforms,
identical bit-diff between platforms confirms target-agnostic
residual):

```
| CID                | wxh       | ours-rust | bc7e/basic/perc=T | next-best          |
| bafkreiffskc6wyx   | 512x512   | 19.95%   | 19.95%            | bc7e/{slow,fast,...} 19.90% |
| bafkreibbuyvqmb6   | 512x512   | 19.95%   | 19.95%            | bc7e/{slow,fast,...} 19.90% |
| bafybeih4xgkars5   | 1024x1024 | 80.95%   | 80.95%            | bc7e/fast=56.16%             |
| bafkreid6fpxypr6   | 1024x1024 | 82.76%   | 82.76%            | bc7e/basic/perc=F=80.50%    |
| bafybeigs5ygjyxj   | 1024x1024 | 85.84%   | 85.84%            | bc7e/fast/perc=T=73.00%     |
```

Two facts jump out:

1. **`ours-rust` matches `bc7e/basic/perc=True` exactly** for every CID
 (our pure-Rust port is bit-exact bc7e ISPC at the basic preset — the
 port is *correct*).
2. **No bc7e preset clears > basic** on any CID. The pure-Rust port
 isn't underperforming; bc7e itself can't reach prod's bytes from our
 input pixels. *Per-block tie-break differences between bc7e variants
 exist but never bridge the gap to prod's output.*

## The smoking gun — alpha-bleed in prod

Decoded prod's mip 0 BC7 vs the source PNG (flipped to Unity's
bottom-left origin) for the 5 identity-class CIDs:

```
cid wxh alpha=0 src RGB@a=0 prod RGB@a=0 bleed?
bafkreiffskc6wyx 512x512 59.0% (0,0,0) (205,205,205) YES
bafkreibbuyvqmb6 512x512 59.0% (0,0,0) (205,205,205) YES
bafybeih4xgkars5 1024x1024 33.4% (166,133,128) (222,128,125) YES
bafkreid6fpxypr6 1024x1024 26.2% (0,0,0) (59,51,51) YES
bafybeigs5ygjyxj 1024x1024 64.5% (37,37,37) (48,31,48) YES
```

For `bafkreiffskc6wyx`:

```
total mip0 4×4 blocks : 16,384
blocks containing at least one α=0 pixel : 13,067 (79.8%)
blocks where ours BC7 ≠ prod BC7 (mip 0) : 13,067
of which contain at least one α=0 pixel : 13,067 (100.0%)
of which are fully opaque : 0
```

**100% of differing blocks contain at least one transparent pixel.**
Every block whose 16 source pixels are all α > 0 is byte-exact between
ours and prod. The encoder is fine.

The 19.95% block-match rate maps cleanly to "20% of mip 0 blocks have no
transparent pixels, and those 20% match prod byte-for-byte; the other
80% contain at least one α=0 pixel and our (0,0,0,0) ≠ Unity's
(205,205,205,0) input causes a mode/partition/endpoint cascade".

## What we tried (and why none of it works)

`Params::basic` is parameterised. We patched the following variants on
top of basic, rebuilt `ab-build-local`, and re-measured the 5 CIDs
(10 measurements each, windows + mac always identical, total bits-diff
shown):

| variant | total bits | delta vs baseline | verdict |
|---|---:|---:|---|
| **baseline** (`uber=1 pbit=false al7=1`) | **6,490,182** | — | reference |
| A: `pbit_search = true` | 8,182,630 | **+26 %** | strictly worse |
| B: `uber_level = 2` | 7,226,688 | **+11 %** | strictly worse |
| C: `al_max_mode7 = 2` (restore slow's val) | 6,628,658 | **+2 %** | slightly worse |
| D: keep `use_mode[4]`/`use_mode[5]` enabled | 6,490,182 | 0 % | no-op (only opaque path) |
| E: `refinement_passes = 2` | 6,591,036 | **+2 %** | slightly worse |

Every mechanical knob on `Params::basic` either makes the residual
strictly worse or has no effect. That's the second confirmation: no
encoder-side fix can close this gap, because the gap isn't on the
encoder side.

## Recommendation — defer the v2 BC7 work

The remaining 16.9M-bit Texture2D residual (~13,211 ppm-bits, windows +
mac) is **not** a BC7 encoder tie-break / refinement-pass parity
problem. It is a **`TextureImporter.alphaIsTransparency` RGB-bleed**
problem. Closing it requires implementing Unity's alpha-bleed
preprocessing pre-BC7, which is a self-contained-but-non-trivial workstream:

1. **Identify Unity's bleed algorithm.** Likely candidates (in
 order of probability):
 - Unity's `alphaIsTransparency` calls the
     `EditorUtility.GenerateAlphaIsTransparencyMipmaps` /
     `TextureImporter.alphaIsTransparency` path which runs a
     vertex-shader-style dilation (a few passes of nearest-opaque-RGB
     blend) before color-mip generation.
 - Could also be the `WeightedAverageFilter` /
     `PreserveAlphaCoverage` path that re-weights mip RGB by alpha
     coverage.
2. **Probe the bleed pattern.** Sample 5 prod mip-0 textures at the
 alpha=0/alpha>0 boundary and reconstruct Unity's bleed by inverting
 one pass: for each (i, j) with α=0, fit `prod_rgb[i,j] = f(neighbors
 where α>0)`. If `f` is "nearest opaque pixel" we'll see the bleed
 match the geodesic distance map; if it's a Gaussian-weighted blend
 we'll see distance-weighted decay.
3. **Implement in `src/builder.rs`** (or a new `src/alpha_bleed.rs`)
 as a preprocessing step gated on `alphaIsTransparency = true`
 (always true on the standalone path — Unity's `ImportTextures` sets
 it unconditionally).
4. **Measure.** Re-run the windows + mac corpora and the 5 identity
 CIDs. Expected: every identity-class case drops to near-zero
 bits-diff (mid-image-region cases were already ≤ 9 bits-diff; the
 high-bits cases have huge transparent regions which is exactly what
 bleed affects).

The downscale + upscale + mixed subclasses likely also have an alpha-bleed
component (their high `ppm-img` values — 94k, 116k, 162k — match the
identity class' 62k pattern), though they additionally have a
resize-divergence component. Investigation of the bleed step should
quantify how much of each subclass's residual is bleed vs resize.

## Encoder-side work — defer until alpha-bleed lands

The pure-Rust BC7 port is bit-exact `bc7e ISPC` at both `slow` and
`basic` presets (independently confirmed in
`landed/texture2d_windows.md` § "Bit-exact bc7e-port confirmation" and
re-confirmed here for all 5 CIDs). Once the alpha-bleed preprocessing
lands and the identity-class CIDs drop into the "≤ a few hundred bits
diff" range, the genuine BC7 tie-break residual will surface as the
floor and can be measured properly. Right now it's hidden under a
much-larger alpha-bleed signal.

If a Unity Burst SIMD trace becomes available (e.g. via instrumenting
`UnityEditor.TextureImporter` on a dummy 4×4 PNG with one transparent
corner), it would resolve **both** the bleed algorithm (input pixels
post-importer-preprocessing) and any residual encoder tie-breaks in one
pass. Until then: implement alpha-bleed, re-measure, then decide.

## Method-note — the test scripts

- `dev/bc7_probe_prod_encoder.py <cid> [platform]` — encoder
 discriminator (bc7e × every preset × perceptual flag + etcpak).
- `dev/classify_texture2d_residuals.py` — per-class subclass breakdown
 (now confirms identity / downscale / upscale / mixed counts).
- `dev/bc7_tiebreak_variant.py [label]` — measure bits_diff for the 5
 identity-class CIDs across windows + mac after editing
 `Params::basic`; emits per-CID + total. Use this for any future
 mechanical-variant A/B.
- `bc7_decode + alpha-region overlay` — ad-hoc, but the recipe is
 "decode mip 0 via `Image.frombytes(..., 'bcn', 7)`, compute
 `(α==0)` mask of source, intersect with `(ours_blk != prod_blk)`":
 if 100% overlap, the residual is bleed; if not, there is a true
 encoder component to investigate.

All scripts default to the worktree's `target/release/ab-build-local`
via `ABGEN_AB_BIN` / sibling-binary auto-detection, so each variant
under test runs end-to-end without manual binary plumbing.
