# CAT7 ids_changed=false residue: one family — RecalculateTangents arithmetic

**What the class is.** The 19 unique non-world CAT7 pairs with matched object
ids (plus the 4 world CAT7s, plus bafkreifbxxne) all reduce to the same thing:
every differing object is a `Mesh`, and every differing byte sits in the
`m_VertexData` tangent lane (`Tangent.x/y/z/w`), with a tiny side-tail of
1-ULP `BlendWeight` diffs on two skinned wearables. No ids, no structure, no
textures — `examples/cat7drill` (typetree field attribution) and
`examples/vdiff` (vertex-channel attribution) establish this mechanically.

**The derived rule (landed).** Byte-level enumeration against 306 extracted
reference meshes (`examples/tangext` dumps geometry + both tangent lanes from
a bundle pair; `examples/tangsearch` scores arithmetic variants) recovered
two bit-level facts about the reference tangent recompute that our
implementation missed:

- the per-corner angle-weight edges are computed in **f32** — the same
  rounding as the triangle edges — not f64;
- the angle cosine is normalized by **one combined `sqrt(l1sq*l2sq)`**, not
  `sqrt(l1sq)*sqrt(l2sq)`.

Both are in `src/tangents.rs`. Effect: ~100 of the 306 meshes went fully
byte-exact; val300 windows byte-identical 4564 → 4593. One target bundle
(QmfTVNyi…, the scene with a ref-extra object) closed via the concurrent
"New Game Object" wrap fix; the rest shrank.

**Eliminated hypotheses (don't re-try).** Each of these was scored across all
306 meshes:

- all-f32 pipeline (normals-style): much worse — tangents really are f64 with
  f32-rounded edge inputs;
- per-stage f32 narrowing (raw s/t vectors, dir normalize, weights,
  accumulators, final stage): every f32 stage regresses;
- store-roundings (round normalized dirs / weights / raw vectors to f32):
  regress;
- corner edges chained from the cached triangle edges (`e3 = e2 - e1`):
  regresses vs re-differencing positions;
- welded accumulation (slots keyed by pos / pos+nrm / pos+nrm+uv): all
  regress; co-located duplicate vertices carry *different* reference
  tangents, so the reference does not weld (`examples/tangweld`);
- final-stage variants (renormalizing the vertex normal, reciprocal
  normalize, gate on mag², wider/narrower gate, orthonormalized tan2 in the
  handedness dot): zero fixes without breaks (`examples/tangsolve`) — the
  final stage is right, the divergence is upstream in the accumulated sums;
- ±1-ULP perturbation of each corner weight (libm `acos` identity): explains
  only ~13% of remaining bad vertices (`examples/tangulp`) — not the story.

**What remains (the open frontier).** After the fix, 247 tangent-only meshes
still differ. The residue is concentrated where the accumulated `tan1` sum is
a catastrophic-cancellation residue (|tan1| ~ 1e-7..1e-5 from O(1)-magnitude
contributions): UV-seam rings, mirrored islands, sliver fans. There, a
sub-ULP difference anywhere in the f64 chain changes the surviving residue
direction wholesale (and flips the 1e-6 degenerate gate), which also produces
the `ref-fallback-we-real` and `w-flip` classes as side effects of the same
unknown. `examples/tangratio` (implied weight-ratio solver on 2-triangle
vertices) shows the reference residue at such vertices is not even in the
span of our two per-triangle directions — one contribution enters the
reference sum with a different direction than ours, ruled out so far: raw-vector f32
cancellation sign-flips, UV-V flips, welding. The classes
(`examples/tanganal`): ~9.6k vertices ulp≤4, ~7.2k big-other (cancellation),
~559 ref-fallback, ~626 w-flip-only.

The 4 world CAT7 pairs are this same family (their drill output is identical
in shape: Mesh tangent lanes only) — they were built before the fix and need
a world-store rebuild to re-measure.

Separate small family: 1-ULP `BlendWeight` lane diffs on two skinned
wearables (bone-weight renormalization rounding) — undrilled, low value.
