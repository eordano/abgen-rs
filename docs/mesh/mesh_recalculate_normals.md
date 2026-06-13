# Recomputing mesh normals when the glTF has no NORMAL attribute

**Why it matters:** When a glTF primitive ships no `NORMAL` attribute, abgen-rs
was filling the normal buffer with a constant placeholder and shipping that in
the packed `m_VertexData`. The reference instead carries real per-vertex
normals, so every such mesh diverged on its entire normal channel and could
never be byte-identical.

**How it works:** The converter's glTF importer (the Decentraland fork) sets
`calculateNormals` whenever a triangle primitive has no source normals and its
material requires normals — which is every lit material (only `KHR_materials_unlit`
opts out). It then calls `Mesh.RecalculateNormals()` on the merged
mesh after the winding-flipped index buffer is set. abgen-rs reproduces this:
for any primitive lacking source normals, it runs area-weighted face-normal
accumulation — for each triangle the un-normalized cross product of two edges
(its magnitude proportional to triangle area) is added to each of the three
vertex accumulators, then each vertex normal is normalized. A precision detail
matters: the edge subtractions are done in f32 because the reference operates on
the f32 vertex buffer, and for near-degenerate triangles that f32 rounding decides the
sign of a tiny cross product; the cross product and accumulation stay in f64 and
narrow to f32 only at output. The recompute runs before tangent recomputation,
since tangents consume these normals. Validated against a reference probe
fixture.

A residual class remains but is a different bug: some meshes that *do* carry
source normals still diverge by whole-direction amounts. Those come from the
converter welding vertices across primitives that share one merged-mesh vertex
buffer, so the shared normal differs from our per-primitive copy. Reproducing it
needs modelling the converter's merged-mesh vertex-weld path, not a per-primitive
recompute.
