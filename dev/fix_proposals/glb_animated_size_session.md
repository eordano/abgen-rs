# glb-animated bundle SIZE — source decomposition (val300)

Diagnosis of the 76 non-zero `glb-animated` size deltas in
`/tmp/abgen-val300-integrated-report.json` (corpus
`/tmp/abgen-val300-integrated` vs ref
`unity-reference-ab/ad0564d-val300-windows`).

## TL;DR

The `glb-animated` size deltas decompose into **four** distinct sources. The
dominant **recoverable** size source is a **multi-primitive collider not being
merged** — a gap in the already-landed multi-prim sub-mesh merge
(`landed/multiprim_subm_merge.md`), whose gate explicitly bails on
`node.is_collider`. The Animation / AnimationClip and Mesh-vertex-stream
classes contribute **zero** structural size in this bucket (consistent with
the earlier multiprim finding). The big-byte tail is irreducible texture
encoder output.

Bucket counts (from the report): 33 @ 1–4B, 16 @ 5–64B, 14 @ 65–1k, 12 @
1k–64k, 1 @ >64k. Signed Σ = **+89,513 B** (ours larger), abs Σ = 425,639 B.

## Sources, by contribution

### 1. Multi-primitive COLLIDER split — RECOVERABLE, dominant structural source

When a glTF mesh on a `_collider` node has ≥2 primitives that **share their
entire vertex stream** (same POSITION/NORMAL/TANGENT/TEXCOORD/COLOR/JOINTS/
WEIGHTS accessors, only `indices` + `material` differ), GLTFast builds **one**
Unity Mesh with N sub-meshes, and the converter's `ColliderGenerator` /
`CustomGltfImporter.ConfigureColliders`
(`asset-bundle-converter/.../ColliderGenerator.cs:32`,
`Editor/CustomGltfImporter.cs:148`) adds a **single** `MeshCollider` on that
one merged mesh.

We instead **split** it: prim[0] → MeshCollider on the node, prim[1..N] → one
extra `{GameObject, Transform, MeshFilter, MeshCollider, Mesh}` set each. Cause:
`builder.rs::try_attach_primitives_merged` returns early on
`if node.is_collider { return false; }` (builder.rs:1191), forcing the split
path (builder.rs:1534–1563).

Evidence — `QmUwmgapjirBnBxdKeeyDvubTFucf95NHzmhdkpMMb4Zve/QmPTpG…_windows`
(Δ **+48,122**):
- objalign: OURS 183 objects vs REF 168 → **15 OURS!** (PathIDs only in ours),
  **0 REF!**. The 15 = 3 × {GameObject `mesh_Spare_Room_collider_…_{1,2,3}`,
  Transform, MeshFilter, MeshCollider, Mesh}.
- ref carries exactly **one** matched MeshCollider (PathID 9158757088…, 64B).
- source `.gltf` mesh `mesh_Spare_Room_collider` has **4 primitives all
  referencing POSITION 211 / NORMAL 212 / TEXCOORD_0 213 / TEXCOORD_1 214 /
  TANGENT 215** — shared stream → merge-eligible.
- 3 extra ~68 KB collider Mesh copies dominate (206,001 B extra raw object
  bytes → +48,122 B compressed).
- the node's parent Transform shows up as 104B (ours) vs 68B (ref) — the extra
  `m_Children` entries from the 3 split child GOs; disappears under merge.

The three collider-split bundles in this corpus:

| compressed Δ | +objects | extra colliders | extra raw obj bytes | bundle |
|-------------:|---------:|----------------:|--------------------:|--------|
| +48,122 | 15 | 3 | 206,001 | QmUwmg…/QmPTpG…_windows |
| +26,960 | 25 | 5 | 89,635  | QmRH9C…/QmZzGt…_windows |
| +11,203 |  5 | 1 | 21,939  | QmRH9C…/QmVYUD…_windows |
| **+86,285** | | | 317,575 | **total** |

