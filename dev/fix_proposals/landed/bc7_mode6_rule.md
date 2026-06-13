# BC7 Mode-6 rule discovery — W3 of bit-exact plan

> Status: discovery only — NO `src/` changes. Result of running
> `dev/bc7_probe/analyze_mode6.py` over the full 4.58 M-block W2 corpus
> (319 895 mode-6 blocks).

Baseline: **49.16 % byte-match** on mode-6 (`dev/bc7_probe/baseline_match_rate.md`)
— 162 647 misses, the largest single-mode residual. Decomposed as:

| Bucket | Blocks | % of Unity-m6 | Match? |
|---|---:|---:|---|
| Unity-m6, ours-m6 | 280 767 | 87.77 % | 56.01 % (123 519 miss) |
| Unity-m6, ours-m5 | 30 381 | 9.50 % | 0 % (all miss) |
| Unity-m6, ours-other | 8 747 | 2.73 % | 0 % (all miss) |

So mode-selection accounts for **39 128 misses (24 %)** of all m6 misses; the
remaining **123 519 misses (76 %)** are m6-vs-m6 divergences. We address the
two separately.

---

## Top rules (ranked by expected match-rate lift)

### Rule M6-A — Pbits both prefer 1 when in doubt (3.3:1 to 3.8:1 asymmetry)

**Strength:** strongest and cleanest signal. This is the single
most-explanatory rule.

H2 confusion table (full 280 767 m6-vs-m6 blocks):

```
o_pbits 00 01 10 11
u_pbits
00 69894 2735 1762 419
01 19102 44029 961 735
10 4554 1496 49206 2257
11 1408 5392 4855 71962
```

**pbit_lo disagreement net flow** (Unity vs ours):
- Unity=1, ours=0: **12 850 blocks** (sum of u="1X" rows × o="0X" cols)
- Unity=0, ours=1: **3 877 blocks**
- Ratio: **3.31 : 1 Unity-prefers-1**

**pbit_hi disagreement net flow:**
- Unity=1, ours=0: **26 326 blocks** (sum of u="X1" × o="X0")
- Unity=0, ours=1: **6 907 blocks**
- Ratio: **3.81 : 1 Unity-prefers-1**

Combined: across both pbits there are **39 176 net "Unity chose 1 where
we chose 0"** disagreements vs only **10 784 reverse**. **Adding a +ε
preference for pbit=1 in the non-pbit-search branch of
`find_optimal_solution`** (`bc7_pure.rs`:1735, :1742) would flip a large
fraction of these.

Conservative estimate (assume half the asymmetry is rule, half is noise):
**+15 000 byte-matches** (≈ +4.7 % m6).

### Rule M6-B — Pbit_lo correlates with `min(src_r) & 1` (secondary)

**Strength:** independent secondary discriminator, helps the cases M6-A
gets wrong.

H14 sample of 20 000 blocks:

| `min(src_r) & 1` | u_pbit_lo=0 | u_pbit_lo=1 |
|---|---:|---:|
| 0 (even) | 7 458 | 2 508 |
| 1 (odd)  | 3 395 | 6 639 |

The reference picks `pbit_lo = (min(src_r) & 1)` in 70.4 % of blocks. This is the
natural consequence of LSQ snapping the low endpoint to a value that
preserves the pixel-min LSB. Combined with M6-A, **the rule "prefer
pbit_lo=(min(src_r)&1), else prefer 1"** explains the dominant share of
pbit_lo divergence.

Independent of M6-A: another **+3 000-5 000 wins** for blocks where the
parity rule and the "prefer 1" rule disagree.

### Rule M6-C — Force mode-6 over mode-5 on RGBA-fully-opaque + low-variance blocks

**Strength:** structural mode-selection bug.

30 381 blocks (9.5 % of all Unity-m6) are encoded by us as **mode 5** instead.
Mode 5 = 1-subset RGBA with 7-bit color + 8-bit alpha; mode 6 = 1-subset RGBA
with 7-bit/7-bit. The current cascade (`handle_opaque_block`, `bc7_pure.rs`
:3563) tries mode 6 first, then later modes, with a `MODE6_PREF_SHIFT`
bias — but the bias is currently SHIFT=16 (≈1/65536, inert) for high-var
blocks and SHIFT=8 (≈1/256) for low-var. The fact that we ship **30 K
mode-5 blocks on Unity's mode-6 ground truth** says the bias is still
under-strength on the mode-5↔mode-6 boundary.

Strengthening the mode-6 bias (SHIFT=4 → 1/16 multiplicative preference)
when the alpha channel of the block is "interesting enough to merit
mode-6's index-prec" would flip a large share. Conservative: **+15 000 byte-
matches** (≈ +4.7 % m6).

Risk: this rule reaches into the mode-5 path. See "Risk" below.

### Rule M6-D — Index tie-break for all-off-by-one selector misses

**Strength:** secondary but clean.

