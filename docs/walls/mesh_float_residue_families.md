# Mesh float-residue families: skin-weight and tangent walls

A large group of otherwise-identical bundles differ only in a small cluster of
single-precision float lanes inside one or two `Mesh` objects, with no texture
diffs and no structural (object-set) changes. Aligning the meshes by PathID and
classifying every diverging f32 lane against the reference shows the residue is
never spread randomly across the vertex stream — it falls into exactly two
lanes, and which lane depends only on whether the mesh is skinned:

- **Skinned meshes** (wearables, and scene meshes that carry bones): the diffs
  are exclusively in the **BlendWeight** lanes (the four normalized bone
  weights). Positions, normals, tangents and UVs are byte-identical.
- **Static meshes** (scenes and worlds with no bones): the diffs are exclusively
  in the **tangent x/y/z** lanes. The tangent `w` (handedness) is correct, and
  positions/normals/UVs are byte-identical.

Every diff is 1-2 ULP. Both lanes trace back to a native (C++) routine the
converter invokes that has no readable open-source counterpart, so the residue
is a faithful reproduction problem against opaque hardware arithmetic, not a
missing rule.

## Skinned meshes — bone-weight normalization

The DCL glTFast fork normalizes bone weights at **design-time** (editor import,
not playing) in exactly one place: `SortAndNormalizeBoneWeightsJob`. It sorts the
four lanes descending and renormalizes by the weight sum. (The
`VertexBufferBones.ApplyOnMesh` method earlier notes described as a second
"FIX by jinfeng" pass is, in this fork, just a `SetVertexBufferData` copy — it
does no arithmetic. There is only one normalize.) The integer→unit conversion
feeding the job is single precision (`off[x] / 255f`,
`off[x] / (float)ushort.MaxValue`); these match a `(u8 as f64)/255.0` cast to f32
bit-for-bit, so the conversion is not a source of divergence.

### The reciprocal is correctly-rounded `1/sum`, not a hardware estimate

This was established empirically with the reference corpus as an oracle. Every
reference skinned mesh exposes observed `(weight_sum → normalized-weight-bits)`
pairs; for vertices whose sum is bounded away from 1.0 the solved per-vertex
multiplier is exactly the IEEE correctly-rounded reciprocal of the sum — e.g.
sum `0x3f7fffff` → multiplier `0x3f800001`, sum `0x3f800001` → multiplier
`0x3f7ffffe`, matching `1.0f32/sum` bit-for-bit in every case. There is no
`rcpps`+Newton lookup table to reconstruct. The earlier "`0x3f7fffff` where
correct rounding gives `0x3f7ffffe`" observation was an artifact of comparing
against a *different divisor* than the reference used (a plain f32 sequential
sum), not evidence of a non-correctly-rounded reciprocal.

This draws the **per-CID vs. instruction-model line** cleanly. Had the reciprocal
been a hardware estimate, the right move would have been to recover the CPU's
estimate table *from the corpus oracle* and apply it as a content-independent
CPU-instruction model — legitimate, and categorically different from a forbidden
per-CID lookup. But the operation turns out to be a portable, correctly-rounded
divide, so no CPU model is needed at all.

### What the residue really is — and the fix that landed

The remaining ~1-ULP divergences are a *divisor and rounding-form* problem baked
into the reference's Burst-compiled job, not a reciprocal-table problem:

- The reference's effective divisor is best modelled by the **f64 sum of the
  four sorted lanes rounded to f32**, applied as a **hoisted reciprocal multiply**
  (`mult = 1/sum; w[i] *= mult`). Against the corpus this divisor+form reproduces
  the reference for the largest share of vertices; a plain f32 sequential sum, or
  the per-lane divide form, each reproduce fewer.
- A residual set of vertices match neither form: the reference's exact per-vertex
  summation/rounding came from Burst's vectorized codegen, which is no longer
  reconstructable from the already-sorted, post-conversion lanes. That is the
  irreducible tail.

The concrete landed change: the previous code carried a special case that, when
the f32-cast sum equalled `0x3f800001`, did a *double divide* (`w /= s; w /= s`)
instead of the single reciprocal multiply. That heuristic was a net negative — on
val300 windows, dropping it (keep the f64-sum reciprocal multiply unconditionally
when sum > 0) gains **57 byte-identical bundles on val300 windows, with zero
regressions**. Every bundle the special case helped is also helped by the plain
reciprocal multiply, plus those 57 more.

Conclusion: the bone-weight residue is a **Burst summation/rounding-form wall**,
not a hardware-reciprocal wall. The reciprocal is portable and correctly rounded;
the un-closable tail is the exact f32 reduction order Burst chose per vertex,
which the post-sort lane bits no longer carry enough information to recover. The
tooling that established this — `examples/recip_harvest` (the
`sum → reciprocal-bits` oracle) and `examples/divmul` (the divisor/form
classifier) — models the CPU/compiler operation; it is content-independent and
never a per-CID lookup (which remains forbidden).

## Static meshes — tangent xyz

When a primitive has no `TANGENT` accessor, glTFast hands the mesh to the native
`Mesh.RecalculateTangents()` routine. That routine has no readable source. abgen
reproduces it with a carefully-reverse-engineered f64 kernel (corner-angle
weights on f32-rounded edges, a single combined `sqrt`, Gram-Schmidt projection
with a degenerate gate on the projection magnitude). That model already takes the
bulk of tangent meshes byte-exact, but a few vertices per mesh keep a 1-ULP
residue in the tangent x/y/z lanes — the gap between the f64 model and the native
f32 accumulation/normalize order. The handedness `w` is unaffected.

