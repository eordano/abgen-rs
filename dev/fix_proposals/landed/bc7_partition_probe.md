# BC7 partition probe — per-block Unity prod-bundle choice

## Goal

Drill the mode-1 vs mode-6 contest in Unity's prod-bundle BC7
encoder, on real Unity-produced bundles (not the editor TextureImporter
path probed previously). The standalone-texture residual is ~77 K ppm of
windows-bundle BC7 payload drift, with the previous corpus drill showing
107 K blocks `prod=m6 → ours=m1` and 92 K blocks `prod=m1 → ours=m6`.

## Tooling

Two new pieces, both research-only (no `src/` change):

- `dev/bc7_probe/extract.rs` — Rust binary that walks a directory of
 Unity-built bundles, parses each via the abgen `Bundle::load` API,
 reads every `Texture2D` typetree, and for those with
 `m_TextureFormat == 25` (BC7) extracts each 16-byte block from the
 `.resS` (or inline `image data`). It then decodes each block with a
 small hand-rolled BC7 decoder (`dev/bc7_probe/bc7_decode.rs` — header
 fields + 16 RGBA8 pixels via spec interpolation tables) and emits one
 JSONL record per block: bundle, texture, width/height, mip, block
 index, mode (0–7), partition (0–63), rotation, isb, 16 RGBA pixels,
 16-byte block hex, plus pre-computed features (per-channel mean,
 RGB variance, alpha variance, principal-axis 4-vector).
- `dev/bc7_probe/analyze.py` — feature aggregation: mode distribution,
 mode-1 partition histogram, mode-1 vs mode-6 separation hypothesis,
 partition prediction trial.

The Rust binary lives in its own `Cargo.toml` (path-dep on the parent
crate) so it neither pollutes `src/` nor ships as an example.

Run:

```
cd dev/bc7_probe
cargo build --release
./target/release/extract <ref-dir> out.jsonl
python3 analyze.py out.jsonl
```

## Corpus

Walked **two** Unity-produced reference directories:

| Source | Bundles | BC7 textures | Blocks |
|---|---|---|---|
| `/tmp/abgen-ref-out/test_windows` (live loop) | 6 | 1 (`image_0` 512²) | 21 847 |
| `tests/fixtures/parity/refs` (10 fixtures) | 10 | 6 (4 distinct, mac+windows pairs) | 262 154 |
| **Combined** | **16** | **7 (4 distinct)** | **284 001** |

Distinct textures (after deduping mac+windows pairs):

| Texture | Size | Mode dist |
|---|---|---|
| `bafkreihbgn43...` (atlas) | 1024² | m5=64%, m1=10%, m6=5%, m4=0.4%, m7=0.07% |
| `bafkreibxefote3...` | 512² | m5=63%, m6=24%, m1=6%, m3=6% |
| `bafkreie23rir...` | 512² | m5=74%, m6=24%, m1=2% |
| `image_0` (scene) | 512² | m5=100% |

The Unity-loop corpus has only 1 BC7 texture so far; the parity corpus
provides the majority of the signal.

## Findings

### 1. Mode distribution (284 001 blocks, all sizes)

| Mode | Count | % | Notes |
|---|---|---|---|
| 0 | 12 | 0.00% | 3-subset opaque — vestigial |
| **1** | **25 366** | **8.93%** | 2-subset opaque |
| 2 | 2 | 0.00% | 3-subset opaque, 5cb — almost dead |
| 3 | 2 500 | 0.88% | 2-subset opaque, 7cb |
| 4 | 880 | 0.31% | 1-subset RGBA, sep idx |
| **5** | **222 733** | **78.43%** | 1-subset RGBA, low-precision idx (dominant) |
| **6** | **32 346** | **11.39%** | 1-subset RGBA, full-precision idx |
| 7 | 162 | 0.06% | 2-subset RGBA |

The **mode-5 majority** confirms: most prod-bundle blocks are
low-variance / nearly-uniform (mean per-mode RGB variance for mode 5 is
**0.1** vs **925** for mode 6 vs **4 825** for mode 1). Mode 5 wins
trivially on flat blocks; mode 1 / mode 6 are the contested space.

### 2. Mode-1 partition concentration

Top-20 partitions of the 25 366 mode-1 blocks (cum %):

| Partition | Count | % | Cum % |
|---|---|---|---|
| 0 | 8 740 | 34.46 | 34.46 |
| 13 | 6 212 | 24.49 | 58.95 |
| 2 | 968 | 3.82 | 62.76 |
| 14 | 882 | 3.48 | 66.24 |
| 1 | 874 | 3.45 | 69.68 |
| 15 | 850 | 3.35 | 73.03 |
| 22 | 512 | 2.02 | 75.05 |
| 9 | 448 | 1.77 | 76.82 |
| 20 | 446 | 1.76 | 78.58 |
| 8 | 438 | 1.73 | 80.30 |
| 26 | 416 | 1.64 | 81.94 |
| 5 | 340 | 1.34 | 83.28 |
| 4 | 296 | 1.17 | 84.45 |
| 25 | 256 | 1.01 | 85.46 |
| 17 | 254 | 1.00 | 86.46 |

Partitions **0 and 13** account for **59%** of mode-1 blocks. Both are
"clean halves":
- p0 = `0011 0011 0011 0011` (vertical right-half)
- p13 = `0000 0000 1111 1111` (horizontal bottom-half)

The top-15 partitions cover 86%. This is consistent with edge-aligned
content (texel rows / columns), as expected for terrain / atlas textures.

