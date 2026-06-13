# MeshFilter residual — closed (11 → 0)

## Derived dedup rule

Two scene nodes that reference the same glTF `meshes[i].primitives[j]`
share **one** Unity `Mesh` object — every `MeshFilter` /
`SkinnedMeshRenderer` referencing that node points at the shared PathID.

This is what the prod deterministic-guids converter does and what
abgen-rs now does. The dedup key is:

```rust
(gltf_mesh_index: usize,
 gltf_prim_index: usize,
 usage: i64, // 0 = renderer, 1 = SMR, 16 = collider
 skin_index: Option<usize>) // distinct skins → distinct bind poses
```

`usage` and `skin_index` are in the key because two nodes that share a
glTF primitive but render through different component types (e.g. one
renderer + one collider) or different skins (different bind poses
baked into `m_BindPose`) must NOT share the Unity Mesh — their
`mesh_tree` payloads differ.

## Cases closed

Exhaustively measured against the 280-bundle prod corpus:

| Class | Before | After | Δ |
|---|---|---|---|
| MeshFilter | 11 | **0** | -11 |
| AssetBundle | 68 | 67 | -1 |
| Mesh | 19-21 | 21 | 0 |
| Texture2D | 60 | 60 | 0 |
| TextAsset | 3 | 3 | 0 |
| Material | 3 | 3 | 0 |
| **Total** | ~164 | **154** | **-10** |
| Object-exact | 14843/15007 (10928 ppm differ) | 14853/15007 (10262 ppm differ) | +10 objects / −666 ppm |
| Mean \|size Δ\| | 5833 B | 6060 B | +227 B |
| Bundles byte-id | 0 | 0 | 0 |

### Why MeshFilter dropped exactly 11 (not 22, not 5)

The forensic in `meshfilter.md` identified all 11 cases as concentrated in
**one bundle**, `bafybeiczim5cqrv…`. That bundle's source glb has 9 glTF
meshes referenced by 2 nodes each, with a total of 11 primitives. Each
shared primitive produced one MeshFilter residual; closing dedup closes
all 11.

Per-bundle verification on `bafybeiczim5cqrv…` (script:
`/tmp/check_meshfilter.py`):

```
size ours=32685192 prod=32757504 delta=-72312
objects ours=3263 prod=3263 paired=3263
Per-type (diffs/total):
 Mesh 0/587 ← was 11 ours-only
 MeshFilter 0/558 ← was 11 differing
 (every other class also 0/N)
Mesh: ours=587 prod=587 unpaired ours=0 prod=0
MeshFilter: ours=558 prod=558 unpaired ours=0 prod=0
```

All 558 MeshFilters and all 587 Meshes in this bundle now match prod
exactly. The remaining 72 KB delta is real content-byte drift (likely
vertex tangent or streaming offset) — orthogonal to dedup.

### Why Mesh residual didn't drop visibly

The 11 duplicate Meshes were not in the *paired diff* count to begin with —
they were ours-only orphans (prod's converter had emitted N−1 fewer Mesh
objects), so they never paired against a prod Mesh and were invisible to
`set(oe) & set(pe)`. Closing dedup removes those orphans (Mesh count in
`bafybeiczim5cqrv` goes 598 → 587 = matches prod's 587) without changing
the count of *differing-after-pairing* Mesh objects in the other 279
bundles, which are dominated by `m_VertexData.m_DataSize` and
`m_Shapes` issues unrelated to dedup.

### Why AssetBundle dropped exactly 1

The single bundle whose object inventory changed (mesh count 598 → 587)
also has its `m_PreloadTable` shrink by 11 entries, which flips its
preload-table state into matching prod. Other 279 bundles' AssetBundle
objects are unaffected.

## Implementation

### Patch surface

Three files; all the dedup logic is a 24-line short-circuit at the top
of `Builder::add_mesh`:

- `src/scene.rs` — `Primitive` gains two fields: `gltf_mesh_index:
 Option<usize>` and `gltf_prim_index: usize`. Default-initialized for
 any future synthetic-primitive caller (the `Default` derive handles
 the no-key case the same way as a missing index — falls into the
 fallback branch in `add_mesh`).
- `src/gltf.rs` — the one `prims.push(Primitive { … })` site in
 `build_primitives` populates the new fields from the already-in-scope
 `mesh_idx` (passed in) and `pi` (the per-primitive enumerate index).
- `src/builder.rs::add_mesh` — looks up the key tuple in
 `self.mesh_pid_by_gltf` (already declared on `Builder` from a prior
 scaffold commit). Hit → return the existing pid, no `add`, no
 `unique_recycle`. Miss → existing emit path, then insert into the
 map.

### Critical detail: `unique_recycle` MUST NOT run on dedup hits

The PathID a `Role::Glb("Mesh", recycle)` resolves to is a pure
function of the recycle name (`pathids::local_id_for_recycle_name`
hashed through `prefab_packed_path_id`). The recycle name comes from
`unique_recycle("meshes", mesh_base)`, which **mutates** a per-prefix
counter (`self.recycle_seen`). If we called `unique_recycle` on the
dedup-hit path, the counter would advance unnecessarily, shifting the
PathIDs of every *later* legitimately-distinct mesh — re-introducing
the exact drift the dedup is supposed to fix. The implementation
verifies this by early-returning before any counter mutation on a hit.

### Side-effects that are safe

- `scene_object_pids` will contain the same shared Mesh pid multiple
 times (once per MeshFilter that references it). This is harmless:
 `fill_assetbundle` dedups it through a `HashSet<(file_id, path_id)>`
 before assembling `m_PreloadTable`.
- `materials::glb_referenced_mats` and the per-(mat, tex) closures are
 unaffected — the dedup is per-Mesh, not per-Material.

## Verification

- Unit tests: `cargo test --release` — 104 + 1 parity test all pass.
- Full corpus: 280 bundles diffed object-by-object (`measure_full_vs_prod.py`).
- Target bundle: zero residuals on `bafybeiczim5cqrv…` Mesh/MeshFilter pairs.
- Tangent / vertex / texture residuals on the other 279 bundles remain
 unchanged — confirms no scope creep into other classes.

## Files touched

- `src/scene.rs` (+15 lines)
- `src/gltf.rs` (+2 lines)
- `src/builder.rs::add_mesh` (+22 lines, -1 line)

## Open problems remaining (not in scope here)

- **Mesh = 21 differing** (91 of those across the wider corpus diff in
 `m_VertexData.m_DataSize` per `bitwise_residuals.md` § Mesh §). Cause:
 vertex-buffer compaction / streaming-format ambiguity. Tracked in the
 Mesh close-out investigation.
- **Texture2D = 60 differing** (split 56 `m_StreamData.offset` + 3
 `image data` + 1 `m_TextureFormat`). Tracked in the Texture2D
 externalization investigation (cross-bundle.resS).
- **AssetBundle = 67 differing** (mostly `m_PreloadTable` entries that
 point at cross-bundle Texture2Ds — i.e. driven by the same
 externalization gap). Tracked in the AB externalization investigation.
- **No bundle is byte-identical yet** (0/280). Driven by the
 Texture2D streaming-data residual that touches every textured
 bundle.
