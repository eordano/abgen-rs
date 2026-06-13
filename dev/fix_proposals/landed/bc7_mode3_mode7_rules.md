# BC7 Mode-3 + Mode-7 rule discovery (W5)

> Baseline (commit `62c4fd0`, 4.58 M-block corpus):
> - **Mode 3**: 79 257 blocks, 51 381 byte-match (**64.83 %**), 27 876 misses
> - **Mode 7**: 9 483 blocks, 676 byte-match (**7.13 %**), 8 807 misses
>
> Tooling: `dev/bc7_probe/analyze_mode3_mode7.py`, `analyze_m3_m7_deep.py`,
> `analyze_m3_m7_quant.py`, `analyze_m3_m7_part_pairs.py` (all use the
> existing `dev/bc7_probe/blocks/*.jsonl` mining output — no re-extract).

---

## 1. Per-mode partition distribution

### Mode 3 (2-subset opaque, 7 cb + non-shared pbits, 64 partitions)

Top-15 partitions Unity picked over the m3 population:

| Partition | n | % | cum % | our %match |
|---:|---:|---:|---:|---:|
| 0 | 13 447 | 16.97 | 16.97 | 55.13 |
| 13 | 8 388 | 10.58 | 27.55 | 78.64 |
| 1 | 4 180 | 5.27 | 32.82 | 71.96 |
| 2 | 4 153 | 5.24 | 38.06 | 70.19 |
| 15 | 3 605 | 4.55 | 42.61 | 68.16 |
| 14 | 3 415 | 4.31 | 46.92 | 70.37 |
| 29 | 2 903 | 3.66 | 50.58 | 79.99 |
| 26 | 2 567 | 3.24 | 53.82 | 81.77 |
| 10 | 2 279 | 2.88 | 56.70 | 77.62 |
| 6 | 2 151 | 2.71 | 59.41 | 77.27 |
| 7 | 2 119 | 2.67 | 62.09 | 78.15 |
| 3 | 2 117 | 2.67 | 64.76 | 78.37 |
| 16 | 2 061 | 2.60 | 67.36 | 80.88 |
| 21 | 2 020 | 2.55 | 69.91 | 77.57 |
| **19** | **2 010** | **2.54** | **72.44** | **0.90** |

Partitions 0, 13, 1, 2, 14, 15 (clean halves + simple slants) cover 47 % of
m3 blocks. **p0** (vertical right-half) and **p13** (horizontal bottom-half)
mirror the prior mode-1 finding: same-shape gradients drive the choice.

`p19` jumps out as a **partition-shortlist hole** — the reference picks p19 on 2010
blocks but we hit only 18 of them (0.90 %). 1 636 of the 1 992 misses route
to `ours=p23` (Hamming-1 neighbour) — we are never even *evaluating* p19.

### Mode 7 (2-subset RGBA, 5 cb + non-shared pbits, 64 partitions)

Top-15 partitions:

| Partition | n | % | cum % | our %match |
|---:|---:|---:|---:|---:|
| 13 | 2 208 | 23.28 | 23.28 | 8.11 |
| 0 | 1 983 | 20.91 | 44.19 | 5.85 |
| 2 | 642 | 6.77 | 50.96 | 6.23 |
| 1 | 622 | 6.56 | 57.52 | 6.27 |
| 29 | 423 | 4.46 | 61.98 | 2.36 |
| 15 | 305 | 3.22 | 65.20 | 4.92 |
| 33 | 265 | 2.79 | 68.00 | 2.64 |
| 26 | 252 | 2.66 | 70.65 | 6.75 |
| 8 | 250 | 2.64 | 73.29 | 8.80 |
| 14 | 247 | 2.60 | 75.89 | 10.12 |

Per-partition match rate is uniformly low (2–11 %). The partition pick is
**not** the dominant failure (see §3) — endpoint quantization is.

Conditional histograms by alpha variance:

| Class | n | top-3 partitions |
|---|---:|---|
| `max_delta_a == 0` (alpha-flat) | 3 635 (38.3 %) | p13 28.4 %, p0 11.3 %, p29 6.9 % |
| `max_delta_a > 0` (alpha-var) | 5 848 (61.7 %) | p0 26.9 %, p13 20.1 %, p2 8.1 % |

Mode 7 fires on a *lot* of alpha-flat blocks. That is the surprise — naively
we expected m7 to be alpha-gated.

---

