# Merging shared-stream multi-primitive nodes into one multi-submesh Mesh

**Why it matters:** when a glTF node's mesh has several primitives, abgen-rs
emits one child GameObject (plus Transform, MeshFilter, MeshRenderer, and Mesh)
per primitive. The reference, for certain sources, instead folds all those primitives
into a single Mesh on the parent node, carrying one sub-mesh per primitive. The
mismatch inflates object counts on affected bundles, shifts every PathID, and
dominates the divergence of the animated-glb cohort. Without matching the reference's
choice between splitting and merging, those bundles cannot reach byte parity.

**How it works:** the deciding factor is whether the primitives share a vertex
stream. When every primitive of a node's mesh references the same glTF accessors
for all per-vertex attributes (POSITION, NORMAL, TANGENT, the TEXCOORD set,
COLOR_0, JOINTS_0, WEIGHTS_0) and differs only in its index accessor and material,
the converter merges them into a single Mesh with one sub-mesh per primitive and a
MeshRenderer carrying one material per sub-mesh. When the primitives have
per-primitive attribute accessors, the converter splits them into separate child
GameObjects. This tracks the exporter: Blender's glTF exporter writes per-primitive
accessors and triggers the split path, while glTF exports that collapse
primitives onto shared accessors trigger the merge path.

On the merge path the merged Mesh uses the shared vertex stream, concatenates the
primitives' index buffers, and gives each sub-mesh the mesh-wide local AABB
rather than a per-subset recompute (verified: prod sub-mesh AABBs equal the
parent mesh AABB when the stream is shared). The merge is gated to vanilla
geometry only — skinned, morphed, and collider nodes always take the split path.

Status: designed, prototyped, empirically confirmed to eliminate the divergence on
the target bundles, and since landed in `src/` (see `mesh_tree_merged` in `builder.rs`).
The discriminator is verified to leave the parity fixtures (all in the split
bucket) unchanged. The only residual after merging is a pre-existing ULP-level
AABB precision drift, tracked separately.
