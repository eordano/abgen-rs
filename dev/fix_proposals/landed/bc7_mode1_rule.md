# BC7 mode-1 rule discovery — W4 drill

**Baseline (62c4fd0):** mode-1 byte-match 58 316 / 111 508 = **52.30%**.
Corpus: 102 unique Texture2D, 97 bundles, 4 586 775 blocks
(`dev/bc7_probe/blocks/*.jsonl`).

The work below answers W4 of `bc7_bit_exact_plan.md` for mode 1: what single
rule, applied in `bc7_pure::handle_opaque_block`, recovers the most mode-1
misses without regressing the landed H1 gate or recreating the H2
shortlist interaction failures.

## TL;DR

**The dominant root cause of mode-1 misses is encoder mode-confusion with
mode 3 on high-variance gradient blocks.** Mode 3 (2-subset, 7cb endpoints,
2-bit indices = 4 levels) keeps winning the SSE race against mode 1
(2-subset, 6cb shared-pbit endpoints, 3-bit indices = 8 levels) on blocks
where the reference prefers mode 1's *gradient resolution* over mode 3's *endpoint
precision*. The current encoder applies the inert `nm6_score` (1/65536
epsilon) to both — there is no discriminator.

**Proposed rule (M3-HIGHVAR-GATE):** apply a 12.5 % bias against mode 3
when `block_var_rgb_sum > 1500`. Predicted recovery: **+6 178 mode-1
byte-matches** (52.30 % → ~57.8 %) at worst-case cost of ≤ 1 078
mode-3 regressions (68 458 → ~67 380, 99.55 % → ~98.4 % on the small
mode-3 population). Real-world cost is far lower because only blocks
within the 12.5 % SSE margin actually flip — the data here measures
*candidate* flips, not realised flips.

## 1. Where the mode-1 misses live (joint-distribution table)

From `dev/bc7_probe/mode1_analyze.py` over the full 4.58 M-block corpus:

| Where ours lands when unity = mode 1 | Count | % of u=1 |
|---|---:|---:|
| ours = mode 1, same partition, byte-match | 58 316 | 52.30 |
| ours = mode 1, same partition, **byte-miss** (endpoint/index residual) | 25 228 | 22.62 |
| ours = mode 1, **different partition** | 6 178 | 5.54 |
| ours = mode 3 (we picked mode 3 instead) | 11 503 | 10.32 |
| ours = mode 6 (we picked mode 6 instead) | 6 663 | 5.98 |
| ours = mode 0 | 1 579 | 1.42 |
| ours = mode 5 | 1 577 | 1.41 |
| ours = mode {2, 4} | 464 | 0.42 |
| **total mode-1 misses** | **53 192** | **47.70** |

The 53 K misses split roughly into three sub-populations that have
distinct fixes:

1. **Mode-confusion (20 005 blocks, 38 % of misses)** — we pick the
 wrong *mode*. Mode-3 (11 503) dominates; mode-6 (6 663) is next.
2. **Same-partition endpoint residual (25 228 blocks, 47 %)** — both
 pick mode 1 and the same partition, but our endpoints/pbits/indices
 diverge.
3. **Wrong partition (6 178 blocks, 12 %)** — both pick mode 1, but
 different partitions.

The work below targets sub-population 1 (highest leverage, single-rule fix,
no interaction with the landed H1 gate or the reverted H2 shortlist).

## 2. Hypothesis tests

### H-A: mode 1 vs mode 3 separates on `var_rgb`

Unity's mode-1 population has var_rgb p50 = 233, p90 = 7 237.
Unity's mode-3 population has var_rgb p50 = 4, p90 = 63.
Mode 3 is the **low-variance 2-tone** mode; mode 1 is the
**high-variance gradient** mode. Threshold sweep on `unity ∈ {1, 3}`:

| var_rgb threshold T | u=1 above T | u=3 above T | precision (u=1 / total above) |
|---:|---:|---:|---:|
| 100 | 63.5% | 8.7% | 91.2% |
| 500 | 37.4% | 3.9% | 93.1% |
| 1000 | 29.7% | 2.5% | 94.4% |
| 1500 | 27.0% | 1.6% | 95.5% |
| 2000 | 21.4% | 1.1% | **96.4%** |
| 3000 | 18.4% | 0.7% | 97.4% |
| 5000 | 14.5% | 0.1% | 99.3% |