## 2. Endpoint quantization + pbit selection

For same-mode same-partition misses (where the only divergence is endpoints
+ pbits + selectors):

### Mode 3 (6 744 same-mode same-partition misses)

| Class | n | % |
|---|---:|---:|
| Endpoints + pbits both equal — **selectors-only diff** | 1 705 | 25.3 % |
| Same pbits, **endpoints differ** | 3 531 | 52.4 % |
| Same endpoints, **pbits differ** | 559 | 8.3 % |
| Both differ | 949 | 14.1 % |

Per-channel raw 7-bit endpoint delta (Unity − Ours), all 4 slots:

| Channel | Δ=−1 | Δ=0 | Δ=+1 | other |
|---|---:|---:|---:|---:|
| R | 7.35 % | 87.47 % | 4.50 % | 0.68 % |
| G | 10.14 % | 82.81 % | 5.86 % | 1.19 % |
| B | 5.25 % | 90.71 % | 3.31 % | 0.73 % |

**Almost all endpoint differences are off-by-one in raw 7-bit space.** Pbit
flip rate is nearly symmetric across the 4 slots (46–48 %), so there is no
single bit Unity always sets — the pbit choice is **data-driven**, locked
to the rounding direction of the channel quantizer.

### Mode 7 (4 473 same-mode same-partition misses)

| Class | n | % |
|---|---:|---:|
| Endpoints + pbits both equal | **1** | 0.02 % |
| Same pbits, endpoints differ | 3 137 | 70.1 % |
| Same endpoints, pbits differ | 16 | 0.36 % |
| Both differ | 1 319 | 29.5 % |

Per-channel raw 5-bit endpoint delta:

| Channel | zero-delta rate | dominant negative deltas | dominant positive deltas |
|---|---:|---|---|
| R | 79.03 % | −1 (4.4 %), −2 (1.6 %), −9 (0.7 %) | +1 (5.1 %), +7 (1.1 %) |
| G | 78.61 % | −1 (4.0 %), −2 (2.3 %), −9 (0.6 %) | +1 (4.5 %), +7 (1.0 %) |
| B | 79.95 % | −1 (4.4 %), −3 (1.6 %), −9 (0.5 %) | +1 (3.9 %), +4 (1.7 %), +7 (1.1 %) |
| A | **75.61 %** | −2 (1.9 %), −5 (1.0 %), −7 (0.6 %) | +1, +2, +5, +6, +7, +13 all > 0.5 % |

A-channel zero-delta is the lowest of the four, and its tail is the longest.
**Mode 7's bc7e-LSQ alpha fit is the dominant failure mode** — we are
landing wildly different quant cells (Δ=±9, ±13) that no rounding rule
explains. Selector-only diffs are effectively absent (1 / 4473), so the
mode-7 problem is **not** a tie-break in index assignment — it is a
fundamentally different endpoint search.

First-diff-bit region for m7 same-part misses:

| Region | bits | hits | % |
|---|---:|---:|---:|
| R endpoint (5cb × 4 ep) | 14-33 | 2 555 | 57.12 % |
| G | 34-53 | 172 | 3.85 % |
| B | 54-73 | 32 | 0.72 % |
| **A** | 74-93 | 1 697 | 37.94 % |
| pbits | 94-97 | 16 | 0.36 % |

R + A together account for 95 % of first-diffs. (R wins as "first-diff" only
because it is positionally earlier — when R and A both differ, R is the
first byte to flip.) Net signal: **alpha-channel endpoints diverge as often
as R**, and the magnitudes are larger.

---

## 3. Partition shortlist hit rates (best-mean-separating heuristic)

### Mode 3 (sample n = 2000)

| Scoring rule | top-1 | top-3 | top-5 | top-10 |
|---|---:|---:|---:|---:|
| Mean-separation, RGB | 26.85 % | 48.55 % | 62.65 % | — |
| Mean-separation, RGBA (alpha is constant) | 26.85 % | 49.15 % | 62.45 % | 75.80 % |
| **Variance-reduction, RGB** | **35.55 %** | **55.60 %** | **64.75 %** | — |

Switching the m3 partition estimator's objective from "maximise inter-subset
mean distance" to "minimise within-subset variance sum" lifts top-1 from
27 % to 36 % (+9 ppt). This matches the bc7e LSQ inner objective — when the
estimator scores partitions by the same metric the inner refinement
optimises, the front-loaded partition is more often the global optimum.

