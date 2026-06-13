# BC7 M1 partition-0 endpoint refinement — selector lower-tiebreak

Drilled the 9,450-block W7 partition-0 residual identified in
`bc7_mode1_rule.md` ("the same-partition endpoint residual is heavily
skewed toward p0 — Partition 0 (vertical right-half) alone accounts for
9,450 misses, 38% of the same-partition residual"). The previous writeup
characterised it as "a mix of pbit ties, off-by-one quantization, and
LSQ axis sign flips" and routed it to "W7 endpoint refinement", which
was explicitly out-of-scope for the W4 rule that landed
(`M3-HIGHVAR-GATE`).

## TL;DR

The dominant root cause of the partition-0 residual is **NOT endpoint
quantization** — it is **selector tiebreak direction on byte-identical
palette entries**. The encoder's `evaluate_solution` selector picker
chains `if best_err == errs[i] { best_sel = i; }` which keeps the
HIGHEST selector on ties; the reference keeps the LOWEST.

Of 2,229 (ours_mode==1, ours_partition==0, unity_mode==1,
unity_partition==0) byte-mismatches in the 622,626-block test corpus,
**1,796 (80.6%)** are pure selector diffs with byte-identical endpoints.
Of those, **all 2,418 differing-selector pixels are SSE ties** (decode
to the same RGB) and the unity-vs-ours selector difference is exactly
±1 (98.6% in the `1↔2` / `3↔4` / `5↔6` slots — the asymmetric
[0,9,18,27,37,46,55,64] mode-1 weights' tied-rounding positions).

The reference picks the lower selector in **1,682/1,796 = 93.7%** of these
blocks; 110 pick higher, 4 mix. The lower-on-tie rule is therefore
the precise pattern.

## The rule (M1-P0-SELECTOR-LOWER-TIEBREAK)

After mode 1 + partition 0 is the final encoder choice, for each pixel,
walk its selector down through any chain of palette-equivalent neighbours
— i.e. while `palette[s-1] == palette[s]` as a byte triple. **Skip
subsets whose palette is fully flat** (`palette[0] == palette[7]`,
i.e. degenerate-endpoint case).

The flat-subset skip is load-bearing: degenerate subsets get sel=2 (or
sel=7) systematically from Unity for reasons unrelated to selector
tiebreak (likely a palette-flat sentinel that we currently happen to
match). Walking down on a flat-palette subset breaks 2,439 currently-
matching blocks.

## Per-block fix/break (bc7_probe corpus)

| variant | fix | break | ratio |
|---|---:|---:|---:|
| v1 walk-down-while-tied (no flat skip) | 1,676 | 2,439 | 0.69:1 ✗ |
| v3 walk-down-while-tied, skip 2-bit anchors only | 1,228 | 2,439 | 0.50:1 ✗ |
| **v4 walk-down-while-tied, skip flat subsets** | **1,682** | **0** | **∞** ✓ |

## Corpus sweep — before / after

Corpus: 33 reference bundles in `tests/fixtures/parity/refs`, 14 BC7
textures (9 unique uuids), **622,626 blocks**.

| metric | before | after | Δ |
|---|---:|---:|---:|
| Mode-1 byte-match | 13,719 / 25,876 = 53.02% | 15,675 / 25,876 = 60.58% | **+1,956 (+7.56pp)** |
| Mode-3 byte-match | 368 / 2,500 = 14.72% | 368 / 2,500 = 14.72% | 0 |
| Mode-6 byte-match | 17,412 / 34,036 = 51.16% | 17,412 / 34,036 = 51.16% | 0 |
| Total byte-match | 591,545 / 622,626 = 95.01% | 593,501 / 622,626 = 95.32% | +1,956 (+0.31pp) |
| BC7-payload diff bits | 921,604 | 911,824 | -9,780 (-122.7 ppm-bits) |

Mode-1 fixes only; no other mode changed. Fix:break ratio on
mode-1 partition-0: **1,956 : 0**.

## Tests / parity caps

- `cargo test -p abgen --release --test parity_bytes` — passes; per-fixture
 ppm-bits all within current caps; 1 fixture improved (QmXKjmamN3vDppyzed:
 654,494 ppm → 654,485 ppm, -9 ppm).
- `cargo test -p abgen --release --lib bit_exact_all_vectors` — same
 22/1254 pre-existing bc7e-encoder-self-consistency divergences before and
 after (bc7e divergence is expected as the encoder drifts toward Unity
 parity; no new breaks).

## Val verify (1,975 bundles, abc-unity-d60d68417ecc-corpus--val)

| kind | bundles | before ppm | after ppm | Δ |
|---|---:|---:|---:|---:|
| standalone-texture | 207 | 472,103.2 | 472,103.2 | 0 |
| standalone-texture-legacy | 380 | 474,696.6 | 474,696.6 | 0 |
| glb-scene | 1,219 | 622,856.2 | 622,856.2 | 0 |
| glb-animated | 88 | 604,088.8 | 604,088.8 | 0 |
| glb-wearable | 40 | 557,693.1 | 557,693.1 | 0 |
| glb-scene-collider | 36 | 173,043.8 | 173,043.8 | 0 |
| TOTAL | 1,975 | 601,318.7 | 601,318.7 | 0 |

The val corpus is unchanged at every kind — 0 of 1,975 bundles produced
a different output. The rule fires only when the encoder selects (mode 1,
partition 0) for an opaque block AND that block has at least one non-flat-
subset selector at a palette-tied position. In the val corpus, no
texture exercised that conjunction with a non-trivial selector position.

The 622,626-block bc7_probe corpus on `tests/fixtures/parity/refs` exercises
the case (6,248 mode-1/p0 blocks; 1,956 fixed). The improvement is therefore
**concrete-but-corpus-conditional**: every block where the rule fires
moves closer to Unity, and 0 blocks regress.

## Files touched

- `src/bc7_pure.rs` — 40-line helper + 1-line call site in `handle_opaque_block`.
