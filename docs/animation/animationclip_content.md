# AnimationClip content parity — keyframe weights, paths, blendshape names

**Why it matters:** Once abgen-rs started emitting AnimationClips for glTF sources at all, the clips it produced still diverged from the reference in their interior fields — keyframe weighting, node-path strings, and blendshape attribute names. Each mismatch flips bytes in the serialized clip, so byte-identical output was impossible until the emission matched the converter's importer exactly.

**How it works:** The converter's glTF importer applies several non-obvious conventions when baking animation curves, and abgen-rs now reproduces each in `src/animation.rs`:

- **CUBICSPLINE keyframes carry `weightedMode = 3`** (both tangents weighted), not `0`. Multi-keyframe curves use a uniform half weight across every component; single-keyframe curves use the half weight on the first component only and fall back to the converter's single-key default-weight constant (the round-up f32 of one third) on the remaining components.
- **LINEAR single-keyframe curves** follow the same first-component-zero, rest-default-third weight pattern (with zero slopes and `weightedMode` left at `0`), where abgen-rs previously emitted all-zero weights.
- **Unnamed-node path fallback.** For a node whose name is absent or empty, the converter synthesizes `Node-{index}` (capital N, hyphen, zero-based) rather than a lowercase-underscore form. This propagates through the shared path helpers to the mecanim path as well.
- **Blendshape attribute fallback.** When a mesh has no `targetNames`, the converter names the FloatCurve attribute by the bare target index (`blendShape.0`), not a `Key`-prefixed form. The `Key`-prefixed spelling belongs only to the SkinnedMeshRenderer typetree path, not to AnimationClip curves.

With these in place the decoded AnimationClip content matches the reference exactly. Any residual reported by a byte-window audit is a layout-shift artifact from neighboring classes changing size in the same SerializedFile, not a real AnimationClip difference.
