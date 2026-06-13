# Mecanim constant-curve classifier — value-rule recovered, per-clip gate not observable

Follow-up to `constant_curve_split.md`. `partition_curves` in
`src/animation_mecanim.rs` stays a pass-through (all curves streamed,
`m_ConstantClip` empty) because the rule below regresses the corpus.

## What the constant set actually is

Decoding reference emote bundles directly (`m_MuscleClip.m_Clip.data.{m_StreamedClip,
m_ConstantClip}` plus `m_ClipBindingConstant.genericBindings`) and joining against
the source glb curves shows, for an emote where Unity does populate the constant
clip:

- Classification is per binding-vector, never per-component. A whole
  Position/Rotation/Scale vector is constant or streamed as a unit.
- The streamed bindings come first in `genericBindings`, then the constant
  bindings, each group keeping ingest order. Scalar fan-out in
  `m_ValueArrayDelta` follows the same layout; the constant scalars are written
  as raw `f32` into `m_ConstantClip.data`.
- The bindings Unity factors out are near-constant transforms — typically the
  static leaf finger bones (`*Hand*4`) whose rotation never changes. Their
  source samples are not bit-equal: the x/y/z components jitter across dozens
  of distinct `f32` values all within a sub-ULP band (~5e-8) of the first
  sample, while w stays pinned. Unity stores the first sample's value.

For one such emote, selecting every binding whose max per-sample deviation from
its first sample is ≤ 5e-8 reproduces Unity's constant set exactly: same count,
and the emitted `f32` values bit-match `m_ConstantClip.data` entry-for-entry.
So when extraction runs, the rule is a value-constancy test with a sub-ULP
tolerance, emitting the leading sample.

## Why it can't ship — the gate is not in the source

The tolerance that reproduces one emote does not generalize, and no tolerance
can. Two emotes in the corpus are structurally near-identical at the source
level — same Blender exporter family, same node count, all-LINEAR, single
animation, same static leaf bones with the same ~5e-8 jitter — yet one
populates `m_ConstantClip` and the other leaves it empty. The decision to run
constant extraction at all is per-clip and depends on Unity's internal
post-retarget muscle representation, which is not derivable from the glb
samples we can observe. Tested and rejected as discriminators: source/baked
bit-equality, ULP-relative tolerance, absolute tolerance, keyframe rate
(keys/sec), exporter generator string, sampler interpolation, node/animation
counts. The selected/rejected boundary for the value test is also razor-thin
(selected max dev 4.5e-8 vs rejected min 5.1e-8), so even within a single clip
the band overlaps the noise floor of curves Unity keeps streamed.

A full val300-windows A/B confirms the regression. Implementing the value rule
(per-binding-vector, ≤5e-8 tolerance, streamed-then-constant binding reorder,
leading-sample emit) and rebuilding the whole corpus moves glb-emote the wrong
way: 10 of 21 emotes improve (the ones where Unity did extract — the worked
example drops ~8%), but 11 regress, several severely, for a net increase in
diff-bits. The regressors are exactly the emotes where Unity extracted fewer
constants than the value rule predicts: the extra curves we move out shift the
streamed renumbering and shift `m_ValueArrayDelta`, cascading the rest of the
clip out of alignment (the same cascade `constant_curve_split.md` predicted).
Non-emote kinds are untouched.

## Bottom line

The classifier is value-based with a sub-ULP tolerance, and we can match the
extracted set bit-for-bit when Unity chooses to extract. The unsolved part is
the per-clip gate that decides whether extraction runs; it lives in the native
muscle-clip builder (`Internal_BuildClipMuscleConstant`) and is not recoverable
from black-box source/reference inspection. Until that gate is known, shipping
any value-based split regresses more emotes than it helps, so `partition_curves`
remains a pass-through. Emote bundles are in any case blocked from byte-identity
by the clip-LFID wall regardless of this split.

## Pointers

- `src/animation_mecanim.rs::partition_curves` — current pass-through.
- `constant_curve_split.md` — prior framing (the cascade prediction).
- `animation_curve_data_residuals.md` — `m_MuscleClipSize` blocker (same class).