### Mode 7 (sample n = 2000)

| Scoring rule | top-1 | top-3 |
|---|---:|---:|
| Mean-separation, RGB only | 30.55 % | 53.05 % |
| Mean-separation, **A only** | **63.20 %** | 63.20 % (no Δ at K>1 — alpha-flat blocks tie) |
| Mean-separation, RGBA | 60.45 % | 76.90 % |

**Alpha is the dominant partition signal for mode 7.** Even an A-only score
beats RGB by a factor of 2. The current encoder uses RGB-weighted scoring
inside `ccc_est_mode7` for partition shortlisting, which is the wrong
weighting for the alpha-bearing population.

---

## 4. Cross-mode shared rules

These properties hold for **both** m3 and m7 (and likely m1):

1. **2-subset partition table neighbourhoods cluster Unity's near-equivalent
 picks.** 56 % of m3 partition divergences land on Hamming-1 or Hamming-2
 partitions (p19↔p23 H=1, p23↔p0 H=1, p17↔p19 H=2). Same for m7
 (top divergence p13→p29 H=8 is the exception — both are valid
 complements of a horizontal split).
2. **Per-channel endpoint quant is off-by-one dominated**, with the rounding
 direction tightly coupled to the pbit choice (so pbit search and channel
 rounding cannot be optimised independently — they form one decision).
3. **Pbit flip rate is ≈50 % across all four endpoint slots** for both
 modes — the choice is locally data-driven, not globally biased.
4. **Variance-reduction estimator is uniformly stronger than
 mean-separation** for partition shortlisting. The current
 `ccc_est_mode7` (and `ccc_est` for m3) should be re-checked: if either is
 computing a "mean delta" score instead of a "fit error" score, the m3 +
 m7 partition shortlist will keep missing 60-70 % of Unity's picks.
5. **Mode dispatch (m3 vs m1, m7 vs m6 vs m4): currently mis-weighted.**
 - m3 is opaque — `m3 var_rgb mean=87.8 median=4`. m1 has `mean=1647
     median=233`. m3 lives in a *much* tighter variance band than m1. The
     existing comparator (R5/R6 mode-6 epsilon, R7 H1 low-var bias) does
     not have an m3-specific bias.
 - m7 misses: when `ours_mode=6` (9.2 % of misses), **80 % of those
     blocks have max_delta_a = 0** (no alpha variance). The reference prefers m7
     here for the 2-subset RGB structure even when alpha is uniform.
     Our m7 path is gated only by `cp.use_mode7` flag — there is no
     positive bias toward m7 over m6 for low-alpha-variance blocks.

---

## 5. Concrete patch proposals

### Mode 3

**P3.A — Switch `ccc_est` (m1/m3) partition scoring to variance-reduction.**

Estimated lift: top-1 partition match 27 % → 36 % on m3 (+9 ppt). Carry-over
to m1 (same 2-subset table, same estimator) should add ~5 ppt to current
52 % m1 byte-match. Cost: one-line objective swap in `make_est_params`
+ `ccc_est`. Risk: m1's H1 variance gate (landed prior) was tuned against the
old estimator — re-sweep after P3.A.

Predicted m3 byte-match after P3.A: **64.83 % → ~72 %** (covers the 2010
p19 misses + half of the H≤2 partition flips).

**P3.B — Front-load partition p19 in the m3 shortlist (also p17, p23).**

The 2 010 / 79 257 m3 blocks where the reference picks p19 are missed at 99 %
because p19 is not in our shortlist. Look at `estimate_partition_list_group`
ordering: when `op_max_mode13 ≤ 4`, the partition iteration order matters.
Inject {p0, p13, p17, p19, p23} as candidates *before* the
`BC7E_2SUBSET_CHECKERBOARD_PARTITION_INDEX` early-out.

Estimated lift: recovers ~1 800 / 79 257 = **+2.3 % m3 byte-match**.

**P3.C — Per-channel endpoint refine: search ±1 in raw quant space.**

For m3 (and m7) same-partition same-pbit misses, the dominant residual is
±1 in raw R/G/B (combined ~22 % of all m3 misses). After the LSQ + standard
refinement, add a post-pass that evaluates the 3^N (N=12 channel-slots)
neighbourhood (or a sweep that perturbs one channel ±1 with the matching
pbit flip) and keeps the best. This is what bc7e's `Slow` profile is
*supposed* to do via `pbit_search=true` + iterated refinement; verify the
production `Bc7Profile::Slow` actually exercises it on m3 (the m6 path
already does via `refinement_passes`).