All multi-prim collider meshes in these sources share accessors (verified:
`Portal_General_collider` 3 prims shared, `walls_collider` 4 prims shared,
`Spare_Room_collider` 4 prims shared) → all merge-eligible.

**+86,285 B = 96 % of the entire glb-animated signed Σ (+89,513).** This is the
single dominant recoverable size source in the bucket.

### 2. Extra Material (+1 object, 1264B each) — separate, already-known wall

Many small-to-mid deltas (+178, +210, +484, …) are `oc/rc = X+1/X` with a
single `OURS! Material` (1264B). Example
`bafkreidtxky…/bafkreiekk…_windows` Δ +484 → 1 extra Material. This is the
"structural +Material" wall the task notes as already-known — out of scope for
this SIZE pass beyond attribution. It accounts for the bulk of the 65–1k bucket
that is *not* compression noise.

### 3. Texture `.resS` encoder output — IRREDUCIBLE, the big-byte tail

The single >64k delta and several large-bundle deltas are texture-payload
compression, not mesh/anim structure:

- `bafkreigkfyk…/bafybeicd…_windows` Δ **+168,020** (largest): objalign shows
  **99 = 99 objects, every paired object identical size** (only the AssetBundle
  name-list differs by 12B; 26 Texture2D all DIFF = BC7 texel noise, excluded).
  The serialized CAB is 178,648 vs 178,632 B (≈identical); the entire raw
  difference (699,056 B) lives in the `.resS` streaming-texture file. Texture2D
  headers are byte-identical (208B → same w/h/format/mipcount/StreamingInfo) yet
  `.resS` raw length differs → texture-encoder (BC7/crunch) output length, the
  existing `crunch_encoder` / `texture2d_*` wall. Irreducible under clean-room
  discipline.
