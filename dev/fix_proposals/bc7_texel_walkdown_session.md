# BC7 texel encoder bit-values — m6_walk_down_palette_eq_runs re-confirmation

> Status: NEGATIVE FINDING. No code change landed. Gate green at baseline.
> Area: RESEARCH_AREAS #9 (BC7 texel encoder bit-values), Tier B.
> Method: clean-room observation of reference bytes + black-box encoder probing.

## TL;DR

The mode-6 texel residual on **clean (byte-count-equal) standalone-texture
bundles** is **4 blocks / 26 bits** across the entire windows test-set, and a
near-identical tiny slice on mac. **None of the 4 are addressable by the
`m6_walk_down_palette_eq_runs` heuristic.** The heuristic has **zero effect on
the size-matched (clean-texel) slice on either platform** — toggling it
off leaves byte-id and size-matched diff-bits bit-for-bit unchanged. Its only
measurable corpus effect is on **size-mismatched** bundles via DXT/bundle
compression-size noise, which is not a texel-parity signal. Re-confirmed the
prior conclusion that this area has no clean mode-6 signal to exploit.

## Corpus methodology

Per the brief, filtered standalone-texture to **byte-count-equal** pairs so
texel diffs aren't masked by size mismatches. Built ours from the test-set
reference (`ad0564d-windows`) via `abgen-corpus --from-reference`, verified
with `abgen-verify`.

standalone-texture total: 2440 bundles.
- size-matched: 240 (mac: 241)
- byte-identical: 233
- **size-matched but differ: 7 bundles, 170484 bits**

The other 2200 bundles are size-mismatched (1212 smaller + 988 larger) — those
diffs are mode-selection / mip differences that change block byte counts, not
mode-6 texel values, and cannot be block-aligned for texel comparison.

## Per-block decomposition of the 7 size-matched-diff bundles

Decoded BC7 image data from both bundles (Texture2D class 28, "image data"
field), mode-classified every 16-byte block, tallied (ours_mode, ref_mode):

| bundle | diff blocks | mode-pair breakdown |
|---|---|---|
| bafkreie3jlg5... (dominant, 163850 cmp-bits) | 9 | (5,4)×5 (7,7)×1 (5,7)×1 (5,6)×1 (6,1)×1 |
| bafkreigkgkxsxo... | 3 | (7,7)×1 (6,6)×1 (7,6)×1 |
| bafkreia4fonpkk... | 2 | (6,1)×1 (5,7)×1 |
| bafkreifniucl75... | 2 | (7,7)×1 (5,6)×1 |
| bafkreidaid5gu6... | 1 | (4,4)×1 |
| bafkreigqaf6evp... | 4 | (6,6)×2 (4,4)×1 (7,7)×1 |
| bafkreicw4pgimb... | 1 | (6,6)×1 |

Total mode-6-vs-mode-6 (the texel area) divergences across the **entire**
size-matched corpus: **4 blocks, 26 bits**. The bulk of the residual is
**mode selection** (5↔4, 5↔6, 5↔7, 6↔1, 7↔6) — consistent with the
`bc7_mode6_epsilon.md` finding that the dominant texture gap is mode-5
over-pick / 2-subset mode selection, not mode-6 texel values.

## The 4 mode-6 texel blocks, decoded

(little-endian mode-6 decode: 7+7+7+7 RGBA endpoints ×2, 2 pbits, 3+15×4 idx)

1. **gkgkxsxo blk 21845 (18 bits)** — R=G=B=0, only **Alpha** varies
   (lo 39 hi 17, monotone, 16 distinct palette entries → **no equal runs**).
   7 alpha-index diffs. Heuristic's `run_size>=min_size` snap can never fire
   (all runs size 1). Pure alpha index-search divergence.
2. **gqaf blk 240 (3 bits)** — endpoint+pbit: G lo 111→110, A hi 55→54,
   pbit_hi 0→1. Zero index diffs. Joint endpoint/pbit search tie.
3. **gqaf blk 312 (1 bit)** — endpoint: B hi 127→126. Zero index diffs.
   Single endpoint LSB at the 127 clamp boundary.
