# Mesh.m_BonesAABB — morph-aware recompute (D-followup)

> **Superseded by `bones_aabb_diagonal_corners.md`** (commit `c2b73e0`).
> The diagonal-corner rule closes the residual that R5 (this rule) left
> open. The R5 implementation it documents is no longer in `mesh_layout.rs`.


## TL;DR

Unity's mesh importer recomputes per-bone AABBs **after** blendshape (morph
target) deltas are folded into the rest pose — Agent D's hypothesis was
correct. The fix is to union `base` and `base + delta_t` (per morph target)
in `compute_bones_aabb` instead of using `base` alone.

In the full 280-bundle corpus only **one mesh** combines morph targets with
finite bone-AABBs: `bafybeic63qquhbk / pid=3836835455412195215` —
`M_uBody_BaseMesh_Mesh.004` (982 verts, 62 bones with 11 finite-AABB,
1 morph target with `fullWeight=1.0`).

With the morph-aware union (rule **R5** in `dev/bones_aabb_forensic.py`):

| measure                | before (R1: base-only) | after (R5: base ∪ base+δ) |
|---|---:|---:|
| differing bones (this mesh) | 10/11 | 9/11 |
| differing fields (this mesh) | 47/66 | 33/66 |
| Mesh bundle count | 21 | 21 (mesh still differs) |
| paired-object exact (corpus) | unchanged | unchanged |

R5 fully closes bones **10** (Spine1) and **36** (RightShoulder), partly
closes bones 12-14 / 37-38 (arm chains), and *opens* bone **0** (Hips) +
slightly worsens the spine x-axis for bones 9/11/60 (Spine, Spine2, Neck).
The mesh is therefore still not byte-equal; the residual 8 fields are
addressed in the §"What R5 doesn't fix" section below.

The patch lives in `src/mesh_layout.rs::compute_bones_aabb`; the caller in
`src/builder.rs` now passes the primitive's morph targets through.

## How Unity's importer treats BonesAABB

`Mesh.m_BonesAABB` is one AABB per bind-pose bone, expressed in **bone-local
space** (i.e. each vertex transformed by `inverseBindMatrix[b]`). Unity uses
it at runtime as a static, conservative bound for skinned-mesh culling — the
SkinnedMeshRenderer's `m_AABB.m_Extent` starts at zero and is recomputed
lazily, but the per-bone bounds drive `Bounds.Encapsulate` calls during
animation, so they need to cover **every blendshape configuration the bone
will ever see** at runtime.

Agent D's note (in `transform_rotation_simd.md` §"What remains") observed
this empirically: "Unity recomputes BonesAABB after considering blendshape
delta vertices. Including morph deltas matches some bones exactly but
overshoots others". The forensic confirms the first half exactly — for some
bones, the **R5 union** equals prod bit-for-bit:

| bone | description | R1 status | R5 status |
|---|---|---|---|
| 0  | Avatar_Hips        | EQ | DIFF (x ±0.34) |
| 9  | Avatar_Spine       | DIFF (x ±1.57) | DIFF (x ±0.37) |
| 10 | Avatar_Spine1      | DIFF | **EQ** ← R5 closes |
| 11 | Avatar_Spine2      | DIFF (x ±1.34) | DIFF (x ±0.81) |
| 12 | Avatar_LeftShoulder| DIFF | DIFF (z +0.04) |
| 13 | Avatar_LeftArm     | DIFF | DIFF (3 axes) |
| 14 | Avatar_LeftForeArm | DIFF | DIFF (4 axes) |
| 36 | Avatar_RightShoulder| DIFF | **EQ** ← R5 closes |
| 37 | Avatar_RightArm    | DIFF | DIFF (4 axes) |
| 38 | Avatar_RightForeArm| DIFF | DIFF (4 axes) |
| 60 | Avatar_Neck        | DIFF (x ±0.96) | DIFF (x ±0.35) |

So R5 closes 2 of the 10 originally-different bones at the cost of opening
bone 0 — net **−1 bone differing**, but **−14 fields differing**.

## Forensic, in one paragraph

`dev/bones_aabb_forensic.py` tests 18 rules against the prod bone-AABB for
this one mesh:

```
R1 base only 52/62 bones exact 324/372 fields
R2 base + 1.0*delta_t 53/62 339/372
R3 base + w_init*delta_t 53/62 339/372 (w_init=1.0 here)
R4 base + max(w_init,1.0)*delta_t 53/62 339/372
R5 union (base, base+delta_t) 53/62 339/372 ← chosen
R6 base ± |delta_t| 52/62 334/372
R7 base+delta_t only (no base) 53/62 336/372
R8 base+delta_t at f32 53/62 336/372
R9 base + 0.5*delta_t 52/62 325/372
R11 base + 2*delta 51/62 320/372 (overshoots more)
R12 base + 3*delta 51/62 318/372
R13 base + 1.5*delta 51/62 320/372
R14 union (base, +δ, +2δ) 51/62 320/372
R15 morphed only (per-vert non-zero)53/62 339/372 (== R5 since all 982 morph-positions can extend)
R16 per-axis split (x:R1, yz:R5) 52/62 336/372
R17 per-axis split (xy:R1, z:R5) 52/62 330/372
R18 per-axis split (x:R5, yz:R1) 51/62 327/372
```

