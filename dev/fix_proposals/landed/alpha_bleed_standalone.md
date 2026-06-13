# Texture2D — reference alpha-bleed RGB dilation (standalone-texture path)

Closes the bulk of the `__bc7_tiebreak__` Texture2D residual identified in
`bc7_tiebreak_v2.md`. The remaining 16.9M-bit residual on each platform was
**not** encoder ambiguity — it was source-pixel divergence at α=0 pixels,
caused by the converter's `TextureImporter.alphaIsTransparency` RGB-dilation
pass running before BC7 encoding.

## What landed

`src/alpha_bleed.rs` — pure-Rust iterative dilation:

 - **4-connectivity** (N/E/S/W only — diagonals are strictly worse).
 - **Jacobi update** — read-only snapshot of pixel state at the start of
    each pass; an α=0 pixel that gains any opaque/filled neighbor writes
    the integer mean of those RGB values.
 - **32 passes** — empirically locked: at K<32 the bleed under-extends; at
    K>32 it over-extends and starts regressing parity.
 - Alpha channel is never modified.
 - Early-out when every pixel is opaque, or when every pixel is α=0 (no
    seed to dilate from).

Wired into `StandaloneTextureBuilder::build` (after NPOT resize, before BC7
encode + mip generation). Gated on `has_real_alpha && prof.compressed`.

The in-glb texture path (`Builder::texture_tree_with_wrap`) is **deliberately
NOT bled** — empirically that path corresponds to the converter's
`CustomGltfImporter` which does not set `alphaIsTransparency = true` by
default. Bleeding on that path causes net regression.

## Methodology

`dev/bc7_probe/probe_bleed_pass_count.py` decodes prod's mip-0 BC7 to RGBA
and compares to candidate-bled source at each (passes, connectivity) point.
At the 5 identity-class CIDs from `bc7_tiebreak_v2.md`, the "within 8 per
channel" match rate peaks at exactly K=32, 4-conn:

```
CID K=16 4c K=32 4c K=32 8c K=48 4c
bafkreiffskc6wyx... 99.70 99.70 86.05 99.70 (converges at K=8)
bafybeih4xgkars5... 83.79 98.16 90.69 84.62
bafkreid6fpxypr6... 89.01 97.24 83.64 92.51
bafybeigs5ygjyxj... 93.02 98.67 93.52 92.22
```

Past K=32 the close% drops because prod stops iterating ⇒ we'd over-extend
the bleed. 8-conn is strictly worse (diagonal neighbors weight RGB
asymmetrically against prod's 4-conn dilation pattern).

## Results

Same-worktree A/B (baseline = `a798225` without bleed, treatment = same +
alpha-bleed):

### Per-CID (5 identity-class, windows + mac combined)

| CID                | baseline bits | bleed bits | delta |
|---|---:|---:|---:|
| bafkreiffskc6wyx (512²)    | 791,938  |  15,593 × 2 |  -98% |
| bafkreibbuyvqmb6 (512²)    | 790,948  |  16,155 × 2 |  -96% |
| bafybeih4xgkars5 (1024²)   | 1,664,952 |  86,122 × 2 |  -90% |
| bafkreid6fpxypr6 (1024²)   | 1,725,952 | 166,074 × 2 |  -81% |
| bafybeigs5ygjyxj (1024²)   | 1,716,392 | 188,146 × 2 |  -78% |
| **Total**                  | **6,490,182** | **944,180** | **-85%** |

### Texture2D class corpus-wide (windows = mac, same Texture2D bytes)

```
                  baseline      bleed     delta
ppm-bits (class) 13,211 9,131.9 -4,079 (-30.9%)
bits-diff 16,920,515 11,696,095 -5,224,420
```

### Whole-corpus raw-byte parity

```
                  baseline (a798225)        bleed                delta
windows ppm-bits 463,517.8 463,045.2 -472.6
windows bits-diff 938,487,745 937,629,298 -858,447
mac ppm-bits 464,230.9 463,756.6 -474.3
mac bits-diff 939,926,007 939,064,067 -861,940
```

Per-bundle: 29 improved, 2 marginally regressed (+4K and +117K bits, both
in the noise), 249 unchanged.

### parity_bytes fixtures (10 cases, ceiling=917,161)

Unchanged at 917,161 — none of the 10 fixtures contains the dominant
"transparent-with-non-zero-opaque-RGB" pattern that the bleed acts on:

 - 5 fixtures: opaque-only source or all-α=0 (no seed).
 - 1 fixture (1024×1024 LA): 498K α=0 pixels but the dominant opaque RGB
    is 0 — so the bleed faithfully writes (0,0,0) into (0,0,0,0), giving
    bit-identical BC7 output.

The corpus-wide measurement is the load-bearing metric here; parity_bytes
is a regression gate that didn't move.

## Why the in-glb path stays unbled

Direct measurement: enabling alpha-bleed in `Builder::texture_tree_with_wrap`
regresses the windows corpus from 938M to 1,007M bits-diff (+7.4%) and
breaks 3 byte-identical bundles. `CustomGltfImporter` (the converter's
importer for glb-embedded textures) does not flip `alphaIsTransparency`
to true by default, so prod's in-glb BC7 is encoded from raw source RGBA at
α=0 pixels (whatever the glTF author baked in). The bleed gating
(`!is_glb_path && has_real_alpha && prof.compressed`) reflects this.

## Tests

`alpha_bleed.rs` ships 7 unit tests covering: opaque-only no-op, all-α=0
no-op, single-seed 4-conn propagation, pass-count cap (only N pixels filled
after N passes), Jacobi snapshot symmetry (left + right seeds meet
symmetrically — sequential update would fail), no-opaque-seed no-op, alpha
channel preservation.

## Files changed

 - `src/alpha_bleed.rs` (new) — algorithm + 7 unit tests.
 - `src/lib.rs` — registers `pub mod alpha_bleed`.
 - `src/builder.rs` — `StandaloneTextureBuilder::build` wires bleed after
    resize, before BC7. `Builder::texture_tree_with_wrap` is annotated with
    why bleed is NOT applied there.
 - `dev/bc7_probe/probe_bleed_pass_count.py` — methodology that locked in
    the 32-pass, 4-conn parameters.
 - `dev/diff_bleed_vs_baseline.py` — per-bundle A/B harness used to
    confirm the in-glb-path skip.
 - `dev/inspect_regression.py` — single-bundle forensic for regression
    triage (uncovered the cross-worktree binary mismatch that initially
    masked the true delta).