- `bafkreicrwknc…` family (Δ −32,959 / −32,490 / −31,743 / −28,770): `.resS`
  raw length is **byte-identical** (55,359,680 both sides); the only structural
  object is 1 extra Material (source #2). The remaining compressed delta is
  pure LZ4 block framing over BC7 texel noise. Irreducible.

### 4. PathID-renumber-only + LZ4 framing noise — IRREDUCIBLE, the small tail

The 33 @ 1–4B and most of 5–64B are bundles where `oc == rc` (equal object
counts) — often `OURS!=N REF!=N` with the same N (e.g. 37/37 OURS!=25 REF!=25):
PathID values differ between sides but the object set is structurally identical.
The byte deltas are LZ4 framing / ULP-level vertex-stream drift, already covered
by `mesh_windows_*` / `sf_pad_lz4_noise`. Irreducible.

## Recoverable vs irreducible (this corpus)

| source | bundles | signed Δ | status |
|--------|--------:|---------:|--------|
| multi-prim collider split | 3 | **+86,285** | **RECOVERABLE** (fix below) |
| extra Material | ~12 | ~+2.5k | separate known wall |
| texture `.resS` encoder | ~6 | mixed (±, big) | irreducible (BC7/crunch) |
| PathID-renumber + LZ4 noise | ~55 | ±small | irreducible |

The Animation, AnimationClip, MeshFilter/MeshRenderer, and Mesh-vertex-stream
classes contribute **no** recoverable structural size in glb-animated —
confirming the earlier multiprim conclusion that the residual is the Mesh class
via the merge case (here, the collider variant of it).

## Fix proposal — extend the multi-prim merge to colliders

Lift the blanket `if node.is_collider { return false; }` bail in
`try_attach_primitives_merged` and add a collider-aware merged-emit path. A
merged collider must emit **{merged Mesh (usage 16/36), MeshFilter,
MeshCollider}** — **no MeshRenderer** (the current merged path at
builder.rs:1223–1234 always emits a MeshRenderer, which is correct for renderers
but wrong for colliders).

Sketch:

```rust
// builder.rs::try_attach_primitives_merged
// replace `if node.is_collider { return false; }` with a branch after the
// shared-accessor / no-skin / no-morph gate passes:
if node.is_collider {
    // !becomes_smr is guaranteed (gate rejects skin/morph)
    let collider_baked = scene.materials.iter().any(|m| m.alpha_mode == "MASK")
        && scene.materials.iter().any(|m| m.uses_emissive_strength);
    let usage = if collider_baked { 36 } else { 16 };
    let mesh_pid = self.add_mesh_merged_with_usage(prims, mesh_base, usage);
    self.scene_object_pids.push(mesh_pid);
    // orphan each prim's material (collider meshes drop their material)
    for p in prims { if p.material_index.is_some() {
        let _ = self.material_orphan(scene, p.material_index); } }
    let mf = self.add("MeshFilter",
        map!{"m_GameObject" => pptr(0, go_pid), "m_Mesh" => pptr(0, mesh_pid)},
        Role::Glb("MeshFilter".into(), format!("{node_path}/MeshFilter")));
    let mut mc = self.base_clone("MeshCollider");
    mc.insert("m_GameObject", pptr(0, go_pid));
    mc.insert("m_Mesh", pptr(0, mesh_pid));
    let mc_pid = self.add("MeshCollider", mc,
        Role::Glb("MeshCollider".into(), format!("{node_path}/MeshCollider")));
    self.scene_object_pids.push(mf);
    self.scene_object_pids.push(mc_pid);
    self.component_pids = vec![mf, mc_pid];
    self.component_roles = Vec::new();
    return true;
}
```

`add_mesh_merged` currently hardcodes the renderer-path mesh usage; for
colliders it needs the `16`/`36` usage flag the split path computes in
`attach_primitive` (builder.rs:1670–1676) — hence the
`add_mesh_merged_with_usage` variant (or thread a `usage` param through
`add_mesh_merged`). The merged-mesh geometry invariants (shared vertex stream
from prim[0], concatenated index buffer, N sub-meshes each carrying the
mesh-wide AABB, `m_MeshLodInfo.m_SubMeshes` N stubs) are identical to the
renderer merge already implemented in `add_mesh_merged`.

### Regression safety

- No parity fixture contains a multi-prim collider mesh (checked all 80
  `tests/fixtures/parity/sources/*`; the colliders present are all single-prim).
  The discriminator (≥2 prims **and** shared accessors **and** is_collider)
  therefore never fires on a parity fixture → `cargo test --test parity_bytes`
  unaffected.
- PathID layout shifts on the 3 affected bundles (5 PathIDs removed per merged
  extra prim) — same class of renumber the renderer merge already accepted.
- Gate must keep rejecting skinned / morph multi-prim (the SMR/baked collider
  suppression path at builder.rs:1664 stays the authority for those; the
  collider-merge branch should only fire when `becomes_smr` is false, which the
  no-skin/no-morph gate already guarantees).

### Expected result

+86,285 B (≈96 % of the glb-animated signed Σ) cleared from this corpus, plus
the parent-Transform `m_Children` over-size on each affected node. Must be
re-verified with `objalign` showing OURS==REF object counts on the three
bundles, then a full `abgen-verify` pass to confirm no parity regression.

## Reproduction

```bash
export ABGEN_CONTENT_ROOT=/home/dcl/umbrella/data/content_server/contents
OURS=/tmp/abgen-val300-integrated
REF=/home/dcl/umbrella/ab-generator/unity-reference-ab/ad0564d-val300-windows
B=QmUwmgapjirBnBxdKeeyDvubTFucf95NHzmhdkpMMb4Zve/QmPTpGqQukY3hvc77FWT4G7TXkBbfuxqSYbA1FiR48NBvW_windows
./target/release/examples/objalign $OURS/$B $REF/$B   # 183 vs 168, 15 OURS!, 3 collider GOs
# source mesh: 4 prims sharing accessors 211/212/213/214/215  →  merge-eligible
```