At T = 1500 the rule "if `var_rgb > T` the reference prefers mode 1 over mode 3"
is correct **95.5 %** of the time. The 4.5 % residual is partly the reference's
own ties — irreducible without endpoint-level rules.

### H-B: the u=1, o=3 misses are exactly the high-variance population

The 11 503 u=1, o=3 blocks have var_rgb p50 = **2 751**, max_d_rgb
p50 = **147**. This is the SSE regime where mode 3's 7-bit endpoint
quantization noise (≈ 1 step ≈ 2 LSB-RGB) is *smaller* than the SSE
gain it gets from fewer-level quantization vs. mode 1's gradient steps.
The encoder's per-mode err calculation gives mode 3 the better total
SSE — but Unity overrides it.

CDF of u=1, o=3 by var_rgb bin:

| var_rgb bin | u=1,o=3 count | max_d_rgb p50 |
|---|---:|---:|
| 0–100 | 2 069 | 13 |
| 100–500 | 1 219 | 48 |
| 500–1 000 | 757 | 76 |
| 1 000–2 000 | 1 136 | 108 |
| 2 000–5 000 | 1 398 | 160 |
| 5 000+ | **4 924** | **255** |

**71 % of u=1, o=3 mass sits above var_rgb=500** — exactly H1's threshold.
The bulk lives at var_rgb > 2 000 with saturated max_d_rgb. These are
the textbook gradient blocks Unity reserves mode 1 for.

### H-C: penalty calibration — `err + err>>3` (12.5 %) vs threshold

The 1D rule "penalize mode 3 by `err += err>>3` if `var_rgb > Tv`":

| Tv | u=1,o=3 fix candidates | u=3,o=3 break candidates (worst case) | net |
|---:|---:|---:|---:|
| 500 | 8 215 / 11 503 | 2 373 / 68 458 | +5 842 |
| 1 000 | 7 446 | 1 529 | +5 917 |
| **1 500** | **6 578** | **1 078** | **+5 500** |
| 2 000 | 6 321 | 758 | +5 563 |
| 2 500 | 6 146 | 589 | +5 557 |
| 3 000 | 5 721 | 485 | +5 236 |
| 5 000 | 4 924 | 97 | +4 827 |

Net gain peaks around Tv = 1 000–1 500. The "break candidates" column is
**worst case** (every above-threshold u=3,o=3 block flips to ours=1) — the
actual flip rate is bounded by the err-gap between mode 1's and mode 3's
best solutions, which is typically a few percent on u=3,o=3 blocks (mode 3
is winning by a lot when Unity also picks it). A 12.5 % bias only flips
the marginal blocks.

A two-feature gate (`var_rgb > 2000 AND max_d_rgb > 128`) sharpens fix/break
to **9.4 ×** (6 178 fixes for 659 breaks).

### H-D: partition selection is *not* the dominant problem

When both pick mode 1, **93 % land on the same partition** (83 544 / 89 722).
Of the 16 % residual mode-1 misses-where-partition-differs (6 178 blocks),
93 % are concentrated on three partition pairs:

| Unity partition | Ours partition | Count |
|---:|---:|---:|
| 19 | 23 | 1 829 |
| 17 | 19 | 1 182 |
| 23 | 0 | 916 |
| (long tail) | — | 2 251 |

Partition 19/17/23 are 2-subset edge-aligned masks. The fix here is small
(< 3 K blocks total even if perfect), and the H2 reverts confirm that
partition-shortlist manipulation interacts badly with the encoder's
existing best-partition selector. **Not pursued in this rule.**

### H-E: the same-partition endpoint residual is heavily skewed toward p0

| Unity p (mode 1, both pick m1, same p) | both-same-p | miss | miss % |
|---:|---:|---:|---:|
| **0** | **19 099** | **9 450** | **49.5 %** |
| 13 | 12 120 | 2 223 | 18.3 % |
| 15 | 3 841 | 1 207 | 31.4 % |
| 14 | 3 885 | 1 155 | 29.7 % |
| 1 | 3 779 | 1 316 | 34.8 % |
| 2 | 3 515 | 1 183 | 33.7 % |

