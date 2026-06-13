# AnimationClip constant-curve split — blocked on Unity's classifier

Goal: partition AnimationClip curves into `m_StreamedClip` + `m_ConstantClip`
to match prod's 1185 streamed / 395 constant split on emote
`bafybeickh2wiibpue…/super_emote.glb`. See
`animation_curve_data_residuals.md` for the prior framing.

## What was tried

`partition_curves` in `src/animation_mecanim.rs` was extended to classify a
curve as constant when every baked sample is **bit-exactly equal** to the
first sample (`(k.value as f32).to_bits == first_bits`). Constant curves
emit a flat `f32` into `m_ConstantClip.data`; streamed curves keep the
forward-difference encoding. `m_ValueArrayDelta` order: streamed first
(binding-order), constant second (binding-order, with `m_Start == m_Stop`).

Build was clean; lib + parity_bytes (10 non-emote fixtures) all passed.

## Why it didn't land — over-extraction cascade

Counts on the emote glb:

| metric                  | ours (with split) | prod   | Δ      |
|-------------------------|-------------------|--------|--------|
| streamed curveCount     | 1177              | 1185   | -8     |
| constant data len       | 403               | 395    | +8     |
| streamed.data bytes     | 318,227           | 325,607| -7,380 |

Bundle bit-diff vs prod:

| build      | bits diff | ppm     |
|------------|-----------|---------|
| baseline (no split) | 1,515,237 | 489,841 |
| with split (this attempt)        | 1,552,426 | 501,864 |
| Δ                                | **+37,189** | **+12,023** |

So the per-emote bundle gets **worse**, not better. The 8 extra curves we
classify as constant (but prod kept streamed) cascade because:

1. Their values disappear from prod's streamed binary at specific binding
 positions — the rest of our streamed stream shifts.
2. The 8 extras land in `m_ConstantClip` at positions prod uses for entirely
 different curves — every prod constant after position 0 mismatches.
3. `m_ValueArrayDelta`'s constant tail of 395/403 entries diverges from
 entry 0 onward.

`animation_curve_data_residuals.md` predicted this case exactly:
> Over-extract (move curves prod kept in streamed) → ours-constant
> longer than prod's, ours-streamed shorter → cascade in the **other**
> direction.

## What the 8 extras are

Counter-diff of (ours classified const) vs (prod constant data):

- Ours marks 403 constants; prod has 395.
- Many extras are quaternion w-components with value exactly `1.0` (raw glTF
 keyframes all bit-equal) — but prod kept them streamed.
- Other extras include translation/scale curves with raw glTF keyframes that
 are all bit-equal yet prod kept streamed.

This means Unity's classifier isn't operating on raw glTF samples after our
axis-conversion. Plausible alternatives, none confirmable without disassembly
or Unity source:

- Unity may resample at the bake rate (60 Hz) and check post-bake values
 — a 2-key curve [t=0:1.0, t=1:1.0000001] would produce 60 distinct samples
 with linear interp, none constant.
- Unity may inspect the curve's **tangents** in addition to values — a
 curve with equal endpoints but non-zero tangents (overshoot) bakes to
 varying samples.
- Unity may use a small absolute-or-ULP tolerance for "equal", but the
 multi-distinct-constant-value distribution in prod (84 of `1.0`, 40 of
 `0.9999999403953552`, 34 of `1.0000001192092896`, …) rules out coarse
 rounding.

## Why m_MuscleClipSize stayed 0

Even if the split landed, the validation also required
`m_MuscleClipSize ≠ 0`. Per the prior write-up (proposal #2 in
`animation_curve_data_residuals.md`), `m_MuscleClipSize` is a Unity-runtime
in-memory size of the `m_MuscleClip` struct, not derivable from the
serialized layout without disassembly or a per-corpus lookup table (forbidden
by the no-LUT discipline). For this emote, prod's value `1319176` is not a
simple sum of `len(streamed.data)*4 + len(constant.data)*4`
(`= 1302428 + 1580 = 1304008`, off by 15,168 bytes attributable to internal
struct overhead).

## Next probe (not run here — same blocker as TOS ordering)

The minimum probe that could crack this is a Unity Editor IPC method —
analogous to `AbgenBundleProbe.cs::ProbeAcTOS` — that:

1. Loads an emote AnimationClip via `AnimationClip.GetCurveBindings`.
2. For each binding's curve, dumps:
 - raw `Keyframe[]` (time, value, inTangent, outTangent).
 - whether `EditorCurveBinding` is classified into streamed vs constant
     (introspect the `AnimationClipSettings` or `MuscleClip` blob if
     accessible — may require reflection of internal types).
3. Cross-references against the corresponding glTF source curves.

Until that probe surfaces the exact classifier — and a derivation for
`m_MuscleClipSize` — the constant-curve split stays out. Per
`animation_curve_data_residuals.md` §"Why these aren't blocking", the
AnimationClip PIDs don't pair across abgen-rs and prod today, so the
bits attributed to this delta count as ours-only / prod-only rather than
paired_diff_bits.

## Pointers

- `src/animation_mecanim.rs::partition_curves` — current pass-through impl.
- `animation_curve_data_residuals.md` — prior framing of proposal #1 + #2.
- `animator_controller_tos.md` — same Unity-internal-state blocker.
- Probe scripts (tmp, not checked in): the prod inspection used UnityPy via
 `cd /home/dcl/umbrella/ab-generator && nix-shell --run "python3 …"`
 reading `m_MuscleClip.m_Clip.data.{m_StreamedClip, m_ConstantClip}` and
 `m_ValueArrayDelta`.
