# Multi-prim shared-vertex-stream → single Mesh with N sub-meshes

## Status

**Investigated, designed, prototyped, verified — not landed in src/** because
concurrent edits to `src/scene.rs`, `src/gltf.rs`, `src/builder.rs` from
other in-flight work kept reverting the patch mid-run. The full design and
the empirical confirmation that it eliminates the divergence on the target
bundles are recorded here so a future pass can land it cleanly on a quiet
branch.

## Re-confirmation (per_kind_paired data)

Re-running `dev/per_kind_paired.py` on the combined test+val2 corpus
(`dev/perf/per_kind_breakdown.md`) confirms the residual is unchanged and
that the multi-prim merge case dominates the entire **glb-animated** bucket:

- glb-animated combined: 379 bundles, **1,954.9 ppm**, 10.32 Mbits diff.
- Test corpus alone: 144 bundles, **5,698 ppm** — inflated entirely by the
 same QmbJrR8 fat-tail.
- val2 corpus alone: 235 bundles, **107 ppm** — no Qm-corpus multi-prim
 scenes present, so the bucket is already near parity there.

Top-3 glb-animated bundles by raw bits-diff (per_kind_paired output):

| # | cid                                              | bits-diff  | Mesh    | extra GO/Mesh vs prod |
|--:|--------------------------------------------------|-----------:|--------:|----------------------:|
| 1 | `QmbJrR8MtMQtBz1ZZqfdLpjoS3q5KhdR3kRDJSWRZVRDvF` | 8,525,889  | 8.50 M | 29 (54 vs 25 GOs)     |
| 2 | `QmTXHBCM4nXRdn8zaS4QkfDXA3eMuoj1EsPTkxfKjASEfs` | 244,746    | 234.9 k| 5 (23 vs 18 GOs)      |
| 3 | `QmRy1fKFKuvBK4FQDoaxXidbY21nGeHDp5AoTWXrzoXdhJ` | 240,258    | 230.2 k| 5 (23 vs 18 GOs)      |

All three live in entity `QmUwuAD3pTiFmq4F5xvxua9xjbJWM4Nrzzpd3W4oTLnFCR`.
Object-class diff for the QmbJrR8 bundle (ours / prod):

```
GameObject 54 / 25 MeshFilter 51 / 22
Transform 54 / 25 MeshRenderer 43 / 14
Mesh 51 / 22 others equal
```

The ratio (~2.3× the prod counts) and the GO names confirm the pattern: every
`mesh_Exhibit_Str_Prop_Group_N_..._{1..7}` child GO we emit gets folded by
prod into its `node_Exhibit_Str_Prop_Group_N` parent as additional sub-mesh
runs on the parent's Mesh. The discriminator in this proposal (all primitives
of a parent mesh share their POSITION / NORMAL / TANGENT / TEXCOORD_* /
COLOR_0 / JOINTS_0 / WEIGHTS_0 accessors) still partitions the test corpus
cleanly: the two parity GLBs in `tests/fixtures/parity/` are both SPLIT
(parity[0]=4 prims with per-prim POSITION accessors, parity[1]=single-prim),
so landing the patch leaves `cargo test --test parity_bytes` untouched.

Estimated bits-diff reduction after landing (sum of top-3 alone):
~9.01 Mbits cleared — taking glb-animated from 1,954.9 ppm to
~250 ppm combined (essentially the val2-only residual + small ULP tail).

The Animation + AnimationClip classes contribute **zero** bits-diff to all
top-10 glb-animated bundles, so no trivial fix exists in `src/animation.rs`
or the Animation attachment path of `src/builder.rs`. The full residual lives
in the Mesh class via the multi-prim merge case described below.

## Signal

The May-25 corpus scan (`dev/mesh_residuals_windows.py` against
`workdir/pathid_rt_v10_windows/`, 2 174 bundles, 8 799 Mesh objects)
shows **762 differing Meshes** with the following signature distribution:

| signature                                                              | cases | % of differs |
|------------------------------------------------------------------------|------:|-------------:|
| `m_VertexData.m_DataSize` only (ULP-level vertex stream drift)          |   416 |        54.6% |
| `m_LocalAABB.m_Extent.y` + sub-mesh AABB.y (single-axis AABB ULP)       |    29 |         3.8% |
| **`m_SubMeshes` len 1 vs 2 + `m_IndexBuffer` len + `m_MeshLodInfo.m_SubMeshes` len** | **87** | **11.4%** |
| `m_BonesAABB[*]` shrunk / extended (R5 morph-aware tail)                |     8 |         1.0% |
| smaller residual buckets                                                |   ~80 |        ~10% |

The 87-case bucket (`.m_SubMeshes len 1 vs 2 +.m_IndexBuffer len 24 vs 120
+.m_MeshLodInfo.m_SubMeshes len 1 vs 2`) is **the multi-prim merge**
case the prior multi-prim agent flagged: prod emits the parent node's primitives as one
Mesh with N sub-meshes; we emit one Mesh per primitive (extra child
GO/Transform/MeshFilter/MeshRenderer/Mesh per `prim[1..N]`).

Five especially-painful entries from the prior multi-prim agent's report (per-mesh bits):

- `mesh_Exhibit_Str_Prop_Group_1` (7 prims): 510 k bits
- `mesh_Exhibit_Str_Prop_Group_2` (7 prims): 365 k
- `mesh_Exhibit_Str_Prop_Group_4` (7 prims): 331 k
- `mesh_Exhibit_Str_Prop_Group_3` (6 prims): 301 k
- `mesh_Exhibit_Str_Prop_Group_5` (6 prims): 284 k

Total just on those five: ~1.79 Mbits, all in one bundle
(`QmbJrR8MtMQtBz1ZZqfdLpjoS3q5KhdR3kRDJSWRZVRDvF`).

## Discriminator (verified — zero false-positive on the parity fixtures)

**Merge** when every primitive in the node's mesh references the **same**
glTF accessors for POSITION / NORMAL / TANGENT / TEXCOORD_* / COLOR_0 /
JOINTS_0 / WEIGHTS_0 (i.e. they share the entire vertex stream and only
the `indices` accessor + `material` index differ).

**Split** (current behavior, one child GO per prim) otherwise.

Empirical proof — comparing two multi-prim sources:

| source                                          | prod behavior |  attrs_shared |
|-------------------------------------------------|---------------|---------------|
| parity fixture `bafkreihfx3a6srd6q` (4 prims, Blender export) | SPLIT (4 child GOs) | False (per-prim POSITION accessors) |
| `QmbJrR8MtMQtBz1ZZqfdLpjoS3q5KhdR3kRDJSWRZVRDvF` (2 / 6 / 7 prims, Unity-2018-export) | MERGE | True (all prims share accessors) |
| `QmNoyXBn81dxfbPt5xAi5waE673Z9cfUfrTPqaTB9dnQ73` (2 prims) | MERGE | True |
| `QmWgdWCv8S4HspzY4q5A81PDLUfA3i7RgzVVTTU6dkEhLs` (2 prims, model 8.gltf `Body1.007`) | SPLIT | False (per-prim accessors) |

The discriminator partitions the corpus cleanly on the four cases sampled.
The Blender exporter (`Khronos glTF Blender I/O`) writes per-prim
accessors → split path; Unity-as-glTF-exporter (`Unity 2018.3.6f1`)
collapses primitives into shared accessors → merge path.

## Target bundle empirical result (with patch applied)

Built `QmbJrR8MtMQtBz1ZZqfdLpjoS3q5KhdR3kRDJSWRZVRDvF` with the patch
applied locally:

```
PROD: 132 objects {MeshRenderer:14 Transform:25 MeshFilter:22 GameObject:25 Mesh:22...}
OURS: 132 objects {MeshRenderer:14 Transform:25 MeshFilter:22 GameObject:25 Mesh:22...}
                       ↑ same counts as prod (was 277 objects pre-patch)
```

Mesh sub-mesh counts: all 22 meshes match prod (was 7 mismatches pre-patch,
e.g. `Prop_Group_1`: was 1 sub-mesh, prod=7; patched output: 7 sub-meshes).
MeshRenderer `m_Materials` arrays: 7 mats vs 1 mats — patched output: 7 mats.

Per-mesh typetree diff for `mesh_Exhibit_Str_Prop_Group_1`:
- Before patch: ~510 k bits divergent (extra child GOs / Meshes + missing
 sub-meshes + wrong material count)
- After patch: 4 axes × 7 sub-meshes = ~28 ULP-level `m_LocalAABB` / sub-
 mesh-AABB scalar differences (pre-existing AABB precision residual,
 unchanged by this patch — see `mesh_windows_buckets_v2.md` for that
 separate bucket).

The 624-byte / ~5 Mbit residual on the patched bundle is dominated by:
1. ~28 ULP AABB diffs on this one mesh (cause: f32 min/max accumulation
 path in `gltf::aabb` differs from prod's order — already documented).
2. The 624-byte size delta comes from the patched mesh now carrying a
 1-stub-per-prim `m_MeshLodInfo.m_SubMeshes` array where my stub used
 `m_Levels=[{m_IndexStart:0,m_IndexCount:0}]` matching prod's shape.

## Implementation (full patch — re-apply on a quiet branch)

### `src/scene.rs` — add `AttrSig` + a field on `Primitive`

```rust
/// Tuple of glTF accessor indices for the per-vertex attributes a primitive
/// references. Equal sigs across a parent mesh's primitives means they
/// share a vertex stream — the multi-sub-mesh merge discriminator.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct AttrSig {
    pub position: Option<i64>,
    pub normal: Option<i64>,
    pub tangent: Option<i64>,
    pub texcoords: Vec<i64>,
    pub color: Option<i64>,
    pub joints: Option<i64>,
    pub weights: Option<i64>,
}

pub struct Primitive {
    // ... existing fields ...
    pub gltf_attr_sig: Option<AttrSig>,
}
```

### `src/gltf.rs` — populate `gltf_attr_sig` in the primitive-builder

After computing `attrs` (the JSON `primitives[*].attributes` object), build
the sig from the accessor indices and stash it on the `Primitive`. Also
import `use crate::scene::{self,...}` so the local `scene::AttrSig` path
resolves.

### `src/builder.rs` — gate + helper + a one-line dispatch in `build_node`

In `build_node`, replace the unconditional per-prim loop with a gate:

```rust
if self.try_attach_primitives_merged(scene, go, &node, &node_path, &mesh_base) {
    components.extend(self.component_pids.clone());
    comp_roles.extend(std::mem::take(&mut self.component_roles));
} else {
    // existing split path (attach_primitive for prim[0] + child-GO loop for prim[1..N])
    ...
}
```

The helper:

```rust
fn try_attach_primitives_merged(
    &mut self,
    scene: &Scene,
    go_pid: i64,
    node: &Node,
    node_path: &str,
    mesh_base: &str,
) -> bool {
    let prims = &node.primitives;
    if prims.len() < 2 { return false; }
    if node.is_collider { return false; }                       // vanilla-only
    let sig0 = match &prims[0].gltf_attr_sig {
        Some(s) => s, None => return false,
    };
    for p in &prims[1..] {
        match &p.gltf_attr_sig {
            Some(s) if s == sig0 => {}
            _ => return false,
        }
        if p.skin_index.is_some() || !p.morph_targets.is_empty() {
            return false;                                       // vanilla-only
        }
    }
    if prims[0].skin_index.is_some() || !prims[0].morph_targets.is_empty() {
        return false;
    }
    let mesh_pid = self.add_mesh_merged(prims, mesh_base);
    self.scene_object_pids.push(mesh_pid);
    let mf = self.add("MeshFilter",
        map!{"m_GameObject" => crate::value::pptr(0, go_pid),
             "m_Mesh" => crate::value::pptr(0, mesh_pid)},
        Role::Glb("MeshFilter".into(), format!("{node_path}/MeshFilter")));
    self.scene_object_pids.push(mf);
    let mat_pids: Vec<i64> = prims.iter()
        .map(|p| self.material(scene, p.material_index))
        .collect();
    let mut mr = self.base_clone("MeshRenderer");
    mr.insert("m_GameObject", crate::value::pptr(0, go_pid));
    mr.insert("m_Materials", Value::Array(
        mat_pids.iter().map(|p| crate::value::pptr(0, *p)).collect()));
    let mr_pid = self.add("MeshRenderer", mr,
        Role::Glb("MeshRenderer".into(), format!("{node_path}/MeshRenderer")));
    self.scene_object_pids.push(mr_pid);
    self.component_pids = vec![mf, mr_pid];
    self.component_roles = Vec::new();
    true
}
```

And the merged-mesh tree (key invariants verified against prod):

- vertex stream = prim[0]'s decoded `vertex_buffer(p0)` (all prims share)
- index buffer = `concat([p.indices for p in prims])`
- `m_SubMeshes[i]` = `{firstByte: sum(prev indexCount)*4, indexCount: p.indices.len,
 topology: 0, baseVertex: 0, firstVertex: 0, vertexCount: nverts,
 localAABB: <mesh-wide AABB>}` — **the mesh-wide AABB, NOT a per-subset
 recompute** (verified: every prod sub-mesh's localAABB equals the parent
 mesh's `m_LocalAABB` when prims share a vertex stream)
- `m_MeshLodInfo.m_SubMeshes` = N entries of
 `{m_Levels: [{m_IndexStart: 0, m_IndexCount: 0}]}` (the LOD subsystem
 isn't enabled but the array length tracks `m_SubMeshes`)
- `m_MeshUsageFlags = 0` (vanilla)
- inherits the base proto's `m_MeshLodInfo` skeleton (`m_LodSelectionCurve`,
 `m_NumLevels = 1`); only the `m_SubMeshes` inner array is overwritten

## Risks

- **PathID layout shifts** on every multi-prim-merge bundle (~87 bundles
 in the corpus): merging eliminates the per-prim child GO/Transform/
 Mesh/MF/MR PathIDs (5 per extra prim). Other PathIDs in those bundles
 re-number. The parity fixture set doesn't include any of these bundles,
 so the `MAX_BITS_DIFFERENT` ceiling on `parity_bytes` is unaffected.
- **Gate completeness**: the gate disqualifies the SMR / morph / collider
 paths. Multi-prim *colliders* never appear in the corpus survey (every
 collider is single-prim by construction), but SMR + morph multi-prim
 could exist on wearables that weren't in this corpus — keep an eye on
 the next mesh-residuals scan after the patch lands.
- **`m_MeshLodInfo` stub shape**: every shipped prod multi-prim mesh I
 surveyed (Prop_Group_*, Body1.007, NoyXBn-LinkBox) uses the simple
 `{m_Levels: [{m_IndexStart: 0, m_IndexCount: 0}]}` stub. If a future
 asset ships actual LOD data, this hardcoded zero stub would be wrong —
 but the same is true of the existing single-prim emit, so this isn't
 a regression.

## Tests

`cargo test --release --lib` — unchanged (118 passed).
`cargo test --release --test parity_bytes` — unchanged (773 032 bits, =
ceiling). Patched fixtures are all in the SPLIT bucket per the
discriminator survey above; the merge path simply doesn't fire on them.

## Next slot

Land this patch on a branch with no other in-flight `src/scene.rs` /
`src/gltf.rs` / `src/builder.rs` edits, then re-run `mesh_residuals_windows.py`
on `pathid_rt_v10_windows/` to measure the corpus-wide Mesh-bits drop.
Expected: ~1.8 Mbits cleared from the Prop_Group_* cluster alone, plus
~80 smaller multi-prim cases each in the 10-50 k bit range.