### 3. Mode-1 vs mode-6 separator: alpha + RGB variance

```
mode1 fully-opaque (mean_a=255 AND var_a=0): 25 366 / 25 366 (100.0%)
mode6 fully-opaque (mean_a=255 AND var_a=0): 12 572 / 32 346 (38.9%)
```

**Mode 1 is unconditionally opaque** in this corpus — Unity never picks
mode 1 on alpha-varying blocks. Mode 6 splits ~39 / 61 opaque /
alpha-bearing. So the m1↔m6 contest is **only meaningful on opaque
blocks** (12 572 m6 candidates), and the discriminator is RGB variance:

| Threshold T | m1 recall (above T) | m6-opaque false-pos | precision |
|---|---|---|---|
| 100 | 85.1% | 26.2% | 86.8% |
| 200 | 83.9% | 25.7% | 86.8% |
| **500** | **81.2%** | **17.6%** | **90.3%** |
| 1000 | 78.3% | 16.3% | 90.6% |
| 2000 | 61.5% | 14.5% | 89.6% |

`var_rgb > 500` on opaque blocks predicts mode-1 with ~90% precision and
81% recall. Mean var_rgb for the m1 population is 4 825 (huge), for
opaque m6 is ~6 (mostly), with a long tail.

### 4. Partition prediction: best-mean-separating heuristic

For each of the 25 366 mode-1 blocks, score all 64 partitions by
squared distance between subset-0 and subset-1 mean RGB. Pick the
highest.

```
top-1 hit rate: 10 890 / 25 366 (42.9%)
top-5 hit rate: 21 928 / 25 366 (86.4%)
```

So the naive "best mean-split" rule lands the exact Unity partition
**43%** of the time, and lands within Unity's top-5 candidates 86% of
the time. This is encouraging — `bc7_pure.rs`
`estimate_partition_list_group(1, …)` does roughly this, then refines
via endpoint LSQ, which should add coverage. The big residual (~57%
top-1 miss) reflects ties or weak gradients where a perceptual /
chrominance-weighted score would tie-break differently.

## Proposed rule

Two **research** hypotheses, both bounded by the same corpus:

### H1 — gate mode-1 on var_rgb > 500 (precision: 90%)

In `bc7_pure.rs handle_opaque_block`, when comparing mode-1 best
solution vs mode-6 best solution, **bias toward mode-6 if `var_rgb <
500`**. Today `MODE6_PREF_SHIFT` already nudges mode-6 by a fixed
epsilon (see `bc7_mode6_epsilon.md`); H1 would replace the fixed epsilon
with a variance-gated bias.

Predicted parity recovery:
- Previous corpus drill: 107 K blocks of `prod=m6 → ours=m1` (we
 over-pick m1). If 90% of those have low var_rgb, the rule would flip
 ~96 K of them back to m6.
- Each flipped block changes ~16 bytes (the entire BC7 block payload).
 Most bit-flips concentrate in the endpoint + partition area (~50–80
 bits) since mode-6 vs mode-1 share *no* layout.
- At ~50 bits/block × 96 K blocks × ~bytes/block / total_bundle_bits,
 estimated ppm recovery: **30–50 K of the 77 K standalone-texture
 residual.**

### H2 — partition shortlist must include p0, p13 at the front

Mode-1 partitions 0 + 13 cover 59% of Unity's choices. Inspect the
current `estimate_partition_list_group(1, …)` partition-LUT seeds; if
either is gated out or sorted past the shortlist budget, frontload them.

Predicted parity recovery: bounded by `prod=m1 → ours=m1 but wrong
partition` count — not directly measurable from the current dataset
(needs ours-vs-prod block-level join, which is blocked on the Unity
loop completing). Likely smaller than H1 (single-digit K ppm).

## What the data DOES NOT show (limits of this probe)

- No `(prod, ours)` pairing yet. We can characterize Unity's choice
 conditional on pixel features, but cannot directly count flips
 attributable to each rule until the Unity loop finishes producing
 the full reference corpus and we re-run `abgen-corpus` to generate
 matching `ours/` bundles. The previous probe (synthetic + editor
 importer, `bc7_partition_lut_prod.jsonl`) is the wrong shape (editor
 encoder, not prod-bundle encoder); the new probe is the right shape
 but missing the `ours/` side.
- Corpus is only 4 distinct textures. Mode-1 partition concentration
 on p0/p13 may be artifact of the textures (terrain-y, grid-aligned).
 A 21-bundle re-run when the loop completes will multiply the corpus
 ~5×.

## Next steps (gated)

1. When the Unity loop produces ≥20 bundles, re-run `extract` and
 capture a `ours.jsonl` from a parallel `abgen-corpus` run. Build
 a per-block `(unity_mode, unity_partition, ours_mode, ours_partition)`
 join and **measure** the H1 / H2 prediction directly.
2. Implement H1 behind a `BC7_VARIANCE_GATE` const in `bc7_pure.rs`.
 Sweep threshold via `cargo test --release --test parity_bytes -- --nocapture`.
3. Iterate H2 if the partition-shortlist diff is non-trivial.

## Files

- `dev/bc7_probe/Cargo.toml`, `extract.rs`, `bc7_decode.rs`, `analyze.py`
 — extraction + analysis tooling.
- `dev/perf/bc7_blocks_agg.csv` — per-texture mode histogram (small).
- `dev/perf/bc7_partition_analysis.txt` — full analysis stdout.
- The 189 MB per-block JSONL (284 001 records) is NOT committed —
 regenerate via `./extract <dir> out.jsonl`.
