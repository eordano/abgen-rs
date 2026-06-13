# glb-emote bundle SIZE — Mecanim AnimationClip streamed-coefficient residual

> **Diagnosis.** One trivial recoverable sub-cause landed (`-0.0` in
> `m_ValueArrayDelta`); the dominant size source is the
> `Internal_BuildClipMuscleConstant` streamed-clip `a`/`b` cubic coefficients —
> **irreducible** (same Unity-native blocker class as `constant_curve_split.md`).

## Target

21 `glb-emote` bundles on val300-windows (17 smaller, 4 larger). Per-kind
492,872 ppm. Total bundle size delta (ref − ours) across the 21 = **+330,509
bytes**. Example deltas: `bafybeicoja22…` Δ−10,298; `bafybeicfc4hmd…` Δ−25,139.

## SOURCE — pinned to the byte

The headline framing (clip serialization *length* differs) is **wrong**: the
AnimationClip objects are **byte-length-identical** ours↔ref. The size delta is
entirely a **content-value** divergence that changes LZ4 compressibility of the
SerializedFile block.

### Evidence chain

`objalign` on `bafybeicoja22…` (167 objects each side):

- Every object pairs at identical size **except** the AnimationClip pair, which
  doesn't pair at all — ours/ref carry **different PathIDs** (the known
  irreducible LFID, `landed/emote_animclip_pathid.md`). Matched by
  class+length instead, both clips are **the same byte length**:
  `132952 ↔ 132952` and `929528 ↔ 929528`.
- The only *paired* size diff is `AssetBundle(142)` 2548 vs 2536 (+12 B, the
  preload-table entry order, separate wall).

Decompressed SerializedFile: ours 1,305,488 B vs ref 1,305,472 B (Δ **+16 B**
= the AssetBundle delta). `.resS` byte-identical (1,398,128 == 1,398,128). So
the **compressed** bundle differs by 10,298 B while the **decompressed**
content differs by 16 B → the gap is pure **LZ4 compressibility of differing
clip-content bytes**.

Aligning the AnimationClip objects (same length) and diffing *values*:

| sub-field (typetree leaf)                              | small clip | big clip | recoverable? |
|-------------------------------------------------------|-----------:|---------:|--------------|
| `m_MuscleClip.m_Clip.data.m_StreamedClip.data[]`      | 494        | 5395     | **NO** (native) |
| `m_MuscleClip.m_ValueArrayDelta[].m_Start/m_Stop`     | 36         | 0        | **YES** (`-0.0`) |
| `m_MuscleClipSize`                                     | 1          | 1        | NO (native) |
| `m_MuscleClip.m_Clip.data.m_DenseClip.m_FrameCount`   | 1          | 1        | NO (native) |
| `m_DenseClip.m_BeginTime` (bundle 2)                  | 1          | —        | NO (native) |

No array **lengths** diverge — `curveCount`, binding count, scalar fan-out all
match. The streamed-clip frame **time grid** is identical (both at the source
glTF's 1/30 s spacing — *not* a 60 Hz resample; ours and ref both have 280
frames at identical times). The classification is all-streamed / zero-constant
on both sides (the `partition_curves` split is still a no-op, per
`landed/constant_curve_split.md` — and it is NOT the source here).

### The dominant source: streamed `a`/`b` Hermite coefficients

Each streamed key is a 5-tuple `(curveIdx, a, b, slope, value)`. Decoding the
differing slots across all 21 bundles:

```
streamed-clip 'a' coeff diffs (ours always 0): 417,054
streamed-clip 'b' coeff diffs (ours always 0): 541,200
streamed-clip slope diffs (last-ULP rounding):  654,013
streamed-clip value diffs:                            0
m_ValueArrayDelta -0.0 entries (ours):              112
```

- `a` and `b` are the cubic-Hermite polynomial coefficients of each segment.
  **Ours hardcodes them to 0** (`bake_scalar_keys` sets `a: 0.0, b: 0.0`;
  `encode_streamed_clip` writes `[k.a, k.b, k.slope, k.value]`). The reference
  carries nonzero `a`/`b`, computed by the native `Internal_BuildClipMuscleConstant`
  the converter invokes.
- The ref values are **tiny** (e.g. `b = -8.19e-10`, `a = -1.97e-7`) on
  segments where the `slope` already matches bit-for-bit — they are
  **muscle-space-projection numerical residue**, not the standard
  Hermite-from-glTF-tangent coefficients. Confirmed: the closed-form
  `b = (3Δv − (2m0+m1)dt)/dt²`, `a = (−2Δv + (m0+m1)dt)/dt³` over the
  glTF-derived tangents predicts magnitudes ~1e6× too large (matched only
  14.6 % bit-exact, all on already-zero segments). The reference coefficients come
  out of the native humanoid muscle builder operating on rest-pose-subtracted,
  muscle-axis-projected curves — the same engine code (`MuscleClipUtility →
  Internal_BuildClipMuscleConstant`, a native extern the converter runs) that blocks
  `constant_curve_classifier_probe.md` and `m_MuscleClipSize`.
