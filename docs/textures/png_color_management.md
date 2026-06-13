# PNG color management: which ancillary chunks the converter honours on decode

Some source PNGs carry color-management chunks — `gAMA` (gamma), `iCCP` (an
embedded ICC profile), `sRGB` (a rendering-intent assertion), or `cHRM`
(chromaticities). The converter's standalone-texture decode path runs through
FreeImage, which can in principle apply any of these as a color/gamma transform *before*
resize and BC7 encoding. abgen decodes raw samples and (almost always) ignores
them; this page records exactly which chunks matter, derived from the reference
bytes.

## Verdict per chunk

The val300 + shibu-world corpus carries every chunk in quantity. The oracle is
`examples/decode_dist.rs` (alpha-weighted premultiplied RGB distance of the
BC7-decoded mip-0, run against `ABGEN_REAL_TEXTURES=1` output so the texture is
real artwork, not the flat mean-color stub) plus straight byte-identity on the
no-resize populations.

| Chunk | Population (standalone) | Behaviour | Evidence |
|---|---|---|---|
| `gAMA` (non-trivial) | 1 | **Applied** — gamma decode | mean RGB distance 13.5 → 0.0 once applied |
| `gAMA` (≈ 1/2.2) | ~270 | No-op | exponent within 3e-4 of identity |
| `iCCP` (any profile) | ~1125 | **Ignored** | Adobe RGB / Generic RGB / Display-P3 all decode-identical (dist 0.0) |
| `sRGB` chunk | ~540 | No-op | alpha-matched RGB distance median 0.0, max 1.07 (BC7 noise) |
| `cHRM` | ~415 | No-op | same |

### gAMA — the one transform that matters

A non-trivial embedded gamma shifts every pixel. The reference reproduces

```
out = round( (sample/255) ^ (1 / (gamma · 2.2)) · 255 )
```

i.e. FreeImage applies its file gamma against a fixed **screen gamma of 2.2**,
correction exponent `1/(gamma·2.2)`. For the corpus case `gAMA = 0.55531` the
exponent is `0.8185`; input 28→42, 128→145, 224→229. Implemented as a 256-entry
LUT (`apply_png_gamma`), round-half-up, applied to RGB only (alpha untouched).
The other candidate curves were ruled out by the per-level transfer table
(`examples/gamma_pairs.rs`, median of BC7-decoded ref vs un-gamma'd ours):
`(v/255)^(gamma·2.2)` and `(v/255)^(1/gamma)` miss by 26 and 45 levels
respectively; the screen-2.2 exponent fits to within BC7's ±1 noise.

### iCCP — ignored, including non-sRGB color spaces

This was the population at risk of needing a full ICC transform. It does not.
Decoding mip-0 of every `iCCP`-carrying standalone texture against the reference,
restricted to texels where the alpha channels match (so the comparison is a pure
color test, not a stub/bleed artifact), shows a **median RGB distance of 0.0**
across all profiles — `Adobe RGB (1998)`, `Generic RGB Profile`,
`kCGColorSpaceDisplayP3`, `Photoshop ICC profile`, `sRGB IEC61966-2.1`. If
FreeImage applied an Adobe-RGB→sRGB transform those would shift visibly; they do
not. The few large distances that appear in the raw oracle are all **oversized
sources** (any axis > 1024) whose reference texture is a flat mean-color stub
(the headless batchmode oversize-collapse artifact, see
`docs/textures/standalone_texture_validation_regression.md`); abgen's default output
reproduces those stubs byte-identically. No ICC code is needed.

### sRGB / cHRM — no-ops

The `sRGB` chunk only asserts that the data is already sRGB, so honouring it is
a no-op; `cHRM` is ignored when present. Both confirmed on the pure-chunk
populations (no gAMA, no iCCP): alpha-matched RGB distance median 0.0, p95 0.001.

## The profile-aware gate

A blanket transform would regress the ~270 identity-gamma textures, so the gate
is narrow. `png_gamma_to_apply` returns `Some(gamma)` only when:

1. a `gAMA` chunk is present, **and**
2. no `sRGB` chunk is present (per the PNG spec, `sRGB` pins gamma to the sRGB
   default — a no-op — and overrides `gAMA`), **and**
3. the gamma is non-trivial: the mid-tone (v=128) moves by ≥ half a byte once
   put through `1/(gamma·2.2)`. This classifies `0.45455`, `0.45454`, and even
   `0.4547` as identity and skips them.

Across the whole corpus the gate fires on exactly **one** unique standalone PNG
(`bafybeietj4xdydz5z4by77c3iksnpexc5fpskeazvf3o5576ps6uhv3c74`, `gAMA=0.55531`).

## The glb path is untouched

glb-embedded images decode through glTFast, not FreeImage — a different decoder
with different upsampling (`docs/textures/texture_jpeg_decoder.md`). Scanning all 3127
glb sources, 173 embedded PNGs carry a `gAMA` chunk but **none is non-trivial**
(every one is ≈ 1/2.2). So no glb image would exercise a gamma transform, and
the glb decode path is left as-is. (Whether glTFast honours gAMA is moot — the
corpus has no glb case to disambiguate it, and applying the standalone rule
there blindly would be unverified speculation.)

## Byte-id impact

Landing the gamma transform leaves the val300 windows byte-id count **unchanged
(zero change, zero regression)**: the single affected bundle was already
non-byte-id because of the irreducible BC7 encoder float-order wall, and its
texture pixels are now correct (decode distance 13.5 → 0.0) rather than uniformly
darkened. The value is **serving fidelity** — that texture now renders at the
right brightness — and closing the wall in the standalone-texture parity
decomposition (`docs/textures/standalone_texture_remaining_walls.md`).
