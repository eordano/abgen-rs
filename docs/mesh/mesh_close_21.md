# Mesh flag and blendshape-compaction rules

**Why it matters:** Several Mesh objects diverged from the reference on flag values and blendshape (morph-target) content. These are subtle import-semantic details: which mesh-usage flag a mesh carries, whether a near-zero morph-delta vertex survives compaction, and whether a mesh keeps its vertices — getting any of them wrong makes the Mesh bytes differ even when the geometry is correct.

**How it works:** Three rules were derived from full-corpus inspection of the reference corpus.

First, orphan-skin meshes get a zero usage flag. When a collider-named parent primitive is also skinned, abgen-rs suppresses the renderer/filter/collider emission but still allocates the mesh (to keep sibling PathIDs deterministic). That mesh has no consumers, and the reference marks such consumer-less meshes with usage zero rather than the skinned-renderer flag — so abgen-rs forces the usage to zero whenever emission is suppressed.

Second, blendshape compaction uses a one-ULP epsilon, not strict zero. The reference drops a morph-delta vertex whose position, normal, and tangent components are all at or below one f32 ULP near one (two-to-the-minus-twenty-three), treating that as sparse-accessor round-trip noise rather than real morph data. abgen-rs uses the same threshold with a strict greater-than test, and sets the per-shape hasNormals/hasTangents flags from the surviving vertices rather than from mere accessor presence.

Third, `m_KeepVertices` and the mesh-usage flag are keyed on whether the source primitive carried morph targets at all — an import-level semantic — not on whether any morph vertex survived compaction. A mesh whose morph deltas all compact away still keeps its vertices and its morph usage flag in the reference, so abgen-rs gates those on the presence of source morph targets.

Remaining Mesh divergences (recomputed tangents on UV seams, per-bone AABB bounds, a non-standard collider flag, and isolated blend-weight rounding) are not addressed here. The tangent case is notable as a near-irreducible finding: the sign flips occur on lone triangles with tiny UV determinants, where catastrophic cancellation in the tangent formula makes the result sensitive to the exact f32 reduction order — reproducing the reference's SIMD order without disassembly remains open.
