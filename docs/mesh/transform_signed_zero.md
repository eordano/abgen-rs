# Signed-zero normalization on translation.x

**Why it matters:** Transform `m_LocalPosition.x` was emitting `-0.0` where Unity emits `+0.0`, flipping a single sign bit in the serialized bytes. Though semantically identical, `-0.0` and `+0.0` are different byte patterns, so each occurrence broke byte-identical parity for that Transform.

**How it works:** The glTF-to-Unity basis flip negates the x translation. When the source value is zero, `-0.0` is the result, which Unity never produces here. The fix normalizes that single negation back to `+0.0` (`tx = -t[0]; if tx == 0.0 { tx = 0.0 }`). The scope is deliberately translation.x only: the basis flip is the sole mechanical source of `-0.0` in TRS, and forensic classification confirms every Transform residual in the corpus is exactly that one sign byte. Broadening the fix was rejected with measurements behind it — Unity's rotation and scale paths deliberately preserve the IEEE sign bit on zero results, so a universal writer-site sign-strip regressed Transform, AnimationClip, and Mesh parity.

## Rotation y/z: the integer `-0` source token

There is a second, *parser-level* signed-zero case that the translation.x fix does not cover, on the rotation lanes. The glTF→Unity basis flip negates rotation y and z (`-node.rotation[1]`, `-node.rotation[2]`). Crucially, this negation is applied **on top of the value Unity's JSON parser produced**, and Unity's `JsonUtility` has a token-form quirk for signed zero:

- An **integer**-form token `-0` (no `.`/`e`/`E`) goes through `JsonUtility`'s integer parse path, which discards the sign of zero → `+0.0f`.
- A **decimal**-form token `-0.0` goes through the float path and keeps the sign → `-0.0f`.

serde_json collapses *both* token forms to `-0.0`, so it cannot tell them apart after parsing. That mismatch surfaces on the negated lanes: a source `-0` (integer) is what Unity reads as `+0.0` and then negates to `-0.0` (the byte Unity serializes), but serde_json reads `-0.0` and negates to `+0.0` — losing the sign byte. (A decimal `-0.0` source, conversely, must stay `-0.0`→`+0.0` and is already correct.)

The fix mirrors Unity's parser exactly, at the read site, before any flip: re-read the raw JSON tokens for each `nodes[*].rotation` array and fold any **integer**-form negative-zero component (`-0`, `-00`, …) to `+0` in the parsed tree; decimal `-0.0` tokens are left untouched. This is not the rejected blanket sign-strip — it is the precise, token-derived rule (`fold_integer_neg_zero_node_rotations` in `src/gltf.rs`), confined to rotation, and it reproduces Unity's `JsonUtility` behaviour rather than guessing. It is scoped to rotation because that is the only TRS field where the basis flip negates a lane that can carry an integer `-0`; folding translation/scale/matrix the same way *regresses* corpus parity (their lanes legitimately carry decimal `-0.0` that must be preserved).

Any residual `-0.0` that remains after these fixes is data-driven — an explicit decimal `-0.0` shipped in the source glTF, faithfully preserved through the same negation Unity applies — not produced by the basis flip, and is out of scope for a code fix.
