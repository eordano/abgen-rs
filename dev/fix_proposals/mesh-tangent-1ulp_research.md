# Mesh tangent 1-ULP residual — recoverability research (RESEARCH_AREAS #10, follow-up)

Baseline commit: 12e0ff4. Reference: `ad0564d-windows` test-set.
Builds/binaries run through an FHS shell. Corpus regenerated from
`--from-reference` into `/tmp/ours_*`, censused with
`examples/mesh_nt_census.rs`. New tools added this pass:
`examples/tan_predicate.rs` (rounding-direction predicate analysis),
`examples/tan_churn.rs` (per-variant flip/break accounting),
`examples/tan_lab.rs` + `examples/list_tan.rs` (helpers).

## Source operation — pinned

The converter loads glTF with the **GLTFast** importer (decentraland fork
vendored at `asset-bundle-converter/Assets/git-submodules/unity-gltf`, namespace
`GLTFast`). Tangents are computed only when a primitive lacks `TANGENT` **and**
its material `RequiresTangents` (has a normal map) —
`GltfImport.cs:2787-2789`, gate matches ours (`gltf.rs:1084`,
`normal_image.is_none()` skip). The actual computation is Unity's **native
`Mesh.RecalculateTangents()`** — `PrimitiveCreateContext.cs:113-114`. It is
closed-source C++; our `src/tangents.rs` is the clean-room reimplementation.
Supplied-`TANGENT` meshes use the verbatim copy path (`gltf.rs:758`, `t.z*=-1`)
and are already byte-exact; the entire residual is in computed tangents.

## Headline (baseline, /tmp/ours_v0 vs ad0564d-windows, size-matched meshes)

```
TANGENT: 462 size-matched, 349 byte-identical, 10146 diff bits, ppm 325.7
  component ULP hist [=1,=2,3-8,9-32,33+] = [2930,114,143,78,484]
  per-mesh max-ULP class [max=1, max<=4, max<=32, max>32] = [32, 30, 15, 36]
positions/uv0/normals(computed-tangent set): 0 diff bits
```

Two disjoint residual classes:
- **1-ULP rounding class** — ~2930 components, 77 meshes (max-ULP <= 32).
- **Structural class** — 36 meshes with max-ULP > 32 (e.g. `DiscoBall` -611,
  `Object_0.002` +29). These are UV-seam / multi-primitive-merge /
  degenerate-triangle topology effects (Unity `RecalculateTangents` runs on the
  *merged* multi-submesh mesh sharing one vertex buffer; we compute per glTF
  primitive). Out of scope for the 1-ULP question.

## 1. Characterizing the 1-ULP residual — it is unbiased boundary noise

`tan_predicate.rs` over the 4602 exactly-1-ULP components:

```
direction [ref>ours, ref<ours] = [2400, 2202]          (~52/48, no bias)
by ours-LSB: lsb0 [1175,1062]  lsb1 [1225,1140]        (no even/odd signal)
by lane x/y/z/w = [[785,741],[787,730],[828,731],[0,0]] (w handedness sign never 1-ULP)
ref-even mantissa fraction 0.514, ours-even 0.486
round-to-even predicate correct: 2287/4602 (49.7%)     (= chance)
```

Every candidate predicate — diff sign, mantissa LSB parity, lane, value sign,
round-to-even target — is ~50/50. **There is no determinable predicate that
predicts the rounding direction.** The w (handedness) lane is never off by
1 ULP; w errors only occur inside the structural class.

## 2. Pinning the operation — every f64-preserving perturbation is neutral

All variants run through the **full pipeline** (the only faithful oracle; a
self-contained recompute from the stored vertex buffer does NOT reproduce the
pipeline — `tan_lab --selfcheck` matched only 57% / 131 meshes, because the
stored geometry/index order and the multi-primitive merge differ from the
per-primitive glTF inputs `calculate_tangents` actually receives). Variants were
env-gated in `tangents.rs` (`ABGEN_TAN_VARIANT`, reverted after measurement),
rebuilt, regenerated over the 22 entity dirs holding all 462 tangent meshes
(`/tmp/ref_tan`), and censused.