Best is R2-R5 / R15, all equivalent for `mesh.weights = [1.0]` and one
target. R5 is chosen because it generalises cleanly to multi-target meshes
(the union extends naturally) and matches Agent D's "include morph deltas"
phrasing.

Weight thresholds were swept (`w > 1e-5.. 0.25`); none improved the match.
Mul-order variants for `mul_point3x4_f32` (V0/V1/V2/V3) were swept; the
current V0 (Rust `mul_point3x4` order) is the unique best, so the inner
loop math is already Unity-equivalent.

## What R5 doesn't fix

Two systematic residuals remain in this mesh — both arise from vertices
whose mesh-space coordinates, after the bone's inverse-bind transform,
extend further than `base + delta` predicts:

### 1. Spine/neck x-axis overshoot (bones 0, 9, 11, 60)

For bone 9 (Avatar_Spine), `prod m_Min.x = -22.5413`, R5 gives `-22.9140`
(overshoots by 0.37). The R5 extreme comes from vi=565 with weight 0.127 on
bone 9; its morphed-x in bone-9 space is **−22.9140**, but prod's value
sits at **−22.5413**, exactly **N = 0.82** along the [base, base+δ]
segment. Across the four overshooting bones the implied N is non-constant
(0.66 - 0.99), not a clean scale factor.

Hypotheses explored and ruled out:

- N as a function of `weight[bone]`: vi=121 on bone 60 has w=0.987 and
 N=0.74; vi=43 on bone 13 has w=0.869 and N=0.997. No correlation.
- Per-axis split rules (R16/R17/R18): break MORE fields than they fix.
- `base + N*δ` for any fixed N ∈ {0.5, 1.0, 1.5, 2.0, 3.0}: never beats
 R5 globally.
- Decomposing the affine bind transform into `bind_t(base) + bind_rot(δ)`
 and summing at f32: ULP-equivalent to R5, no benefit.
- "Primary-bone-only" contribution (vertex contributes only to its
 highest-weight bone): drops the matching count further (51/62).
- Threshold `weight > ε`: doesn't help at any ε ∈ {1e-5.. 0.25}.

### 2. Arm-chain z-axis undershoot (bones 13, 14, 37, 38)

For bone 13, `prod m_Min.z = -13.6158`. The most-negative z achievable by
ANY vertex bound to bone 13 (whether base or morphed) is **−12.8657**
(vi=86 morphed). prod's value is unreachable in bone 13's local space from
any vi bound to bone 13. The nearest vertex giving z≈−13.61 is vi=794, which
is bound to bones [11, 60, 36, 10] — *not* bone 13.

Hypotheses explored and ruled out:

- "Include descendants": bone 13's descendants are bone 14 + fingers; the
 union does match z in some bones but explodes y in spine bones to ~58.
- "Include mesh[2] vertices": mesh[2] has 25 vertices bound to bone 13,
 but their y-extent (18..39) is way above prod's max-y (21.70).
- "Child bone origin as virtual vertex": bone 14's bind-origin in bone 13
 space is (~0, 27, ~0) — y=27 exceeds prod's max-y (21.70).
- Including normals as extra position deltas (NORMAL targets are unit
 vectors and shouldn't drive position bounds, but worth ruling out):
 doesn't change AABB.

The likely answer is some Unity-internal logic specific to FBX-export
handling (Unity ingests the avatar through Maya/Blender → FBX → Unity, and
the FBX cooker may emit per-bone bounds-hints that the importer reads back
verbatim).

**Path to close**: the FBX-side of the import lives in
`asset-bundle-converter`'s public Unity project — specifically the Editor
scripts that handle the avatar/skeleton ingest. Reading them and the
deterministic-guids fork's avatar-import patches will surface either the
bounds-hint format or the virtual-vertex set Unity adds before
`Mesh.RecalculateBounds`. All public source — no Unity binary involved.

## The patch

`src/mesh_layout.rs::compute_bones_aabb` now takes `morph_targets: &[MorphTarget]`
and, for each vertex × bound bone, contributes both `base` and
`base + δ_t` for each target. For meshes without morph targets this
collapses to the original R1 loop, so non-morph meshes are unaffected.

`src/builder.rs` passes `&prim.morph_targets` through (already on the
`Primitive` struct from Agent J's morph wiring, commit `3128471`).

## Why this is safe

A scan of the full 280-bundle corpus
(`dev/measure_full_vs_prod.py` + UnityPy on every Mesh object) shows that
exactly **6 meshes** ship `m_Shapes.channels` (morph data), but only **1**
of those has any finite-AABB bone — the very mesh we're targeting. The
other 5 morph-having meshes' `m_BonesAABB` is all-(±inf), meaning no
vertices are bound to any bone (they're non-skinned morphs, e.g. blink
shapes on static face meshes). For those, R5 is a no-op — the inner loop
never runs.

The non-morph meshes (the other ~30 with finite bone AABBs in the corpus)
have `morph_targets = []` so the outer loop reduces to the original R1.
Those bones' AABBs are bit-identical before/after this patch.

## Files

- `abgen-rs/src/mesh_layout.rs` — `compute_bones_aabb` now takes morph targets.
- `abgen-rs/src/builder.rs` — pass `&prim.morph_targets` through.
- `abgen-rs/dev/bones_aabb_forensic.py` — 18-rule tester + per-bone breakdown
 for the residuals (committed alongside this doc).
- `abgen-rs/dev/fix_proposals/bones_aabb_morph.md` — this file.
