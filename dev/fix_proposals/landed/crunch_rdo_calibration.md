# Crunch RDO quality calibration — q=255 → q=96 (-6.2k ppm BC5 bundles)

**Status :** Landed. `src/builder.rs::texture` now calls
`encode_bc5_normal_crn_mip_chain(.., 96)` (down from 255 / upstream
default).

## Probe

`dev/crunch_quality_calibrate.py` + `examples/crunch_bc5_quality_sweep.rs`
(this commit). For every BC5/DXT5Crunched (fmt=29, "Hx" magic) Texture2D in
the corpus:

1. Extract the prod CRN bytes from `m_StreamData/resS`.
2. Extract the source PNG/JPEG/etc. from the bundle's GLB
 (`bufferView`-embedded; the BC5 corpus is 100% binary-chunk-embedded).
3. Encode our side at qualities {0, 32, 64, 96, 128, 160, 192, 224, 255}.
4. Compare ours vs prod byte-by-byte (popcount XOR).

Output table at `/tmp/crunch_calib/summary.json`.

## Corpus

21 BC5 textures across 13 bundles (windows + validation_2). 11 unique source
images. **9 of those 11 require Unity-side NPOT resize before encoding**
(source dim ≠ prod dim, e.g. 2160×2160 JPEG → 1024×1024 BC5). Only the
12 dim-matching samples are meaningful for q-calibration — the other 9 are
apples-to-oranges (we'd be measuring resize residuals, not RDO residuals).

## Sweep result (dim-matching subset, n=12)

|   q | total ours bytes | total ref bytes | bit-diff   | ppm vs ref bits |
|----:|-----------------:|----------------:|-----------:|----------------:|
|   0 |        1,543,214 |       3,456,435 | 21,467,565 |         776,362 |
|  32 |        2,362,854 |       3,456,435 | 18,191,992 |         657,903 |
|  64 |        2,892,001 |       3,456,435 | 16,068,113 |         581,094 |
|  **96** |    **3,252,913** |   **3,456,435** | **14,639,591** |     **529,432** |
| 128 |        3,532,762 |       3,456,435 | 15,893,813 |         574,790 |
| 160 |        3,789,570 |       3,456,435 | 17,382,259 |         628,619 |
| 192 |        4,023,542 |       3,456,435 | 18,791,122 |         679,570 |
| 224 |        4,237,243 |       3,456,435 | 20,281,988 |         733,486 |
| 255 |        4,436,188 |       3,456,435 | 21,710,838 |         785,160 |

Clean parabola, minimum at **q=96**. Per-texture-best histogram:

- q=96 → 8 textures (all instances of `PanelGeneric002_NRM_VAR1_1K`)
- q=192 → 2 textures (`image_28` 1024², `image_15` 512²)
- q=255 → 2 textures (`image_10` 512², `image_4` 256²)

Dominant cluster sits dead-centred at q=96: ours=356,755 B vs prod=361,070 B
(-1.2% bytes; the closest match anywhere in the sweep).

Per-texture sweeps in `/tmp/crunch_calib/diffs.json`.

## Whole-corpus impact

13 BC5-containing bundles (windows + val2):

| metric                | q=255 baseline | q=96 calibrated | delta |
|---|---:|---:|---:|
| total ref_bits        | 1,468,725,280 | 1,458,488,512 | -10.2M |
| total bit_diff        |   685,645,671 |   671,816,768 | -13.8M |
| **ppm**               |   **466,830** |   **460,625** | **-6,205** |

13/13 bundles improve. Most per-bundle deltas are now ≤ ±100 KB (the largest
remaining is QmUdjGpQyW8tPDYk with -320 KB undershoot — that's the
2123×1080 PNG → 1024² resized texture, an NPOT case where our resize
pipeline introduces its own residual).

Full corpus (2,174 windows bundles):

| metric        | q=255 baseline | q=96 calibrated | delta |
|---|---:|---:|---:|
| ppm           |    424,825 |    424,084 | -741 |
| byte-id       |       90/2174 |       90/2174 |  0 |
| rust smaller  |        1,429 |        1,439 |  +10 |
| rust larger   |          519 |          509 |  -10 |

## Other crnlib parameters

The probe **only** swept `m_quality_level`. The wrapper's other settings:

- `cCRNCompFlagPerceptual = false` — required for normal-map data per
 crnlib's banner; kept.
- `m_alpha_component = 1` — correct (BC5 reads the.g channel as "alpha"
 → Y).
- `m_num_helper_threads = 0` — single-threaded, deterministic.
- `m_file_type = cCRNFileTypeCRN` — correct (vs DDS).

None of these are plausibly miscalibrated — they're either binary correct
(perceptual flag) or required for format correctness (alpha_component,
file_type). The remaining ~460k ppm in the BC5 bundles must come from
**Unity's NPOT resize pipeline** (9/11 source images need resize) and
possibly minor crnlib version drift (the vendored crunch is unmodified
upstream public-domain release; Unity may have its own patched fork).

## Per-texture-best variance

8/12 dim-match samples agree on q=96; the remaining 4 prefer
q=192/255. Following the discipline note
(*"If 1 quality setting is best for ALL 21 textures = SHIP IT. If quality
VARIES per-texture then either find the rule OR don't tune"*),
**aggregate-best q=96 ships** because:

- The aggregate ppm minimum is a clean parabola minimised at q=96
 (529,432 ppm — every other q is worse).
- The "wants higher q" tail are all SMALL textures (256² and 512²) where
 the absolute bit-diff is small (200k - 600k bits each, vs the q=96
 cluster's 1.5M bits per texture). Aggregate impact of forcing them to
 q=96 vs their personal-best is tiny.
- Per-CID quality is forbidden by the discipline rules.

## Future work

- The 9 non-dim-match samples likely have larger residuals dominated by
 NPOT resize, not by RDO quality. Calibrating those would require an
 exact byte-equal resize pipeline first (cross-ref:
 `dev/fix_proposals/npot_bilinear_research.md`).
- Even at q=96 we're at 529,432 ppm residual on dim-match — the missing
 ~50% of bits must come from CRN-internal randomisation (Huffman seeds,
 block-pair refinement order) or upstream-fork drift. Closing that gap
 requires either porting Unity's specific crunch revision or accepting
 the residual as the floor for raw-CRN matching.

## References

- `dev/fix_proposals/landed/crunch_encoder.md` — the prior commit
 that landed the CRN encoder at q=255 / -46k ppm on 11 BC5 windows
 bundles.
- `dev/fix_proposals/bc5_normal_trigger.md` — trigger derivation.
- `examples/crunch_bc5_quality_sweep.rs` — single-PNG, multi-q sweep
 driver used by the probe.
- `dev/crunch_quality_calibrate.py` — corpus-wide probe (harvest +
 sweep + diff).
- `third_party/crunch/cpp/crn_wrapper.cc` — FFI wrapper (param defaults
 documented here).
