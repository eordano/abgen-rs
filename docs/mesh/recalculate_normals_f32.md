# RecalculateNormals: single-precision arithmetic, no unit-axis fallback

**Why it matters:** When a glTF primitive ships no NORMAL accessor, the converter's importer recomputes normals on the mesh (`Mesh.RecalculateNormals()`), and abgen must reproduce the recompute bit-for-bit or the whole vertex blob diverges. abgen's earlier model accumulated the area-weighted face normals in f64 and substituted a `(0,0,1)` unit axis for vertices whose accumulator came out zero. That left two kinds of divergence on every recomputed mesh: 1-ULP noise across hundreds of vertices (different rounding order), and materially wrong normals on degenerate/seam vertices — a family of skinned wearable bundles stayed out of byte-identity on exactly this.

**How it was derived:** `examples/normprobe` dumps the recompute inputs (f32 positions, index buffer, abgen's normals) for every normal-less primitive of a glb. Comparing those against the reference bundle's normal lanes across many wearable meshes, exactly one arithmetic variant reproduces every vertex bit-exact:

- edges and cross products are computed in f32, each multiply/subtract individually rounded;
- the per-vertex accumulator is f32, accumulated in index-buffer triangle order (cross added to corners a, b, c in that order);
- normalization multiplies by `1.0f / sqrtf(x*x + y*y + z*z)` (left-associated f32 sums) — a reciprocal multiply, not a per-component division;
- an exactly-zero accumulator stays `(0,0,0)`; there is no unit-axis fallback.

The giveaway for the f32 accumulator was the reference value on degenerate seam vertices: unit vectors with component ratios like `(-1,0,8)/√65` or `(1,-1,0)/√2` — i.e. a normalized f32 *rounding residue* a few ULP in size, where exact (f64) accumulation cancels to zero. Where the reference instead holds `(0,0,0)`, the f32 accumulation also cancels exactly — so the residue, not an epsilon test, decides between the two.

**Where:** `src/normals.rs`. The function still takes and returns f64 (pipeline convention); all internal arithmetic is f32, so the returned values are exactly representable.