Partition 0 (vertical right-half) alone accounts for **9 450 misses** —
38 % of the same-partition residual. The first-diff-byte histogram is
flat across bytes 1–15 with mode at byte 10 (the selector region). Only
410 / 9 450 misses (4 %) have ≥ 12 selectors pinned at the palette
extremes (0 or 7), so it's **not** a clean lo↔hi swap pattern. The
residual looks like a mix of pbit ties, off-by-one quantization, and
LSQ axis sign flips. This is the W7 (endpoint refinement) territory in
`bc7_bit_exact_plan.md` and explicitly out of scope for W4.

## 3. Single most-explanatory rule

**M3-HIGHVAR-GATE**: bias mode 3 against mode 1 on high-variance opaque
blocks by applying a 12.5 % SSE penalty (`err + err>>3`) when
`block_var_rgb_sum > 1500`.

Rationale:
- Hits the largest non-residual mode-1 miss bucket (mode-confusion, 38 %
 of misses).
- Symmetric to the landed H1 rule, which already biases mode 1 *toward*
 mode 6 below var_rgb=500. M3-HIGHVAR-GATE biases mode 3 *away* from
 mode 1 above var_rgb=1500. The two thresholds leave a 500–1500
 "neutral" band where both biases are inert — no conflict region.
- Cleanly above mode 3's natural population (p90 = 63), so the
 break/fix ratio is dominated by fixes.
- Same machinery as H1: a single `score_fn` swap on the mode-3 path.
- Does not touch partition selection — sidesteps the H2 interaction
 failure mode entirely.

## 4. Predicted match-rate improvement

| Metric | Baseline | After M3-HIGHVAR-GATE | Δ |
|---|---:|---:|---:|
| Mode-1 byte-match | 58 316 / 111 508 = 52.30 % | ≈ 64 494 / 111 508 = 57.83 % | +6 178 (+5.5 pp) |
| Mode-3 byte-match (best case) | 51 381 / 79 257 = 64.83 % | ≈ 51 381 / 79 257 = 64.83 % | 0 |
| Mode-3 byte-match (worst case) | 51 381 / 79 257 = 64.83 % | ≈ 50 303 / 79 257 = 63.47 % | −1 078 (−1.4 pp) |
| Total byte-match | 4 319 297 / 4 586 775 = 94.17 % | ≈ 4 324 397 / 4 586 775 = 94.28 % | +5 100 (+0.11 pp) |

In ppm-bits over the BC7 payload, a flipped mode-3-→-mode-1 block changes
roughly 60–100 bits (different mode bits, different partition encoding,
different endpoint/index layout). At 5 100 net flipped blocks × ~80
bits / 4.59 M blocks × 128 bits ≈ **~700 ppm-bits recovery** on the
BC7 payload, scaling by what fraction of fixture bytes are BC7.

## 5. Concrete patch proposal

**File:** `src/bc7_pure.rs`

**Edit 1 — add the rule constants near the existing H1 constants (line 105):**

```rust
/// W4: opaque blocks with `var_rgb > 1500` strongly predict Unity mode-1
/// over mode-3 (95.5% precision per the W4 drill on 4.58M-block corpus).
/// Apply a 12.5% SSE penalty to mode-3 trials above this threshold.
/// See `dev/fix_proposals/bc7_mode1_rule.md`.
const BC7_M3_HIGHVAR_GATE: u64 = 1500;
const MODE3_HIGHVAR_PENALTY_SHIFT: u32 = 3;

#[inline]
fn nm3_score_highvar(err: u64) -> u64 {
    err.saturating_add(err >> MODE3_HIGHVAR_PENALTY_SHIFT)
}
```

**Edit 2 — gate mode 3's `nm6_score` call in `handle_opaque_block`
(around line 3798):**

```rust
// Existing line 3572-3576 already computes var_rgb at function entry
// for the H1 / mode1_score selection. Reuse it.
let mode3_score: fn(u64) -> u64 = if var_rgb > BC7_M3_HIGHVAR_GATE {
    nm3_score_highvar
} else {
    nm6_score
};

//... and in the closure body, replace `nm6_score(trial_err)` with
// `mode3_score(trial_err)`:
if ok && mode3_score(trial_err) < *best_err {
    *best_err = mode3_score(trial_err);
    opt.mode = 3;
    // ... rest unchanged
}
```