4. **cw4p blk 5461 (4 bits)** — identical endpoints/pbits; palette is two
   runs of 8 (RGB 212/213, A 254/255). 4 index diffs where pixels 8–11
   sit at the **run boundary**: ours assigns them to the high run, Unity to
   the low run. The walk-down heuristic *does* run here (nruns=2≤6,
   pal_alpha_varies=true → min_size=2, all runs ≥2 so every selector snaps
   to its run start) — but the snap is not the disagreement. Both ours and
   ref are already snapped to run starts (0 or 8); the disagreement is the
   **upstream run choice** in `evaluate_solution`, a midpoint-rounding tie at
   the 212.5 boundary. Tuning the heuristic cannot move it.

So: 1 alpha-index, 2 endpoint/pbit, 1 run-boundary-assignment. **None is a
walk-down-heuristic decision.** 3 of 4 involve the alpha channel.

## Heuristic toggle experiment (ABGEN_WALKDOWN=off, since reverted)

Added a temporary env-gated bypass to `m6_walk_down_palette_eq_runs` and
measured full-corpus verify ON vs OFF:

| platform | metric | ON (baseline) | OFF |
|---|---|---|---|
| windows | standalone-texture diff-bits | 1138407360 | 1138289769 (−117591) |
| windows | **size-matched diff-bits** | **170484** | **170484 (0)** |
| windows | size-matched byte-id | 233 | 233 |
| mac | standalone-texture diff-bits | 1137068814 | 1136964097 (−104717) |
| mac | **size-matched diff-bits** | **1049372** | **1049372 (0)** |
| mac | size-matched byte-id | 233 | 233 |

The size-matched (clean-texel) slice is **bit-for-bit identical** ON vs OFF on
both platforms. The −117k/−105k full-corpus deltas come entirely from
size-mismatched bundles where changing index bytes changes the DXT/bundle
compressed size — compression noise, not a parity win, and not trustworthy
(verifying texel parity on size-mismatched bundles is exactly what the brief
warns against).

Pinset probe: of 72 mode-6 entries in `tests/fixtures/bc7_pinset.jsonl`, the
heuristic ON vs OFF gives **identical** results (36 matches, same set) — it
never fires on any pinset mode-6 entry either.

## Why the heuristic still must stay

`bc7_pure::tests::m6_walk_down_palvar_a_low_nruns_fixtures` (3 real Unity
blocks with alpha-varying, low-nruns palettes) FAILS with the heuristic off.
Those block shapes are not present in the current test-set's size-matched
slice, but they are genuine Unity outputs the heuristic was built to match.
The "69% right / 31% wrong on top-run case" the area brief cites is not
visible in this corpus's clean slice — it would require a corpus that
surfaces those palette shapes in size-matched bundles. Do not flip or weaken
the heuristic on this corpus's evidence: it is neutral here and load-bearing
for the pinned fixtures.

## Conclusion / next direction

This re-confirms the prior pass: **no clean mode-6 texel signal** to exploit
on the size-matched standalone-texture corpus. The 26-bit clean residual is
4 isolated search-boundary ties (alpha index, endpoint/pbit LSB,
run-boundary assignment) with no shared block-local discriminator that
isn't already at the rounding-tie level. The real standalone-texture parity
mass is **mode selection** (mode-5 over-pick vs mode-4/6/7 and 2-subset
modes) — the direction `bc7_mode6_epsilon.md` already flagged: a per-block
"rotation worth it" / "disable mode-5 on low-variance opaque" structural
rule, not a mode-6 texel tweak. That is a different area (mode selection),
not BC7 texel bit-values.

## Files

- `examples/bc7_blockdiff.rs` (new) — per-block BC7 mode-diff tool: decodes
  Texture2D image data from two bundles, mode-classifies each 16-byte block,
  reports (ours_mode, ref_mode) counts. `BD_VERBOSE=1` dumps per-block hex.
  Reusable for any future BC7 texel investigation.
- `src/bc7_pure.rs:3854` — `m6_walk_down_palette_eq_runs` (unchanged).
