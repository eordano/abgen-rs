# KHR_materials_specular + DXT1 encoder (LANDED)

Closes the missing-Texture2D-pair residual on bundles whose glTF materials
use `KHR_materials_specular.specularColorTexture`.

## Symptom

`bafybeih5pwnvvwfud…` (Robot wearable, 5 materials) and
`bafybeibetxljzlu2…` (Glass-block scene, 23 materials) both emit one fewer
Texture2D pair than prod. Pre-fix textures:

| variant | prod | ours (pre-fix) |
|---|---:|---:|
| `image_5` (ARGB32, fmt 5)  | present | **missing** |
| `image_5` (DXT1,   fmt 10) | present | **missing** |

(Same story for `image_19` in the second bundle.) Every other Texture2D in
both bundles matches prod pid-for-pid; only the specular-color pair is
absent.

## Root cause

`src/gltf.rs::parse_materials` honored
`KHR_materials_pbrSpecularGlossiness` and `KHR_texture_transform`, but not
`KHR_materials_specular`. The latter routes one extra texture
(`specularColorTexture`) into Unity's `_SpecColorMap` slot. Because
`Material` had no field for it, the texture pipeline in `builder.rs` never
emitted a Texture2D pair for the underlying image.

Prod's streamed variant of the specular-color slot is `TextureFormat.DXT1`
(Unity discriminator 10), not the default `BC7` (25). DCL's URP-Lit shader
expects DXT1 on that slot, and no DXT1 encoder existed in `abgen-rs`.

## Fix (5 steps, all LANDED)

1. **`src/scene.rs`** — add `specular_color_image: Option<TexRef>` to
 `Material`.
2. **`src/gltf.rs::parse_materials`** — read
 `extensions.KHR_materials_specular.specularColorTexture`, populate
 `Material.specular_color_image`, fold its `texCoord` into
 `uses_uv_channel_select`, record its `KHR_texture_transform` under the
 `_SpecColorMap` key.
3. **`src/materials.rs`** —
 - extend `MATERIAL_TEXTURE_SLOTS` from 6 → 7 with a `_SpecColorMap`
     accessor;
 - add `_SpecColorMap` to `LINEAR_SLOTS` (the slot is linear-color per
     spec; the in-glb ARGB32 dup follows this);
 - add `classify_dxt1_images(scene)`: an image is classified as DXT1 iff
     it is referenced *only* via `_SpecColorMap` and no other slot.
4. **`src/texprofile.rs`** —
 - add `TF_DXT1 = 10`;
 - add `dxt1_profile(src, max_size)` — same `bc7_target_size` rounding +
     `default_mip_count` mip chain as `bc7_profile`, but hard-codes
     `color_space = 1` (sRGB) on the streamed variant (matches prod's
     TextureImporter quirk where the DXT1 specular-color is tagged sRGB
     even though the in-glb dup is linear) and pins
     `is_alpha_channel_optional = true` (DXT1 has no alpha channel);
 - add `texture_profile_dxt1(...)` — the slot-aware wrapper that returns
     the in-glb uncompressed Profile + the DXT1 Profile.
5. **`src/dxt1_pure.rs`** (new ~270 LOC) — pure-Rust DXT1 / BC1 encoder.
 PCA-based endpoint selection with power iteration on the 3x3 RGB
 covariance, sum-of-squared-diffs index assignment, always pinned to
 4-color mode (c0 >= c1 — DCL's spec-color slot is opaque, 3-color mode
 would shade index-3 texels transparent black). Mip chain helper
 `encode_dxt1_mip_chain` mirrors `bc7_pure::encode_bc7_mip_chain_with_profile`
 so `builder.rs::texture_tree_with_wrap` can swap encoders on
 `prof.texture_format == TF_DXT1` alone.

**`src/builder.rs`** wiring:
- store `dxt1_images: HashSet<usize>` on the Builder (populated in
 `build` from `materials::classify_dxt1_images`);
- in `texture`, branch `texture_profile_dxt1` when the image is in that
 set;
- in `texture_tree_with_wrap`, dispatch `dxt1_pure::encode_dxt1_mip_chain`
 when `prof.texture_format == TF_DXT1`.

## Validation

`bafybeih5pwnvvwfud_windows`:

| dimension | prod | post-fix | match? |
|---|---:|---:|---|
| Texture2D count                | 16 | 16 | ✅ |
| `image_5` ARGB32 pid/W/H/cs/mips/cis/alphaOpt | (7029.., 512, 512, 0, 1, 1048576, 0) | identical | ✅ |
| `image_5` DXT1   pid/W/H/cs/mips/cis/alphaOpt | (6111.., 512, 512, 1, 10, 174776, 1) | identical | ✅ |
| `image_5` ARGB32 bytes (vs prod)               | 1,048,576 B | **bit-exact** | ✅ |
| `image_5` DXT1 bytes (vs prod)                 | 174,776 B   | structural-match, 677,640 bits diff in payload | partial |
| Bundle byte size                              | 5,317,962 B | 5,295,875 B | -22,087 B |
| Bundle byte size (pre-fix)                    | n/a | 5,285,037 B | -32,925 B |

The DXT1 *structure* (block count, byte count, mip count, alpha-optional
flag, color-space tag, lightmap-format) is identical to prod; only the
*payload* differs because prod uses an undisclosed BC1 encoder (likely
Crunch RDO) and our encoder is a plain PCA-then-snap. Closes ~10.8 KB of
bundle-size gap per Robot-class bundle (the ARGB32 in-glb pair is now
bit-exact; the DXT1 still drifts on the payload).

## Corpus impact (windows, 2174 prod-paired bundles)

Pre-fix baseline: **457,783 ppm-bits**.
Post-fix: **457,551 ppm-bits**.
Delta: **-232 ppm-bits** (-30 byte-identical, +0 regressions).

Modest because only 2 of 2174 bundles contain
`KHR_materials_specular.specularColorTexture`. Both bundles have their
specular-color tex pair structure-perfect post-fix; the residual is the
BC1 payload divergence.

## Bit-exactness — out of scope here

Prod's BC1 encoder is Unity's bundled DXT compressor, which appears to be
the Crunch library configured with RDO+sRGB. Matching it bit-exactly would
require porting Crunch's rate-distortion loop, which is well-known to be
~5k LOC of finely-tuned heuristics. Out of scope; the encoder we shipped
gets the *structure* right (the load-bearing invariant — wrong format byte
or wrong mip count would crash Unity's GPU upload) and produces
acceptable quality for the slot's role (specular-tint lookup).

## Files

- new: `src/dxt1_pure.rs`
- changed: `src/scene.rs`, `src/gltf.rs`, `src/materials.rs`,
 `src/texprofile.rs`, `src/builder.rs`, `src/lib.rs`, `Cargo.toml` (added
 `ab-dump-tex-bytes` diagnostic bin), `src/bin/ab-show-textures.rs`
 (added cs/lmf columns)
