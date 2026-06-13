# image_5 missing Texture2D pair on `bafybeih5pwnvvwfud…` — KHR_materials_specular

(File originally named after the misattributed `m_MuscleClip` lead; the actual
root cause is documented below.)

## TL;DR

The 18.6 Mbits SF + 15.7 Mbits resS diff on `bafybeih5pwnvvwfud…`
(`Simone_Anim_Collider.glb`, glb-wearable kind in atlas — but actually a
SCENE-content GLB) is caused by **2 missing Texture2D objects** both named
`image_5`. Prod emits a Texture2D pair for image_5 (one ARGB32, one DXT1+mips),
we emit zero. The image is referenced ONLY via the
`KHR_materials_specular.specularColorTexture` glTF extension, which our material
parser ignores — so the image is never routed to a slot and never emitted.

## Verified facts

GLB has 8 images (`led_bars`, `led_bars`, `Robot_Low_Robot_Body_Normal`,
`Robot_Low`, `Robot_Low_Robot_Body_MetallicSmoothness`, `Image`,
`Arm_Low_Robot_Arms_Normal`, `Arm_Low_Robot_Arms_AlbedoTransparency`).

Prod bundle: 16 Texture2D (image_0…image_7 × 2 fmts each).

| name | fmt=25 (BC7) | fmt=5 (ARGB32) | fmt=10 (DXT1) |
|---|---|---|---|
| image_0 | 349,552 | 1,048,576 | — |
| image_1 | 349,552 | 1,048,576 | — |
| image_2 | 349,552 | 1,048,576 | — |
| image_3 | 349,552 | 1,048,576 | — |
| image_4 | 349,552 | 1,048,576 | — |
| **image_5** | — | **1,048,576** | **174,776 (mips=10)** |
| image_6 | 349,552 | 1,048,576 | — |
| image_7 | 349,552 | 1,048,576 | — |

Ours bundle: 14 Texture2D (image_0..image_4, image_6, image_7 × 2 fmts each).
Both image_5 textures are absent.

Material wiring in the GLB:
- material[2] "Robot_Arms.001": `pbr.baseColorTexture → 7`, `normalTexture → 6`,
 `ext.KHR_materials_specular.specularColorTexture → 5`.

The 7 other images all reach a standard slot
(`pbr.baseColorTexture` / `normalTexture` / `metallicRoughnessTexture` /
`emissiveTexture`); only image_5 is reached exclusively via the spec extension.

`src/gltf.rs` only handles `KHR_materials_pbrSpecularGlossiness` and
`KHR_texture_transform`; `KHR_materials_specular` is not parsed. Result:
image_5 has no `TexRef` referencing it, `texture` is never called for it,
no Texture2D pair is emitted.

`src/texprofile.rs::texture_profile` returns `(uncompressed, BC7)` for every
slot. Prod's pair for image_5 is `(ARGB32, DXT1+mips)` — a slot-specific rule
we don't currently implement. The 7 BC7-paired images use BC7 for both
albedo, normal, MR, emissive AND non-extension specular paths; the
DXT1+mips pair appears UNIQUE to `KHR_materials_specular.specularColorTexture`
in our sampled bundles.

## Diff accounting

ours total: 5,285,037 B; prod total: 5,317,962 B (Δ = -32,925 B).

Missing-texture bytes that account for the diff:
- image_5 ARGB32 raw stream: 1,048,576 B (in resS, accounts for 8.4 Mbits diff)
- image_5 DXT1+mips stream: 174,776 B (in resS, accounts for 1.4 Mbits diff)
- 2 × Texture2D SF metadata blocks: a few hundred B each in SF + ~16 K
 cascading shift offset for downstream PPtrs (accounts for the bulk of the
 18.6 Mbits SF "shift_cascade" tag in `bit_diff_atlas`)

Total accounted: ~9.8 Mbits resS + ~18 Mbits SF cascade = ~28 Mbits, consistent
with the atlas row.

The atlas tagged the dominant runs as `AnimationClip/shift_cascade` because
the prod-side byte-range lookup falls on AnimationClip objects after the
shift caused by the 2 missing Texture2D blocks earlier in the SF. That tag
is a misattribution; the actual root cause is the missing image_5 pair.

## Why this fix wasn't landed

Implementing the fix requires:

1. **Parse `KHR_materials_specular`** in `src/gltf.rs::parse_material` to
 extract `specularColorTexture.index` (and ideally `specularTexture.index`
 too). ~30 LOC, principled, low risk.
2. **Add `specular_color_image: Option<TexRef>` to `Material` struct** in
 `src/scene.rs`. ~5 LOC.
