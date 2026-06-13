# glb-emote bundle size — Mecanim streamed-clip coefficients

> **Status (2026-06-11):** the streamed-clip `a`/`b` coefficients and the
> constant/streamed partition are now derived (`src/animation_mecanim.rs`).
> Of the 21 val300 glb-emote bundles, 18 are **provably identical to the Unity
> reference except for the m_TOS hashtable order** (`examples/tos_only_proof`
> reorders our controller's m_TOS into the reference sequence, re-serializes,
> and requires byte-equality of every object). The remaining clip-SIZE residue
> lives in exactly **two** bundles (DubSway `Starting_Pose` ~168 B, and the
> `MJ_01` emote ~12 KB): each carries one binding that Unity collapses into
> `m_ConstantClip` but abgen streams, shifting the streamed-curve count by one
> binding and rippling the LZ4-compressed length. Those two are a 1-ULP
> constant-collapse boundary that is **provably unseparable by any threshold**
> on the authored (value-delta, slope) features — full evidence in
> `emote_constant_classification.md` (`examples/collapse_boundary`). The text
> below predates the `a`/`b` and partition derivations; it is kept for the
> `m_ValueArrayDelta` signed-zero note at the end.

**Why it matters:** Emote bundles diverge from Unity in size, and the headline suspicion — that the AnimationClip serializes to a different length — is wrong. The clip objects are byte-length-identical on both sides; the decompressed SerializedFile barely differs. The gap is almost entirely a content-value divergence inside the Mecanim muscle clip that changes the LZ4 compressibility of the file block, so the compressed bundle differs even though the uncompressed content is nearly the same.

**How it works:** Each streamed-clip key is a tuple of curve index plus cubic-Hermite coefficients `a` and `b`, a slope, and a value. abgen hardcodes `a` and `b` to zero, while Unity's `Internal_BuildClipMuscleConstant` computes tiny nonzero values for them. Those reference values are not the standard Hermite-from-tangent coefficients — recomputing the closed form over the glTF-derived tangents yields magnitudes vastly too large. They are numerical residue from the native humanoid muscle builder operating on rest-pose-subtracted, muscle-axis-projected curves. The slope diffs are last-ULP drift from the same native recompute. This is the same native-engine blocker class that also leaves `m_MuscleClipSize` and the dense-clip metadata underivable, and the no-disassembly/no-lookup-table discipline forbids both reverse-engineering the math and tabulating its outputs. The AnimationClip is structurally blocked regardless, because its PathID also diverges from Unity's.

One small recoverable sub-cause was fixed: `m_ValueArrayDelta` start/stop values were emitting negative zero where Unity normalizes to positive zero. abgen's axis conversion negates the component, producing `-0.0` for a zero input, and that was copied straight into the delta. Unity's muscle builder normalizes `-0.0` to `+0.0` in the delta specifically, while the streamed-clip value slot keeps the negative-zero sign — so the fix is a scoped normalization applied only to the delta start/stop fields. It eliminates a genuine byte mismatch but does not move the bundle to parity.