| # | change | byte-id meshes | tangent diff bits |
|---|---|---|---|
| 0 | baseline (f64 accumulate, single f32 narrow at output) | **349** | **10146** |
| 1 | FMA on final Gram-Schmidt dot `n·t` | 349 | 10145 |
| 2 | FMA (fnma) on subtraction `t - n*d` | 349 | 10146 |
| 3 | FMA on both 1+2 | 349 | 10145 |
| 4 | FMA on the mag dot `o·o` | 349 | 10146 |
| 5 | reciprocal-multiply normalize (`1/mag` then `*`) | 349 | 10146 |
| 6 | rsqrt-style normalize (`1/sqrt(dot)` then `*`) | 349 | 10146 |
| 7 | f32-narrow ortho residual before normalize | 317 | (regress) |
| 10 | **atan2(\|cross\|,dot)** angle instead of acos(dot/l1l2) | 349 | 10139 |
| 11 | FMA in the angle-weight dot | 349 | 10146 |
| 12 | normalize edges first, then dot, for the angle | 349 | **10102** |
| 13 | f32-narrow the cos argument before acos | 321 | (regress) |

Every variant that keeps f64 intermediates lands at **exactly 349** byte-id
meshes. The bit count only wiggles by at most -44 / +0 around 10146. The two
that regress (7, 13) inject an *extra* f32 narrowing mid-chain — confirming
(as the prior pass found) that Unity keeps double precision throughout and any
premature narrowing is wrong.

Variant 10 (atan2) is a *completely different transcendental* for the angle and
still moves only 7 bits net — proof that the f64 result differs from Unity's by
far less than f32 resolution everywhere except at rounding boundaries.

## 3. Churn accounting — the net recoverable fraction is ~0

`tan_churn.rs` (per-component, baseline-vs-variant, both vs ref) on the two
best movers:

```
V12 (normalize-edges-first): flip(wrong->exact)=6  broke(exact->wrong)=1  net=+5   meshes_changed=0
V10 (atan2 angle):           flip(wrong->exact)=4  broke(exact->wrong)=2  net=+2   meshes_changed=0
```

Out of ~5717 differing components, the best variant recovers a **net of 5
components (0.09%) and zero whole meshes.** That is indistinguishable from the
random ±churn expected when you nudge an f64 value sitting near an f32 boundary:
some boundary cases tip toward ref, an almost-equal number tip away. There is no
arithmetic ordering or formula that systematically recovers the residual.

## Conclusion — recoverable fraction and what is irreducible

- **Recoverable: ~0%.** No arithmetic reordering (FMA placement, normalize form,
  reciprocal vs divide, accumulation FMA), no alternative angle formula (atan2 vs
  acos, pre-normalized edges), and no per-component predicate (sign, LSB parity,
  round-to-even, lane, magnitude) flips a meaningful fraction. The single best
  candidate nets +5 of ~5717 components and 0 of 113 meshes. **diff-bits delta:
  effectively 0** (best observed -44 bits with 0 mesh-parity gain, not worth the
  algorithm change or the loss of clean Lengyel form).
- **No fix shipped.** `tangents.rs` is left at the baseline (the env-gated
  variant scaffold was reverted); the parity gate stays green
  (`cargo test --release --test parity_bytes` = 2 passed).
- **Irreducible core, precisely stated:** the 1-ULP class is unbiased boundary
  noise. Our f64 chain and Unity's native (MSVC-compiled, Windows-target)
  `RecalculateTangents` agree on the true tangent value to far better than f32
  precision; they disagree only on which side of an f32 rounding boundary a
  handful of components land, because the two implementations execute a
  *different exact f64 instruction sequence* (notably the `acos`/`sqrt`/divide
  primitives — MSVC libm vs platform libm). Closing it requires reproducing
  Unity's native instruction stream bit-for-bit, which needs the closed-source
  `RecalculateTangents` codegen — off-limits under clean-room. The structural
  class (36 meshes, >32 ULP) is a separate, larger-yield target: it would
  require modelling Unity's multi-submesh merged-mesh tangent accumulation
  rather than our per-primitive computation.
- **Magnitude:** the whole tangent residual is 10146 bits across the test set
  (~0.00025% of the 4.01e9-bit total corpus residual); invisible at the
  bundle-kind verify level. Area #10 remains correctly closed as low-yield, and
  this pass adds the rigorous evidence that the 1-ULP fraction is specifically
  *unrecoverable boundary noise*, not an unsolved ordering bug.

## Artifacts (kept, reusable)
- `examples/tan_predicate.rs` — rounding-direction predicate scan.
- `examples/tan_churn.rs` — per-variant flip/break accounting vs baseline & ref.
- `examples/list_tan.rs` — list ref bundles containing computed-tangent meshes.
- `examples/tan_lab.rs` — standalone variant harness (note: not pipeline-faithful;
  documents *why* a self-contained recompute is an invalid oracle).
- `examples/mesh_nt_census.rs` — (pre-existing) per-channel size-matched census.