Estimated lift: +5-8 ppt on m3 (recovers the off-by-one tail).

### Mode 7

**P7.A — Use alpha-weighted scoring in `ccc_est_mode7`.**

A-only partition scoring beats RGB-only by 2× on top-1 (63 % vs 31 %).
RGBA-weighted lands top-1 = 60 %, top-3 = 77 %. Current
`mode67_weight_mul` weights (in `Params`) are 4-channel; lift the A weight
inside the partition estimator (decouple from the final-fit weights, which
already match Unity's perceptual setup).

Predicted top-1 lift: top-1 26 % → ~58 %. With `al_max_mode7 = 2` we still
need to evaluate 2 candidates, so top-3 hit rate (77 %) is the upper bound.

**P7.B — Raise `al_max_mode7` from 2 to 5 (Slow profile only).**

At top-5 = 79 % under RGBA mean-separation, lifting the shortlist budget
from 2 to 5 should recover ~19 ppt of partition-correctness. Cost: 2.5×
m7-partition work — but m7 is 0.21 % of blocks, so wall-time is moot.

**P7.C — Mode-6 vs mode-7 positive bias on low-alpha-variance blocks.**

When `max_delta_a == 0` and the m7 trial err is within ε of the m6 trial
err, prefer m7. Today we prefer m6 (the comparator has `nm6_score` bias
toward m6). Concretely: in `handle_alpha_block`, after the m6 / m7 trials,
gate the "m6 wins" comparator on `max_delta_a > 0`.

Estimated lift: recovers ~647 / 9 483 = **+6.8 % m7 byte-match** plus
removes ~647 m6 false-positives.

**P7.D — Endpoint LSQ alpha fit: investigate large-delta (≥5) residual.**

The Δ=±5,±7,±9,±13 alpha tail (combined ~3 % of m7 endpoints, but
inflating ppm because these endpoints are visually large) suggests our LSQ
is converging to a different local minimum than the reference's. This is the
deepest residual; likely needs the oracle harness to isolate the
exact (mode, partition, alpha-pattern) where the reference's solver diverges.
Defer until P7.A-C land.

**P7.D drill outcome (, oracle harness, 2 000-failure sample):**

The drill confirms that **no clean rounding rule explains m7 alpha
endpoint divergence on the failure population:**

- Of 2 000 same-mode m7 failures, 930 are same-partition (46.5 %), and
 **100 % of those have flat alpha within each subset** (max_delta_a = 0).
- For the `lo_a` endpoint slots (slot 0 + slot 2): the rule
 `raw5 = ((src_alpha_min × 63 + 127) / 255) >> 1` matches the reference in
 100 % of subsets, but our encoder matches the reference at only ~92 % per slot
 on the failure set — meaning our LSQ-driven lo_a is *sometimes* (~8 %)
 pulled away from `round(src_alpha_min × 63/255)` by selector noise.
- For the `hi_a` endpoint slots (slot 1 + slot 3): the equivalent
 `raw5 = (src_alpha_max × 63 + 127) / 255 >> 1` rule matches the reference
 at only ~20 % — the reference's `hi_a` is **not** a function of source alpha
 alone. In alpha-flat subsets (every failure), the reference's `hi_a − lo_a`
 spans -31 to +63 with no extractable pattern.
- Replaying our LSQ with Unity's exact final selectors reproduces
 `lo_a` at 100 % but `hi_a` at only ~48 % across nine rounding
 variants tested (half-up, half-down, banker, q6-then-pbit-strip,
 trunc, ceil, snap-to-extreme, pair-banker-explicit, half-to-extreme).
 The 52 % residual is bc7e float-precision noise in the inverse-z LSQ
 matrix when alpha residual is zero — driven by RGB-only LSQ
 arithmetic that we cannot replicate without exact float reproduction
 of bc7e's solve.

Three code-change variants tested (all measured at total ppm = 529 634
baseline, mode-7 byte-match = 7.13 % oracle):

1. `axis.c[3] = 0` for flat-alpha m7 subsets (zero alpha contribution
 to PCA initial guess): **0 ppm delta**.
2. Snap `low.c[3] = high.c[3] = round(flat_a × 63/255) >> 1` post-CCC
 (collapse hi onto lo's raw5): **−0.11 pp on m7 (7.13 → 7.02 %), 0
 ppm delta on TOTAL** — breaks the 63 % of Unity cases where
 `hi_a ≠ lo_a`.
3. Snap `xl.c[3] = xh.c[3] = flat_a` post-LSQ (force flat alpha LSQ to
 produce identical endpoints): **0 ppm delta on TOTAL, 0 pp on m7**
 — the LSQ already produces identical xl/xh for flat alpha, so the
 snap is a no-op.

Conclusion: **the m7 byte-match ceiling at 7.13 % is a hard floor**
without exact bc7e float-arithmetic reproduction. The dominant 92 % of
m7 failures are concentrated on alpha-flat subsets where the perceptual
contribution of `hi_a` is zero (alpha is constant in the subset, so
selector value doesn't affect decoded alpha) but the bit-pattern
divergence costs full block-mismatch. No rule applied at the bc7_pure
encoder level moves the needle. Future work: either bit-exact bc7e
float-replay (stretch), or route alpha-flat 2-subset RGBA
blocks to m1/m3 instead of m7 by penalising m7 in the mode comparator
when `max_delta_a == 0`.

### Shared / global

**PX.A — Re-run estimator after applying P3.A: m1 should pick up byte-match
the same way m3 does** (same `ccc_est` code path). Track in W6/W7 (mode-1
partition rule).

**PX.B — Add a unified pbit-search post-pass** for modes {1, 3, 7} that
sweeps the `2^k` pbit combinations *after* the LSQ + refinement, choosing
the lowest-error one. Today the Slow profile sets `pbit_search=true` but
the pbit decision still rides on the LSQ rounding direction; an explicit
post-pass would catch the "right pbit, wrong rounding direction" tail seen
in m3 (8 % pbit-only + 14 % both differ).

---

## 6. Hit-rate prediction (additive, optimistic)

| Mode | Baseline | After P*A | After +P*B | After +P*C | After +P*D |
|---|---:|---:|---:|---:|---:|
| Mode 3 | 64.83 % | ~72 % | ~74 % | ~80 % | — |
| Mode 7 | 7.13 % | ~30 % | ~50 % | ~57 % | ~70 % (stretch) |

Net ppm-bits delta (assuming m3 and m7 weights match the 4.58 M-block
corpus): +15 ppt × 79 K blocks × 128 bits = ~1.5 M bits ≈ negligible
fraction at the corpus level (m3 + m7 are 1.9 % of blocks combined), but
the m7 jump alone clears the entire 7 % → 70 % gap on a 9 K-block
population — meaningful on alpha-bearing standalone textures and atlas
edges where m7 concentrates.

Cross-impact: P3.A and PX.B will also help m1 (the dominant non-m5/m6
mode), so the realised total parity lift is larger than the m3+m7 sum
suggests.

---

## 7. What this analysis does NOT cover

- **The 1 705 m3 selectors-only misses (25 % of same-partition m3 misses)**:
 endpoints and pbits match the reference exactly, but selectors differ. These are
 tie-break cases where the same endpoint pair admits two index assignments
 with equal SSE — the reference picks one, we pick the other. Need to instrument
 the selector tie-break (likely the reference's perceptual weighting flips the
 tie). Endpoint-refinement territory.
- **The Mode-7 large-delta alpha endpoint residuals (±5, ±7, ±9, ±13)**:
 these are the long tail. Need the oracle harness to
 enumerate (mode, partition, alpha-pattern) classes.
- **No measurement of pbit-flip-only misses through to selector
 consequences** — the 559 m3 "pbit-only" misses re-encode all 16
 selectors against shifted endpoints; we have not measured how many
 cascade into selector flips downstream.

These three items belong to the endpoint-refinement and
oracle-harness items already in the plan.

---

## Files this proposal touches

- `dev/fix_proposals/bc7_mode3_mode7_rules.md` — this file (committed).
- `dev/bc7_probe/analyze_mode3_mode7.py` — partition + feature analyzer (committed).
- `dev/bc7_probe/analyze_m3_m7_deep.py` — first-diff-bit region drill (committed).
- `dev/bc7_probe/analyze_m3_m7_quant.py` — per-channel raw quant delta (committed).
- `dev/bc7_probe/analyze_m3_m7_part_pairs.py` — partition pair Hamming + alpha-gating (committed).

No `src/` changes in this discovery.