- The `slope` diffs are pure **last-ULP** drift (`0.00030292023` vs
  `0.0003029202`) from the same native recompute path — not a systematic
  formula error.

## Recoverable vs irreducible

**Recoverable (1, landed):** `m_ValueArrayDelta[].m_Start/m_Stop` = `-0.0`
where ref has `+0.0`. Our axis conversion (`conv_translation` does `-v[0]`)
produces `-0.0` when `v[0] == 0.0`; we copied that straight into the delta.
The reference normalizes `-0.0 → +0.0` in `m_ValueArrayDelta`
specifically (the native muscle builder does this) — the streamed-clip `value`
slot **keeps** the `-0.0` sign
(verified: ref streamed lead `value` bits = `0x80000000`, ref delta `m_Start`
bits = `0x00000000`). Fix is a scoped `norm0()` on the delta values only.

**Irreducible (the rest, ~99% of the bytes):**
- `m_StreamedClip.data` `a`/`b`/last-ULP-`slope` — native muscle builder.
- `m_MuscleClipSize` (ours 0, ref e.g. 127608) — runtime struct size, not
  derivable from serialized layout (`landed/constant_curve_split.md` §"Why
  m_MuscleClipSize stayed 0").
- `m_DenseClip.m_FrameCount` / `m_BeginTime` — native; the empty dense clip's
  metadata, no clean closed form across samples (bundle 1: ours 554, ref 556
  at StopTime 9.2333×60=554; bundle 2: FrameCount matches but `m_BeginTime`
  ours 0 vs ref 0.0333).
- AnimationClip **PathID** — `landed/emote_animclip_pathid.md` (identity, not
  size; out of scope per brief).

All irreducible items gate on the same blocker: `Internal_BuildClipMuscleConstant`
is a Unity-engine native extern; the no-disassembly / no-LUT discipline
forbids both recovering its math and tabulating its outputs. Note that even if
`a`/`b` were recovered, the clip still can't reach byte-parity because the
PathID diverges — so the AnimationClip is a structurally-blocked object
regardless.

## Fix landed (the recoverable sub-cause)

`src/animation_mecanim.rs::build_mecanim_clips`, `m_ValueArrayDelta`
construction:

```rust
let norm0 = |x: f64| -> f64 { if x == 0.0 { 0.0 } else { x } };
```

applied to `m_Start`/`m_Stop` for both the streamed and constant tails. The
streamed-clip `value` emission is untouched (must keep `-0.0`).

**Verified:** on `bafybeicoja22…` the big clip's delta now reports
`670 entries, +0=60, -0=0` (was `-0=60`), matching ref's normalization. The
small clip's 36 `-0.0` leaves likewise flip to `+0.0`.

### Impact

Tiny — 112 `f32` slots across 21 bundles flip one bit each (`0x80000000 →
0x00000000`). It removes a real value mismatch (the bytes now equal ref at
those offsets) but does not move the bundle to byte-parity: the AnimationClip
remains dominated by the irreducible `a`/`b` residual + PathID divergence, so
the per-bundle size delta is essentially unchanged. It is landed as a
correctness win (a genuine ours-vs-ref byte mismatch eliminated, zero risk),
not as a size closer.

### Test bars

- `cargo test --release --lib` (animation): 7 passed.
- `cargo test --release --test parity_bytes` (`ABGEN_ROOT` set): 2 passed,
  gate green. The non-emote parity fixtures contain no mecanim clips, so the
  `norm0` change cannot regress them — confirmed unchanged.

## Numbers

| metric                                            | value |
|---------------------------------------------------|------:|
| emote bundles                                     | 21 |
| total size delta (ref − ours)                     | +330,509 B |
| AnimationClip object byte-length divergence       | 0 (lengths identical) |
| decompressed SF divergence                        | 16 B (AssetBundle preload) |
| streamed `a` coeff diffs (irreducible)            | 417,054 |
| streamed `b` coeff diffs (irreducible)            | 541,200 |
| streamed `slope` last-ULP diffs (irreducible)     | 654,013 |
| `m_ValueArrayDelta` `-0.0` slots (FIXED)          | 112 |

## Files

- `src/animation_mecanim.rs` — fix + the `norm0` rationale comment.
- `landed/constant_curve_split.md`,
  `landed/constant_curve_classifier_probe.md` — the `a`/`b` + `m_MuscleClipSize`
  native blocker (same root).
- `landed/emote_animclip_pathid.md` — the PathID identity blocker.
- `landed/animationclip_content.md` — the legacy (non-muscle) clip path that
  IS byte-exact, for contrast.
