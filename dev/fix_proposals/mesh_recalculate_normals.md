# Mesh normals were never recomputed for primitives lacking a NORMAL accessor

> **Status: landed.** val-300 windows: glb-wearable diff-bits
> 1,283,591,835 -> 1,280,735,606 (-2,856,229, -0.22%); TOTAL diff-bits
> 5,934,331,304 -> 5,931,475,075. byte-id 2268 unchanged. No other kind
> regressed. parity_bytes gate green (2 passed, no cap violated).

## Root cause — precise

When a glTF primitive has **no `NORMAL` attribute**, `src/gltf.rs:728` filled
the normal buffer with the placeholder `[0,0,1]` for every vertex and never
replaced it. The packed `m_VertexData` NORMAL channel therefore shipped a
constant `[0,0,1]` where Unity ships a real per-vertex normal.

Unity's GLTFast importer (decentraland fork) handles this case explicitly:

- `GltfImport.cs:2947`:
  `config.calculateNormals = !hasNormals && (mainBufferType & Normal) > 0`.
- `mainBufferType` gets the `Normal` flag for any triangle primitive whose
  material `RequiresNormals` (`GltfImport.cs:2783-2785`), and
  `Material.RequiresNormals => extensions?.KHR_materials_unlit == null`
  (`Schema/Material.cs:203`) — i.e. every lit material.
- `PrimitiveCreateContext.cs:105-109`: when `calculateNormals`, Unity calls the
  native `Mesh.RecalculateNormals()` on the merged mesh after the (flipped-
  winding) index buffer is set.

So for every indexed-triangle primitive with a lit material and no source
normals, Unity computes area-weighted normals. We emitted a constant instead.

## The algorithm (clean-room, Unity-probe-validated)

`Mesh.RecalculateNormals()` is area-weighted face-normal accumulation: for each
triangle the *un-normalized* cross product `(p1-p0) × (p2-p0)` (magnitude = 2×
area) is added to each of the three vertex accumulators; each vertex normal is
then normalized. Validated to 2.3e-7 against the real-Unity probe fixture
`tests/fixtures/unity_probe/no_normal_mesh.json` (the same fixture the existing
`gltf::tests::no_normal_area_weighted_matches_unity_probe` test uses).

Precision detail that matters: the **edge subtractions are done in f32**
(Unity operates on the f32 vertex buffer). For near-degenerate triangles the
f32 rounding of `pb-pa` decides the sign of the tiny cross product; doing the
subtraction in f64 flipped ~6/844 boundary verts (worst error sqrt(2)). The
cross product and accumulation are kept in f64, normalized, then narrowed to
f32 at output — mirroring `src/tangents.rs`.

## Files changed

- `src/normals.rs` (new) — `recalculate_normals(positions, indices)`.
- `src/lib.rs` — `pub mod normals;`.
- `src/scene.rs` — `Primitive::has_source_normals: bool`.
- `src/gltf.rs`:
  - set `has_source_normals = ji(attrs,"NORMAL").is_some()` and pass it through
    `Primitive` construction;
  - new post-load loop (before the tangent-recompute loop, since tangents read
    these normals) that calls `recalculate_normals` for every primitive with
    `!has_source_normals && indices.len() >= 3`.

The gate collapses `material < 0 || RequiresNormals` to "always recompute" —
there is no `KHR_materials_unlit` material anywhere in the val-300 corpus, so
this is exact for it. (If unlit materials ever appear, add an `unlit` flag to
`Material` and gate on `material<0 || !unlit`.)

## Effect on the corpus (val-300 windows)

Per-channel `m_VertexData` divergence (examples/vchan_census), before -> after:

```
NORMAL       diff-bytes 404,411 -> 278,499   meshes 31 (same set)
TANGENT      diff-bytes 111,218  (unchanged, irreducible 1-ULP — see tangent doc)
BLENDWEIGHT  diff-bytes  33,899  (unchanged)
BLENDINDICES diff-bytes     219  (unchanged)
```

Mesh `m_VertexData` total diff-bytes 549,747 -> 423,835 (-125,912).

The pure-`[0,0,1]` meshes are the recovered class. Example
`bafybeid57vtjs4…#-7150752127812958590`: normal channel went from 9,750
diff-bytes (every vertex wrong) to 1,485 — the residual is the irreducible
1-ULP native-recompute boundary noise (same class as tangents), not a
remaining algorithmic gap.

## NEGATIVE FINDING — the 31 surviving normal-diff meshes are a *different* bug

31 meshes still show NORMAL divergence after the fix, but they are **not** the
no-normal class — their byte counts were unchanged by the recompute (e.g.
`bafybeidzj6mfkl…#-2410210866506275363`, 120,637 normal diff-bytes, unchanged).
These primitives **carry source normals** (so `has_source_normals == true` and
the recompute correctly skips them), yet our packed normals differ from Unity's
by a whole-direction amount (`ours=[-0.005,0.568,0.822]` vs
`ref=[-0.813,0.331,0.479]` — not a sign flip, scalar multiple, or 1-ULP).

This is the structural / vertex-welding class: Unity merges/welds vertices
across the primitives sharing one merged-mesh vertex buffer and the resulting
shared normal differs from our per-primitive copy. It is the normal analogue of
the tangent "structural class" (>32 ULP, multi-submesh merge) documented in
`mesh-tangent-1ulp_research.md §Structural class`. Out of scope for this pass;
it needs modelling Unity's merged-mesh vertex-weld + normal-share path, not a
per-primitive recompute.

## Diagnostic tools added (examples/)

- `mesh_field_census` — per-top-level-field Mesh divergence tally (stdin
  ours<TAB>ref pairs). Pinpointed `m_VertexData` as the dominant field and
  confirmed `m_LocalAABB` is 0 (closing out the AABB lead from
  `parallel_wave_findings.md §A` on this corpus).
- `vchan_census` — per-vertex-channel divergence tally. Separates the
  recoverable NORMAL signal from the irreducible TANGENT 1-ULP noise.
- `normal_drill` — per-mesh NORMAL forensic (vert count, diff verts, max ULP,
  sample verts). Used to distinguish the `[0,0,1]` class from the structural
  class.

## Verification

```
cargo test --release --lib normals      -> 3 passed (incl. Unity-probe 2.3e-7)
cargo test --release --lib               -> 132 passed; 2 pre-existing bc7_pure
                                            failures (baseline, unrelated)
cargo test --release --test parity_bytes -> 2 passed, no ppm cap exceeded
abgen-verify val-300 windows: TOTAL diff-bits -2,856,229, byte-id 2268 (held),
                              zero per-kind regression
```
