# Morph-target (m_Shapes) tangent and keep-vertex rules

**Why it matters:** glTF meshes with morph targets (blend shapes) serialize their
deltas into Unity's `m_Shapes` structure. abgen-rs was diverging from the
reference on two fronts: which vertices a morph target retains, and how each retained vertex's
tangent slot is filled. Both are baked into the byte stream, so any mismatch
blocks byte-identical output for every morphed mesh.

**How it works:** the converter's importer keeps a morph-target vertex only when the
target supplies a POSITION or NORMAL delta — a TANGENT delta alone is not a
keep-signal. For the tangent slot of each kept vertex, the reference never marks the
shape as having tangents (`hasTangents` is always false), and it does not read
the source TANGENT accessor. When the target ships no TANGENT accessor the slot
is zero; when the target does ship one, the reference reuses the normal triple verbatim
in the tangent slot instead of decoding the tangent buffer. abgen-rs reproduces
both behaviors in `build_m_shapes` (`src/mesh_layout.rs`): the keep predicate is
POSITION-or-NORMAL, and the tangent slot is filled per-target as either zeros or
a copy of the normal.

This closes the morph-shape divergence itself. A separate, irreducible-here
residual remains on `m_BonesAABB`: when a mesh widens its per-bone bounding boxes
by morph deltas, the reference appears to aggregate the morph contribution by some
weighting rather than a raw delta sum (dropping the contribution entirely makes
the divergence worse, so it is needed but not as a plain add). The exact
aggregation rule is not yet pinned and would require a synthetic single-bone,
single-morph probe through the converter to determine. Other leftover bits in these
bundles (AssetBundle dependency ordering, transform quaternion sign flips,
material default-value bytes) are generic, non-morph issues tracked elsewhere.