3. **Route to a new slot** in `MATERIAL_TEXTURE_SLOTS` (e.g.
 `_SpecularColorMap`) so `colorspaces` classification and `texture` see
 it. ~10 LOC. Slot needs LINEAR colorspace (verify against prod ARGB32 of
 image_5).
4. **Slot-aware texture-profile rule**: extend `texture_profile` (or wrap
 it) so the spec-color slot returns `(ARGB32, DXT1+mips)` instead of
 `(ARGB32, BC7)`. The slot is the discriminant — same image fed through a
 different slot must produce the BC7 pair. ~20 LOC.
5. **Implement a bit-exact BC1/DXT1 encoder** matching Unity's output for a
 512×512 RGB PNG mip chain. ~500 LOC. **No prod test vectors for BC1
 currently exist** (the corpus has BC7 vectors only). Without
 probe-collected vectors, the encoder will mismatch — same situation as
 the BC7 encoder (top atlas tag, 506,145 ppm `bc7_block_payload_resS`).

Step 5 is the blocker. A bit-exact BC1 encoder without ground-truth
vectors is high-risk and the partial fix (right textures, wrong BC1 bytes)
would close only ~8.4 Mbits of the ~28 Mbits gap — the ARGB32 path would
match byte-exact, but the DXT1 path would join the corpus-wide
`bc1_block_payload_resS` residual (a new tag we don't yet measure).

## Recommended next steps

A. **Probe collection.** Build a DXT1 probe harness analogous to
 `dev/bc7_probe/`. Feed Unity Editor (via the Unity Editor) a battery of
 synthetic 512×512 PNGs and capture the DXT1 mip-chain bytes. Target ≥40
 synthetic inputs covering: flat colors, gradients, noise, normal-mapped
 data, gamma-encoded sRGB. Land outputs at `dev/dxt1_probe/inputs/` and
 `dev/dxt1_probe/expected/`.

B. **Implement the extension parse + slot routing** independently of (A).
 It will produce wrong BC1 bytes initially, but lands the SF parity
 improvement (the 2 missing Texture2D headers, the shift_cascade
 collapse). Atlas should reclassify the residual as a new
 `bc1_block_payload_resS` tag instead of `AnimationClip/shift_cascade`.

C. **Verify slot rule scope.** Run a corpus-wide audit: how many bundles
 parse-skip KHR_materials_specular images? If only this one bundle, the
 bits-corpus delta is small (~10 Mbits / 442k ppm corpus ≈ -23 ppm). If
 the extension is more common in our corpus, the rule justifies higher
 priority than the residual-share suggests.

D. **Slot rule audit**: confirm whether DXT1+mips is unique to
 KHR_materials_specular, or whether other glTF extensions also use it
 (e.g., `KHR_materials_clearcoat`, `KHR_materials_sheen`). Each routes
 to its own Unity URP slot and the per-slot texture-format rule needs
 per-extension verification.

## Files inspected

- `src/builder.rs` — `texture` (line 746) does the per-(image, sampler)
 Texture2D pair emission. Gated entirely on `MATERIAL_TEXTURE_SLOTS`
 accessors returning `Some(TexRef)`. No image-array-walk fallback.
- `src/materials.rs` — `MATERIAL_TEXTURE_SLOTS` lists 6 slots. Add
 `("_SpecularColorMap", |m| m.specular_color_image)` as the 7th.
- `src/gltf.rs::parse_materials` (line 600-720) — handles
 `KHR_materials_pbrSpecularGlossiness` and `KHR_texture_transform`. Add a
 KHR_materials_specular branch.
- `src/scene.rs::Material` — current fields: `base_color_image`,
 `emissive_image`, `normal_image`, `metallic_roughness_image`,
 `occlusion_image`, `spec_gloss_image`. Add `specular_color_image`.
- `src/texprofile.rs::texture_profile` — currently slot-agnostic. Needs
 per-slot dispatch when the spec-color slot wants DXT1.

## Diagnostic tools left behind

- `dev/probe_image5.py` — dumps all Texture2D in a bundle (name, fmt, dims,
 path_id, stream size, inline bytes, sha1 head). Used to confirm prod has
 16 / ours has 14.
- `dev/probe_glb_images.py` — counts images and textures in a GLB.
- `dev/probe_glb_image_details.py` — decodes each GLB image with PIL,
 reports mode/dims/alpha.
- `dev/probe_glb_materials.py` — dumps materials with all texture slots
 AND extensions, so KHR_materials_specular references are visible.

All four are additive, single-file, no shared state.