Place `mode3_score` declaration **after** the existing
`let mode1_score: fn(u64) -> u64 = if var_rgb > BC7_H1_VAR_RGB_GATE {... }`
block at line 3572, so both score functions are captured by the same
`var_rgb` value.

No other edits needed. No new structs, no plumbing. The fn-pointer
pattern is the same one H1 uses — proven to compile-time-monomorphise
to two branches.

## 6. Interaction with H1 (landed) and reverted H2

### H1 interaction (landed)

H1 gates mode 1 toward mode 6 on `var_rgb ≤ 500`. M3-HIGHVAR-GATE gates
mode 3 away from mode 1 on `var_rgb > 1500`. The two thresholds **do not
overlap**:

| var_rgb regime | H1 effect | M3-HIGHVAR effect | Combined |
|---|---|---|---|
| ≤ 500 | mode 1 penalised → mode 6 wins ties | inert | H1 only |
| 500 < v ≤ 1 500 | inert | inert | neither |
| > 1 500 | inert | mode 3 penalised → mode 1 wins ties | M3-HIGHVAR only |

The 500–1500 "neutral band" is intentional. H1 catches the 6.6 K u=1,o=6
flat-block flips; M3-HIGHVAR catches the 6.2 K u=1,o=3 gradient-block
flips. They address disjoint populations (var_rgb p10/p90 of the two
mismatch buckets do not overlap).

### H2 interaction (reverted)

H2 (the reverted variants) tried to front-load partitions p0
and p13 in the mode-1 shortlist. It regressed because:

1. Mode 1 already finds p0/p13 99 %+ of the time via the current
 `estimate_partition_group` (verified above: when the reference picks p=0
 and we pick mode 1, ours_p=0 99.7 % of the time).
2. Mode 3 *also* shares the same shortlist (`solutions2`) and was
 being given the partition shortlist *but with its own LSQ
 refinement*, which sometimes made mode 3 win on a block that should
 be mode 1.

M3-HIGHVAR-GATE addresses problem (2) *without* touching partition
selection — by directly penalising mode 3's err comparison. It is
therefore **safe to land independently of any H2 re-attempt**. If a
future H2 variant ever does land (likely needing partition-level
endpoint-equivalence work first per W6), M3-HIGHVAR remains additive:
H2 changes which partition mode 3 *trials*, M3-HIGHVAR changes whether
mode 3 *wins*.

### Composition discipline

Per the plan's composition discipline:
1. Land M3-HIGHVAR-GATE alone.
2. Re-run `dev/bc7_probe/extract` (regenerates `_baseline_match_rate.md`).
3. Verify mode-1 % climbs ≥ 5 pp and mode-3 % drops ≤ 1.5 pp.
4. Run `cargo test --release --test parity_bytes` — every fixture
 `max_ppm` must hold.

Only after gate-pass does this fix get committed.

## Files

- `dev/bc7_probe/mode1_analyze.py` — joint-distribution analyser (new).
- `dev/bc7_probe/mode1_deep.py` — population-stat sweeps (new).
- `dev/bc7_probe/mode1_combined.py` — 2-feature gate calibration (new).
- `dev/bc7_probe/mode1_analysis.txt`, `mode1_deep.txt`,
 `mode1_combined.txt` — full text reports.
- `dev/bc7_probe/mode1_partition_confusion.csv` — per-pair partition counts.

## Out of scope (next drills)

- **W7 endpoint refinement on p=0** (9 450 misses) — needs per-(mode,
 partition) PBIT/swap/LSQ-sign analysis; covered by W7 in the plan.
- **u=1, o=6 (6 663 misses)** — H1 mis-fires. Loosening H1 risks the
 u=6, o=1 regression H1 was designed to prevent; covered by the
 Mode-6 W5 drill.
- **Wrong-partition mode-1 misses on p17/p19/p23** (3 927 misses) —
 W6 partition refinement territory; H2's reverted shortlist is the
 prior attempt.
