# Emote muscle-clip constant classification (black-box recovered)

**Why it matters:** Every emote AnimationClip splits its scalar curves between
`m_StreamedClip` and `m_ConstantClip`. Getting the split wrong shifts curve
indices, permutes `genericBindings`, and cascades into hundreds of thousands of
diff bits per clip. Before this work the partition disagreed with the reference
on 14 of 27 val300 emote clips, in both directions.

**How it was recovered:** the probe technique — synthetic emote glbs substituted
into a real emote entity and run through the actual reference converter
(serving content from a flat local HTTP store), plus a constraint fit over
every val300 emote reference clip (the constant-last binding order exposes the
exact reference collapse *set*, not just counts). Probes: epsilon ladders for
translation wobbles around base values 0.5 and 100, STEP/LINEAR constant and
varying mixes, and partial-constant bindings per attribute.

**The rules** (implemented in `classify_constant`):

1. **Binding-atomic for every attribute.** A position or scale binding
   collapses only when all three components are constant; rotation only when
   all four quaternion components are. No partial bindings exist anywhere in
   the reference output (probe `cls-partial`: a position with only x varying
   streams y and z too).
2. **STEP curves never collapse**, even when every key holds the same value
   (probe `cls-step`; the importer gives step keys infinite tangents). This
   explained the entire "reference streams everything" regime — Body_Avatar,
   Dog, Maracas, GM_emote_prop are STEP-heavy exports.
3. **Two absolute f32 thresholds, both strict:** a curve is constant iff
   `max |v_i - v_0| < 2^-20` **and** `max |segment slope| < 8.940705e-7`.
   The comparison is absolute, not relative (a 1-ulp wobble at value 100
   streams while a 0.69-relative wobble at value 1.4e-6 collapses) and not
   `Mathf.Approximately`. The corpus fit pins each constant into a one-ulp
   window: value tol in `[9.536734e-7, 9.5367432e-7)`, slope tol in
   `(8.9406967e-7, 8.9407052e-7]`. The slope leg is what streams short-spacing
   wobbles (slope = wobble x key rate) and collapses long slow drifts.

**Importer-fidelity prerequisites** (same commits): LINEAR curves drop keys
whose time does not strictly increase and apply shortest-path quaternion flips
(negative f32 dot with the previous, already-flipped key), exactly like the
converter's bundled Apache-2.0 glTF importer; both run before tangents.

**Accuracy:** 4707/4709 non-step reference bindings classified correctly across
all 21 val300 emotes (`examples/collapse_boundary` joins our per-binding
`(max|v-v0|, max|slope|)` f32 bits against the reference clip's actual
constant/streamed span). The two misses are one MJ_01 rotation binding and one
Starting_Pose (DubSway) translation binding — both `ref=collapse, ours=stream`,
each off by exactly one f32 ULP.

These two are **provably unseparable by any threshold on the
(value-delta, slope) features**, not merely a tuning miss. In the Starting_Pose
clip itself:

| binding | `max|v-v0|` | `max|slope|` | reference |
|---|---|---|---|
| `…RightHandIndex4` translate | `0x35800001` | `0x3400891c` | **collapse** |
| `Avatar_RightUpLeg` translate | `0x35800000` | `0x3400891b` | **stream** |

The collapsed binding is strictly LARGER in *both* features than a streamed
binding in the same clip, so no monotone threshold (nor any monotone
combination) can place the larger one inside the constant set and the smaller
one outside. Root cause is a value-magnitude effect our absolute measure cannot
see: the collapsed delta (`9.54e-7`) rides on a tiny value (`v0≈1.4e-6`), the
streamed delta (`2^-20`) rides on a large value (`v0≈-8.91`) — i.e. Unity's
native `Internal_BuildClipMuscleConstant` constant test is NOT a function of the
authored key delta alone. Candidate features ruled out by direct computation:
relative `Mathf.Approximately` (inverts the pattern — its `FLT_EPSILON*8` floor
≈ 2^-20 makes the streamed cases "approximately equal"), the streamed `a`/`b`
monomial coefficients (both exactly `0x0` for collapsed *and* streamed), f32-vs-f64
subtraction order (identical bits). The deciding quantity is a native micro-decision
at the 1-ULP boundary, same irreducibility class as the BC7 AVX2-vs-ISPC
float-order wall. They keep those two clips (and only those) off byte-parity;
everything else in all 21 emote bundles is byte-identical modulo m_TOS order.

**Downstream rules recovered in the same session** (each its own commit):

- `genericBindings` order = stable partition, constant bindings last.
- `m_DenseClip.m_BeginTime` = first keyframe time; `m_FrameCount` =
  `(int)((stop - begin) * rate) + 2` in f32 (20/20 reference combos).
- Streamed `b` coefficient = Hermite quadratic form `3dv - t1 - t1 - t1`
  scaled by `1/dt^2` (NOT `-a*dt`) — bit-exact incl. cancellation residues.
- The `-FLT_MAX` lead frame lists every curve, including late-starting ones.
- `m_ValueArrayDelta` start/stop = Horner eval of the boundary key at local
  time 0; signed zeros follow IEEE propagation through the coefficient chain.

**Result:** 19 of 21 val300 emote pairs now have byte-identical AnimationClip
objects (`clip_cmp` total 0). The emote bundles as a whole remain CAT7 only
because of the AnimatorController clip-PathID session randomness and the m_TOS
ordering (see docs/animation/emote_animclip_pathid.md, docs/animation/emote_animator_tos_order.md).

**Tools:** `examples/stream_solve.rs` (streamed-key aligner + segment context),
`examples/bind_dump.rs`, `examples/bind_diffs.rs`, `examples/clip_cmp.rs`,
`examples/cls_fit.rs`.