Conclusion: the tangent residue is the **native `RecalculateTangents` f32 wall** —
the remaining ULPs come from the exact op order of an opaque native kernel, and
closing them would require either Unity engine source (out of bounds for a
clean-room port) or per-vertex reverse engineering that risks regressing the many
meshes the current model already nails.

## Tangent wall — full constraint-set assault (irreducibility evidence)

A focused harvest extracted every diverging tangent lane across the static-mesh
float-residue corpus: **307 diverging meshes** from the 35 `glb-scene`
float-residue pairs (`float_w>0 tex_w=0`) plus the samoyed emote mesh —
**151,976 vertices**, of which the shipped f64 model gets **132,486 byte-exact**
(`examples/tangext` extracts the inputs + both sides' tangent bits;
`examples/tangsearch` / `tangsolve1` / `tangf32` enumerate kernel variants;
`tang1anal` / `tanganal` / `tanggate` classify the residue).

**The residual (19,492 bad verts) decomposes as:**

| class | count | nature |
|---|---|---|
| `ulp<=4` (xyz 1-4 ULP) | 11,707 | irreducible f32 rounding |
| `big-ULPnoise (<0.5° dir)` | 6,973 | irreducible f32 rounding, amplified on near-zero lanes |
| `w-flip-only` | 597 | handedness 1-ULP boundary noise |
| `ref-fallback-we-real` | 200 | degenerate-gate threshold cases |
| `big-DIRFLIP (>=0.5° dir)` | 15 | genuine direction flips (gate + cancellation) |

**95.8% (18,680) is pure f32-rounding residue.** Only the ~215 gate cases could
conceivably be a derivable rule — and they are not:

- **Precision domain is settled: the native kernel accumulates in f64, not f32.**
  Computing single-triangle vertices (deterministic, no accumulation order)
  entirely in f32 lands only **24%** byte-exact vs the f64 model's **90%**
  (`examples/tangf32`). f32-throughout is a catastrophic regression. The wall is
  op-*order* inside an f64 accumulation, not a precision choice.

- **No kernel variant beats the shipped arithmetic.** `tangsearch` sweeps 256
  op-order/rounding combinations (det reciprocal-vs-divide, normalize
  rsqrt-vs-div, sdir/finalize associativity, combined-vs-split sqrt, edge
  chaining, …). The shipped model (corner-angle weights on f32-rounded edges,
  combined sqrt, |ortho| gate) is the global maximum; every other combo ties or
  regresses.

- **The degenerate gate is provably irreducible.** The reference's
  fallback-vs-real decision does **not** track any magnitude feature: across the
  non-coincidental boundary cases, ref-fallback `|ortho|` spans `4e-19 … 5.6e-3`
  and ref-real spans `1e-6 … 1.3e-2` — fully interleaved (70,411 of 73,172 real
  verts sit below the maximum fallback `|ortho|`; `|tan1|`, `|ortho|`, `|tan2|`
  all overlap the same way, `examples/tanggate`). A real case at `|ortho|=1.0005e-6`
  keeps the projected direction while a fallback case at `1.0103e-6` falls back.
  Raising the gate to catch the `1.384e-6` fallback cluster fixes 26 verts but
  **breaks 790** (net −764 at gate 1.4e-6; every wider gate is worse). The
  reference's `sqrMagnitude < epsilon` test is evaluated on its own exact f32
  accumulation, whose sub-ULP value our f64 model cannot match — so the gate
  outcome is a downstream symptom of the same f32-op-order wall, not a separable
  rule.

- **Cancellation vertices confirm, not crack, the wall.** The catastrophic
  direction-flip vertices (e.g. samoyed's single diverging vertex; the planar
  `N=(0,0,±1)` clusters) are exactly the points where a near-zero accumulated
  tangent has its direction decided by sub-ULP accumulation differences. They are
  the most sensitive probes — and they confirm both sides agree on the f64 math
  to the last representable bit, diverging only because the reference rounded each
  intermediate to f32 in the native kernel's exact order. No reachable op order
  flips them to match without breaking the bulk.

**Verdict: CLOSED — irreducible.** The static-mesh tangent residue is the native
`Mesh.RecalculateTangents` f32 op-order, the same hardware/codegen-arithmetic
class as the BC7 ISPC and Transform-rotation `rsqrt` walls. The shipped f64 model
is already the best derivable approximation (132,486/151,976 verts exact); no
precision change, kernel variant, or gate rule improves the corpus, and several
provably regress it. No code change is warranted.

## Tooling

`examples/meshfloat <ours> <ref> <pathid>` aligns a `Mesh`'s interleaved vertex
stream by channel and reports, per diverging f32 lane (POS/NRM/TAN/UV/BlendWeight),
the two bit patterns and the ULP gap. It is what makes the skinned-vs-static split
visible at a glance.

Tangent-wall solver suite (all read `tangext`-extracted `.txt` mesh files):

- `examples/tangext <ours> <ref> <prefix>` — dump RecalculateTangents inputs
  (positions/normals/uvs/index buffer) + both sides' tangent bits for every
  PathID-matched diverging Mesh.
- `examples/tangsearch <dir>` — enumerate 256 op-order/rounding kernel variants;
  score whole-mesh + per-vertex + per-component byte-exact matches. Shows the
  shipped arithmetic is the global maximum.
- `examples/tangf32 <dir>` — true-f32 single-triangle solver; proves f32-throughout
  accumulation is a net regression (the kernel accumulates in f64).
- `examples/tang1anal <dir>` — single-triangle (deterministic) residual classifier:
  cancellation vs ≤2-ULP vs big.
- `examples/tanganal <dir>` — full-mesh residue histogram with angular
  (direction-flip vs ULP-noise) split.
- `examples/tanggate <dir>` — degenerate-gate boundary finder; proves no magnitude
  threshold separates fallback from real and any gate widening net-regresses.
