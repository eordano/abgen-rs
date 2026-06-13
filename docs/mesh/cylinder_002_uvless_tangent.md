# Tangents for UV-less, normal-mapped primitives

**Why it matters:** A glTF primitive can declare only POSITION and NORMAL — no TEXCOORD and no TANGENT — yet still be assigned a material with a normal texture. In that case Unity fabricates a tangent channel anyway, so prod's mesh carries a TANGENT stream and a wider vertex stride. abgen-rs previously skipped tangent generation whenever UVs were absent, emitting no TANGENT channel and a narrower stride, which diverged across the entire affected mesh.

**How it works:** The discriminator for whether Unity emits a tangent on a UV-less primitive is exactly whether its material has a normal map — verified across the corpus as a clean one-to-one rule with no false positives or negatives. Other plausible signals (collider-only, material-assigned) do not split the cases. When there are no UVs to derive a meaningful tangent basis, Unity's generator produces a degenerate constant tangent of `(1, 0, 0, 1)` for every vertex.

abgen-rs reproduces this by removing the "skip if no UVs" short-circuit in the tangent post-pass (`src/gltf.rs`). A primitive without an existing tangent whose material has a normal image now runs through `calculate_tangents` with an empty UV slice, which already returns the degenerate `(1, 0, 0, 1)` per vertex — matching prod's stride and tangent values byte-for-byte.