Of the 40 755 selector-only misses (H8), **54.7 % (22 293)** have all selector
diffs of magnitude exactly 1. These are pixels exactly midway between two
interpolated palette colors where ours rounds one way and Unity the other.

The fix is in `evaluate_solution` (`bc7_pure.rs`:1195): when two adjacent
selectors `k` and `k+1` produce equal squared-error for a pixel, Unity
appears to pick the **larger** selector (rounds away from low endpoint).
Confirmation requires a per-pixel boundary probe (a follow-up); the
sign-bias evidence from H9 (hi-delta is asymmetric to-the-positive)
indirectly supports this.

If true: **+15 000 byte-matches** (≈ +4.7 % m6) for selector-only misses.

---

## Combined expected impact

| Rule | Class | Est. wins | Δ m6 match-rate |
|---|---|---:|---:|
| M6-A | pbit tie-break: prefer 1 | +15 000 | +4.7 % |
| M6-B | pbit_lo = min(src_r)&1 (secondary) | +4 000 | +1.3 % |
| M6-C | strengthen m6-over-m5 bias | +15 000 | +4.7 % |
| M6-D | selector tie-break rounds high | +15 000 | +4.7 % |
| **Total** | (assuming no overlap, optimistic) | **+49 000** | **+15.3 %** |

Estimated mode-6 match-rate post-fix: **49.16 % → 63-65 %**.

Mode-6 contributes 6.97 % of all BC7 blocks; a 15-point lift in m6 match-rate
maps to ~+1.05 % aggregate BC7 byte-match (94.17 % → ~95.2 %), or
roughly **−45 K ppm-bits at corpus scale**.

This is below the W5 plan target (95 % m6), so a follow-up endpoint-class enumeration is still required for the residual ~35 %
endpoint-true-divergence misses.

---

## Per-hypothesis raw numbers

Source: `dev/bc7_probe/mode6_analysis.md` (committed alongside this
proposal). Top-level numbers:

- 319 895 mode-6 blocks; 162 647 misses
- 87.77 % of Unity-m6 → ours-m6 (280 767 blocks, 56.01 % match)
- 9.50 % of Unity-m6 → ours-m5 (30 381 blocks, 0 % match)
- H2: pbit_lo agreement among misses = 86.5 %; pbit_hi = 73.1 %
- H4: first-diff byte spikes at byte 0 (41.3 %) and byte 8 (17.7 %) — both
 contain pbit boundary bits
- H7: of m6-vs-m6 misses, 67 % are endpoint-diff and 33 % are selectors-only
- H8: 54.7 % of selectors-only misses are all-off-by-one
- H9: hi-channel delta is positively biased — ours systematically picks
 the **high endpoint too low** (more `o_hi < u_hi` cases than the inverse)
- H10: has_alpha=True match-rate (50.94 %) ≈ has_alpha=False (47.38 %) —
 the alpha channel itself is not the dominant rule axis
- H13 surprise: byte-match blocks have **higher** L1 distance from pixel
 min/max than misses — confirms that low-variance / pixel-extreme-collapsing
 blocks are where the encoder has tie-break freedom
- H15: 0 endpoint-swap-equivalent blocks among misses — the mode-6 swap
 convention (`encode_bc7_block_mode6`, `bc7_pure.rs`:2926) is already
 Unity-compatible

---

## Proposed patches

All proposed changes are in `src/bc7_pure.rs`. No new files.

### Patch M6-A (pbit_lo parity bias)

In `find_optimal_solution` (`bc7_pure.rs`:1637), the no-pbit-search branch
(line 1710) picks `best_pbits[0]` by minimising `err0` per endpoint. Replace
the tie-break with: **prefer the pbit whose value matches the LSB of the
nearest-pixel channel-min** when the two error candidates are within an
epsilon.

```rust
// At bc7_pure.rs:1735-1740 (the pbit_lo selection in non-pbit-search):
// Before:
// if err0 < best_err0 { best_err0 = err0; best_pbits[0] = pp as u32;... }
// After:
// let p_min_r_lsb = pixels.iter.map(|p| p.c[0]).min.unwrap_or(0) & 1;
// let bias = if pp as i32 == p_min_r_lsb { 0.999 } else { 1.001 };
// if err0 * bias < best_err0 * bias_other {... }
```

Best implemented as a **pbit-LSB-prior bias** scaled by an ε similar to
`MODE6_PREF_SHIFT`. Affects modes 0/1/3/6/7 (those with pbits).

### Patch M6-B (pbit_hi prefers 1 on tie)

Same site as M6-A, second half (pbit_hi selection at `bc7_pure.rs`:1742).
On exact-tie or near-tie (within an ε), prefer `pp = 1`. Trivial change.

### Patch M6-C (strengthen mode-6 vs mode-5)

