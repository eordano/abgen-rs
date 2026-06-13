# Mesh normal/tangent bit residual — irreducible native arithmetic

**Why it matters:** For byte-identical output, the packed mesh `m_VertexData`
channels must match the reference bit-for-bit. Positions, UVs, and normals already do.
Computed tangents (and a thin slice of computed normals) carry a small
per-component residual that prevents the last meshes from going byte-identical.

**How it works:** This is a negative finding — the residual is irreducible.
Read normals and read tangents (those present in the source glTF) are converted
with a trivial axis flip and are bit-exact. The divergence lives only in our
clean-room reimplementation of the `Mesh.RecalculateTangents()` algorithm,
used when the glTF has no TANGENT attribute. The reference keeps double-precision
temporaries through both the angle-weighted accumulation and the final
Gram-Schmidt orthonormalization; experiments that forced single-precision
accumulation or single-precision orthonormalization made parity dramatically
worse, confirming the f64 model is correct. What remains is the difference
between the reference's native (Burst/SIMD-compiled) instruction sequence for the
double-precision dot/sqrt/divide chain and our scalar f64 chain at shared
vertices with many contributing triangles — a sub-ULP narrowing/ordering
difference. The triangle iteration order and all non-obvious reference specifics
(per-triangle direction normalization, angle-weighted contribution) already
match. Closing the gap would require reading the reference's native codegen, which the
clean-room rule forbids. A separate handful of large divergences are mispaired
or structurally welded meshes, not an arithmetic class.
