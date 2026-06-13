# Material spec-gloss workflow — landed

> **Status: LANDED** in commit `402c533` (the source diff for
> `src/scene.rs`, `src/gltf.rs`, `src/materials.rs` was bundled under
> that commit's hunk despite the title describing a sibling
> AssetBundle fix — the spec-gloss code is the body of those three
> file changes).

Closes the `KHR_materials_pbrSpecularGlossiness` workflow on the
windows v10 reference set. URP's URPLitGUI accepts the legacy
spec-gloss extension as an alternate base-color + specular workflow;
`abgen-rs` was ignoring the extension block entirely and the
metal-rough pbr fields stayed default-zero on every spec-gloss
material in the corpus.

## Corpus impact

`dev/per_class_paired_only.py` against
`workdir/pathid_rt_v10_windows`, 2,173 paired bundles, isolated
binaries built off the same source plus/minus the spec-gloss patch:

| Material metric          | Before |  After |    Δ |
|--------------------------|-------:|-------:|-----:|
| paired-ppm               |    652 |    120 | -532 |
| paired-diff bits         | 31,971 |  5,903 | -26,068 |
| paired-diff objects      |    317 |    297 |  -20 |

The 8 ~3,500-bit residuals called out in the previous landed
material write-up were a subset of these 20 closed materials (the
other ~12 are smaller per-field diffs on the same 6 spec-gloss
bundles).

## Affected bundles (6, all from the same content roots)

| Bundle cid (short)  | Entity (short)      | Source glb/gltf                |
|---------------------|---------------------|--------------------------------|
| `bafybeictj33t2o…` | `bafkreiarheqgyo…` | `assets/scene/models/picnic.glb` (4 mats) |
| `bafybeidr666w4l…` | `bafkreiauwoojhf…` | `models/stage_wedge_game_asset.glb` (2 mats) |
| `bafkreibs2hhdhm…` | `bafkreibwoex6cy…` | `models/hourglass/scene.gltf` (2 mats, no tex) |
| `bafybeigwnrp6gv…` | `bafkreibwoex6cy…` | `models/crow.glb` (2 mats) |
| `bafkreidjcf5ek2…` | `bafkreifxfjzqrz…` | `45623029-…/scene.gltf` (1 mat) |
| `bafybeihkslk2vj…` | `bafkreifxfjzqrz…` | `131016c6-…/magic_ring_-_yellow.glb` (1 mat) |

The 10 `Qm…/unity_assets/s0_dummy_NN.gltf` bundles also carry the
extension but with `baseColorTexture.index = -1` placeholder; their
material classes don't enter the paired-diff metric.

## Witness — picnic.glb material_2 ("cake")

glTF source material[2]:

```json
{
 "name": "cake",
 "extensions": {
    "KHR_materials_pbrSpecularGlossiness": {
      "diffuseFactor": [1.0, 1.0, 1.0, 1.0],
      "diffuseTexture": {"index": 6},
      "glossinessFactor": 1.0,
      "specularFactor": [1.0, 1.0, 1.0],
      "specularGlossinessTexture": {"index": 7}
    }
 },
 "normalTexture": {"index": 8}
}
```

Prod m_SavedProperties → ours after fix (relevant slots only):

```
TexEnv[_BaseMap] pid != 0 — diffuseTexture
TexEnv[_BumpMap] pid != 0 — normalTexture
TexEnv[_SpecGlossMap] pid != 0 — specularGlossinessTexture
Float[_Glossiness] = 1.0 — glossinessFactor (f32)
Float[_GlossMapScale] = 1.0 — same value
Float[_Metallic] = 0.0 — pinned for spec-gloss
Float[_Smoothness] = 0.5 — URPLitGUI spec-gloss-tab default
Color[_BaseColor] = sRGB(diffuse) — same path as metal-rough
Color[_SpecColor] = (1,1,1,1) — specularFactor (linear, alpha 1)
m_InvalidKeywords contains "_SPECGLOSSMAP"
```

## Implementation

1. **`src/scene.rs`** — `Material` gains four fields:
 `uses_spec_gloss`, `spec_gloss_image`, `specular_factor`,
 `glossiness_factor`. When the extension is present
 `base_color`/`base_color_image` are populated from
 `diffuseFactor`/`diffuseTexture`, NOT from
 `pbrMetallicRoughness.baseColorFactor`/`baseColorTexture` — the
 metal-rough block is dropped from base-color routing (mirrors how
 prod's URPLitGUI handles the extension).

2. **`src/gltf.rs::parse`** — reads
 `extensions.KHR_materials_pbrSpecularGlossiness.{diffuseFactor,
 diffuseTexture, specularFactor, glossinessFactor,
 specularGlossinessTexture}` once per material. The `tex_ref`
 helper is reused for the diffuse + spec-gloss texture infos, so
 cross-bundle PPtrs, sampler dedup, and `KHR_texture_transform` on
 the spec-gloss slot all keep working unchanged. Adds the
 `spec_gloss_image` to the `other_uses` set so its source image
 doesn't end up classified as normal-only by the colorspace
 classifier.

3. **`src/materials.rs`**:
 - `MATERIAL_TEXTURE_SLOTS` adds `_SpecGlossMap` as a sixth slot so
     `Builder::material` allocates a Texture2D for the spec-gloss tex
     and threads its PathID through the same `tex_pid` map as the
     other slots.
 - `material_slot_images` adds the same slot for the colorspace
     classifier (LINEAR_SLOTS already listed `_SpecGlossMap`).
 - `material_keywords` takes a new `has_spec_gloss_map` bool that
     appends `_SPECGLOSSMAP` to `m_InvalidKeywords` (preserving the
     alphabetic sort).
 - `build_material_tree` branches on `uses_spec_gloss`:
     - sets `_Metallic`=0, `_Smoothness`=0.5 (URPLitGUI's spec-gloss-
       tab default, overriding the template's `material_0` `_Smoothness`=0)
     - mirrors `glossinessFactor` (f32) into both `_Glossiness` and
       `_GlossMapScale` (prod sets them to the same value)
     - writes `specularFactor` into `_SpecColor` (linear, alpha 1.0;
       no gamma — verified against magic_ring's spec=0.0504954268)
     - skips the metal-rough branch (`_Metallic = m.metallic`,
       `_Smoothness = 1 - roughness` etc.)

## Remaining (non-spec-gloss) diffs on the same 6 bundles

Two material diffs persist after this patch — both outside
spec-gloss scope, tracked separately:

- `45623029-…/scene.gltf` material_0: `_BaseMap` / `_EmissionMap`
 carry the right `(FileID, PathID)` pairs but with the FileIDs
 swapped (2 ↔ 3) — cross-bundle PPtr externals-ordering bug; the
 resolver does pick the right standalone bundles, only the FileID
 slot assignment is permuted.
- `magic_ring_-_yellow.glb` material_0: missing `_EMISSION` keyword
 when `emissiveFactor = 0.001` < `EMISSION_LINEAR_THRESHOLD`
 (`0.5/255 ≈ 0.00196`). The prior threshold was tuned to
 suppress `5.96e-8` / `9.18e-11` authoring noise, which collaterally
 drops `0.001` here. Separate emission-threshold investigation.

## Verification

```
cargo test --release --lib # 118 passed
cargo test --release --test parity_bytes # Total bits-different 773032
                                         # (== MAX_BITS_DIFFERENT ceiling, unchanged)
```

The `tests/parity_bytes.rs` fixture set doesn't include a
spec-gloss source — its corpus is 4 standalone textures + 1 plain
glb with metal-rough materials — so the test ceiling is unaffected
by the spec-gloss path.