In `handle_opaque_block` (`bc7_pure.rs`:3563), the mode-5 trial currently
runs at `mode5_err` raw (no `nm6_score` wrapper, since mode 5 is not in
the m6-bias-shift table). The fix: apply `nm6_score(mode5_err)` (or a new
`nm5_score` that scales mode-5 err *up* by `err >> 4` to bias mode-6 wins
when within 6 %).

Implementation site: the mode-5 vs best_err comparison in `handle_opaque_block`
around `bc7_pure.rs`:3196-3220 (where `mode5_err` is computed and assigned
into `best_err`).

### Patch M6-D (selector round-toward-high tie-break)

In `evaluate_solution` (`bc7_pure.rs`:1195), the `best_sel` ladder at
lines 1295-1323 uses `if best_err == errs[k]` to pick selectors. When two
adjacent selectors tie, the later assignment wins (so selector 3 wins
over 2 on tie — which is already "round high"). But the **`n=16`
fast-paths** (lines 1245-1281, AVX2 + scalar) use `f * sum + 0.5` rounding
which is "round half away from zero, biased to low at exact.5". Replace
with `(f * sum + 0.5).floor` → already there — but verify the AVX2 path
is identical. Hypothesis: the AVX2 path differs at the boundary.

Implementation: audit `eval_solution_n16_rgb_avx2` and
`eval_solution_n16_rgba_*` vs scalar at exact-midway boundary. The
suspected difference is in the `_mm256_floor_ps` op vs scalar `.floor`.

---

## Risk analysis

### Modes affected by each rule

| Rule | Mode 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| M6-A pbit_lo parity | ✔ | shared-pbit (no-op) | — | ✔ | — | — | ✔ | ✔ |
| M6-B pbit_hi → 1 | ✔ | shared-pbit | — | ✔ | — | — | ✔ | ✔ |
| M6-C mode-6 bias up | — | — | — | — | — | **✔ (regression risk)** | ✔ | — |
| M6-D selector tie-break | ✔ | ✔ | ✔ | ✔ | ✔ | ✔ | ✔ | ✔ |

**M6-C is the riskiest.** Mode 5 is 78 % of all BC7 blocks and currently
matches 99.85 % — a regression of even 0.5 % there costs ~20 K
matches, dwarfing the m6 gains. M6-C must be gated by a feature filter
(e.g. "block has uniform alpha and low alpha variance and high color
variance") and validated against the pinset before merging.

**M6-A and M6-B are low-risk** — they affect tie-break only, in modes with
explicit pbits (0/3/6/7 of which only 6 has meaningful volume). Mode 1
uses shared-pbit (single pbit per subset) so the rule is naturally
inert. Modes 0/3/7 are <1 % of corpus each.

**M6-D requires an AVX2 audit** before any code change. If `_mm256_floor_ps`
truly diverges from scalar `.floor` at exact-midpoint, fixing it could
flip many mode-5 blocks too (mode-5 also runs `evaluate_solution`). Net
effect could be positive or negative; needs measurement.

### Composition risk

The prior reverts show that rules in isolation pass but
compose poorly. Each of M6-A through M6-D should ship behind a pinset
gate, and the order is:

1. **M6-D first** (audit only — no logic change unless AVX2 diverges).
2. **M6-A** (pbit_lo parity prior, gated; smallest scope).
3. **M6-B** (pbit_hi tie-break, gated; smallest scope).
4. **M6-C last** (mode-bias change, requires re-measurement of mode 5
 match-rate to ensure no >0.05 % regression there).

Each layer's `dev/bc7_probe/extract` rerun is the regression gate. If
total mode-6 match goes up by less than the predicted ±20 % of the rule's
est. lift, abort and re-analyze.

### Corpus generalisation risk

The 319 895 mode-6 blocks come from 102 distinct Texture2D in the W2 test
corpus. The validation corpus (302 entities, est. 6-12 M blocks) may have a
different mode-6 distribution. **M6-A's `min(src_r) & 1` rule** is content-
agnostic and should generalise; **M6-C's mode-5/mode-6 bias** is content-
sensitive and must be re-measured on the validation corpus before locking
the SHIFT value.

---

## Files referenced

- `dev/bc7_probe/analyze_mode6.py` — discovery script (committed)
- `dev/bc7_probe/mode6_analysis.md` — full per-hypothesis output (committed)
- `dev/fix_proposals/bc7_bit_exact_plan.md` — parent plan (W3 entry)
- `src/bc7_pure.rs`:1637 — `find_optimal_solution`, pbit-selection site for
 M6-A / M6-B
- `src/bc7_pure.rs`:3196-3220 — mode-5 path in `handle_opaque_block`,
 patch site for M6-C
- `src/bc7_pure.rs`:1245-1281 — `eval_solution_n16_*` SIMD paths, audit
 site for M6-D
- `src/bc7_pure.rs`:2923 — `encode_bc7_block_mode6`, the bit-packing layer
 (verified Unity-compatible — H15 = 0 swap-equivalent misses)
