# `multi_scene_emission` — emit every glTF scene, not only the default

**Landed:** `046b75c` (combined commit with other agent's CAB-ordering work;
multi-scene patch is the changes to `src/scene.rs`, `src/gltf.rs`, and the
`build_extra_scene` / `bundle_root_assigned` additions in `src/builder.rs`).

**Target:** the −15.6 MB structural deficit on val `glb-scene-collider`.

## Rule

The converter's glTF importer (`asset-bundle-converter/.../unity-gltf/Editor/Scripts/GltfImporter.cs`,
loop at lines 170–218) iterates `for (sceneIndex = 0; sceneIndex < SceneCount;
sceneIndex++)` and adds every scene's GameObject tree to the bundle via
`AddObjectToAsset(ctx, $"scenes/{sceneName}", sceneGo, gltfIcon)`. The default
scene becomes the bundle's main asset; the others are additional named root
GameObjects. Our `gltf::parse` was only emitting the **default** scene's roots,
discarding every other scene declared in the file.

## Implementation

- `Scene` (in `src/scene.rs`) gains an `extra_scenes: Vec<(Option<String>,
 Vec<usize>)>` field — name + root nodes for every non-default scene.
- `gltf::parse` populates `extra_scenes` from the input glTF.
- `Builder::build` iterates `scene.extra_scenes` after the default scene is
 fully wired (after animation/SMR resolution, before metadata text-asset
 and AssetBundle emission) and calls a new `build_extra_scene` helper for
 each.
- `build_extra_scene` mirrors the converter importer's `useFirstChild` decision:
 if the scene has a single root and the bundle has no animation component,
 drop the wrap GameObject and emit the root node directly at father=0;
 otherwise create a wrap GameObject named after the scene and parent the
 root nodes under it.
- `Builder.bundle_root_assigned: bool` gates the `root_hash`-rename inside
 `build_node` so it fires once (for the default scene) and never on the
 additional scenes' root nodes — those keep their source node names.

## Measurement (val corpus, )

| metric | before | after |
|---|---:|---:|
| glb-scene-collider bundles | 667 | 668 (1 reclassified) |
| glb-scene-collider total delta | −15,585,043 B (−5.26 %) | **−48,188 B (−0.02 %)** |
| smaller / larger / byte_id | 169 / 87 / 291 | 148 / 112 / **313** (+22) |
| single-scene bundles changed | n/a | **0 of 1,374** |

The −15.6 MB deficit was overwhelmingly concentrated in **8 bundles inside
one entity** (`QmVkQnvK4uFTzKUFFesbMu4DckQ6DzHuXFTtFhnFFa7rbw`). Their source
.glbs each declare 5–10 named scenes (`Scene` / `Static` / `Dynamic` / `Walls`
/ `Bosque` / `Static2..4` / `StaticFull`) and only ~5 % of the geometry lives
in the default scene. Top bundle:

```
delta=-2,234,239 ours=94,143 ref=2,328,382 QmNWECJjYZ… (10 scenes, default=Dynamic)
```

After the patch, those 8 bundles drop to small overshoots of +334 to +5,726
bytes (total ~+18 KB) — accounted for by 51 extra wrap-GO/Transform pairs
across 8 bundles. `parity_bytes` and `bc7_block_pinset` both stay clean. No
single-scene bundle in the 1,374-paired single-scene cohort changed by even
one byte.

## Per-class deficit on the 8 multi-scene bundles (post-fix vs ref)

| class | ref | ours | Δ |
|---|---:|---:|---:|
| GameObject | 6,424 | 6,475 | +51 |
| Transform | 6,424 | 6,475 | +51 |
| Material | 241 | 241 | 0 |
| MeshRenderer | 3,524 | 3,524 | 0 |
| MeshFilter | 4,501 | 4,501 | 0 |
| Mesh | 2,011 | 2,011 | 0 |
| MeshCollider | 984 | 977 | −7 |
| AssetBundle | 8 | 8 | 0 |

The +51 GO/Transform pair and −7 MeshCollider residuals are second-order
effects: the converter's importer drops the per-scene wrap GameObject when a scene
has exactly one root *and* no animation, while a handful of `_collider`-named
nodes inside those bundles are produced by the converter's `ColliderGenerator` post-
process on the imported prefab that we do not yet model on the extra-scene
trees. None of this is structural; each remaining bundle is within ±6 KB of
ref.

## What this does NOT close

The remaining −48 KB of glb-scene-collider deficit is BC7 long-tail spread
across ~150 single-scene bundles (already documented in
`bc7_residual_heatmap.md` and the BC7 mode-drill writeups). That is a
separate axis and out of scope for this rule.
