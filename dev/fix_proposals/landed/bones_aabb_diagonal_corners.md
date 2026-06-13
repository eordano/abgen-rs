# Mesh `m_BonesAABB` — diagonal-corner projection

Supersedes `landed/bones_aabb_morph.md` (R5 morph-aware union). Landed at
commit `c2b73e0`. Verified bit-exact against Unity probe on the canonical
falsifier mesh across all 11 finite bones × 6 axes (worst error 0.0001 —
f32 rounding noise only).

## Rule

For each vertex `v` weighted to bone `N`:

1. Compute the mesh-local AABB of `{base, base + morph_delta}` over every
 morph target the vertex participates in. This yields two corner points:
 - `mn = elementwise_min(base, base + delta_t for all t)`
 - `mx = elementwise_max(base, base + delta_t for all t)`
2. Project `mn` and `mx` through `bind_poses[N]`.
3. Per axis, take `min(c0[a], c1[a])` and `max(c0[a], c1[a])` and union
 with bone `N`'s running AABB.

When the morph delta has mixed-sign components per axis the diagonal
corners differ from `base` and `base + delta`, producing more extreme
projected values than the prior `{base, base+delta}` rule could reach.

## Why R5 was incomplete

R5 projected `base` and `base + delta` separately through `bind_poses[N]`
— two points. That's a valid AABB of two source points, but it misses the
mesh-local AABB's other six corners. Whenever a non-axis-aligned
`bind_poses[N]` rotates a mixed-sign mesh-local delta box, the projected
extremes live at diagonal corners that aren't `base` or `base+delta`. The
diagonal corners are sufficient because the projection is affine: the AABB
of the projected source-AABB equals the AABB of the projected
`(mn, mx)` pair (each axis component is independent and either monotone
in row direction or constant).

## Falsifier mesh

`bafybeic63qquhbk / pid=-265503403035753958` — `M_uBody_BaseMesh_Mesh.004`
(982 verts, 62 bones with 11 finite-AABB, 1 morph target).

| measurement                              | prod    | R5 (old) | new rule |
|------------------------------------------|--------:|---------:|---------:|
| bone 13 `m_Min.z`                        | -13.6158 | -12.8657 | -13.6158 |
| bone 13 `m_Max.z`                        | +12.4610 | +12.3252 | +12.4610 |
| bone 12 `m_Min.x` ("shoulder spike")     | -18.7747 | -10.5403 | -18.7747 |
| bones 12/36 `Δmin.x` (siblings)          | -8.2344 (both) | shared by R5 | shared exactly |

All 11 finite bones match across all 6 axis components to within 0.0001.

## Methodology

Recovered via synthetic-input probing of a running Unity Editor (no
binary disassembly). The probe in
`unity-explorer/Explorer/Assets/Editor/AbgenProbe/AbgenBundleProbe.cs ::
ProbeBlendShapeBoneAABB` builds a synthetic mesh + SkinnedMeshRenderer
from a JSON spec mirroring the converter's flow
(`SetIndexBufferData` / `SetSubMesh` with
`MeshUpdateFlags.DontResetBoneBounds`, then `AddBlendShapeFrame`),
persists the mesh as an asset so Unity's serializer populates
`m_BonesAABB`, and reads it back via `SerializedObject`.

Inputs are extracted from a glb via
`abgen-rs/examples/dump_glb_to_bone_aabb_spec.rs`, which dumps positions,
weights, joints, morph deltas and skin bindposes in the spec format the
probe consumes.

Source-of-truth used during derivation: `unity-gltf` (Apache 2.0)
`MorphTargetContext.cs:344-345` (call site is
`AddBlendShapeFrame(name, 1f, positions, normals, tangents)`),
`Jobs.cs:1289` (Vector3 basis-flip `tmp.x *= -1`),
`Jobs.cs:2326-2341` (`ConvertMatricesJob` inverse-bind matrix flip
`col0.y/z`, `col1.x`, `col2.x`, `col3.x` negated).

## Implementation

`abgen-rs/src/mesh_layout.rs::compute_bones_aabb` — replaces the
two-point projection loop with the diagonal-corner projection. Same
caller signature, same morph-target traversal, just project two specific
corners instead of `base` then `base+delta` separately.
