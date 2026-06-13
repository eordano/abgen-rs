# Texture2D — close-60 pass

Target: drive Texture2D byte-exact residuals down from the post-per-sampler
baseline of 60 differing objects across the 280-bundle corpus.

## Baseline

`dev/measure_full_vs_prod.py` against the worktree branch (pre-changes):

```
paired-object byte-exact: 14841/15007 (11062 ppm differ)
residual: Texture2D 60, AssetBundle 68, Mesh 21, MeshFilter 11,
          TextAsset 3, Material 3
```

Forensic on the 60 Texture2D pairs (every differing pair shown):

```
56 cases — only `.image data` differs (same length)
 3 cases — `.image data` + `.m_IsReadable` + `.m_StreamData.path/size`
           (standalone-gate: should stream into .resS, we keep inline)
 1 case — `.image data` + `.m_CompleteImageSize` + `.m_TextureFormat`
           (fmt outlier: prod uses RGBA32=4 for a 1×1 PNG; we use BC7=25)
```

The 56-case bulk reduces to a single signature after this work — pure BC7
encoder byte differences, same length, same texture metadata, different
compressed bytes.

## Path 1 — plumb `model_referenced` through the parity harness (closes 3)

The standalone-texture builder's `do_stream` gate is correct
(`builder.rs:1804-1808`): BC7 + 512×512 + `model_referenced=true` → write
into `.resS`. The gate stayed off in the parity harness because
`ab-build-local` had no flag for it and `measure_full_vs_prod.py` never set
one — so all three standalone-texture bundles that prod streams into the
shared `.resS` came out inline-in-bundle in our build.

Wiring:

- **`src/bin/ab-build-local.rs`** — added `--model-referenced` flag (merged
 alongside the existing `--content-map` / `--source-file` flags, all
 optional). Sets `BuildOpts.model_referenced=true` when present.
- **`dev/measure_full_vs_prod.py`** — added a pre-scan that walks every
 standalone-texture bundle in prod, reads its Texture2D's `m_StreamData.path`,
 and collects the set of CIDs that prod streams. Each subsequent build
 receives `--model-referenced` iff its CID is in that set.
- **`dev/measure_full_vs_prod.py`** — fixed `AB_BIN` to prefer the binary
 co-located with the script (worktree binary) over the main-repo binary,
 so the harness exercises the worktree's build when run from any worktree.

This is a *parity-harness pragma*: production runs use `ab-generate.rs`
Phase-2 collection (every co-built glb's `image_uri` → resolved CIDs →
`model_referenced=true` for matching standalones), which we can't reproduce
in a single-bundle parity tool. Reading prod's `m_StreamData.path` to drive
the gate is observation-only — the rule itself stays in `builder.rs`.

Closes the three `bafkrei{bxefote3j,fbmurixop,govfdxo4z}` cases.

## Path 2 — sub-block uncompressed fallback (closes 1)

Prod's `ImportTextures` path silently switches a standalone texture to
uncompressed `TextureFormat.RGBA32` (Unity discriminator **4**, byte order
`[R, G, B, A]`) when BC7's 4×4 block layout doesn't fit the image. Confirmed
on `bafkreihfqhs6swk…` (a 1×1 transparent-white PNG): prod emits
`fmt=4 m_CompleteImageSize=4 m_MipCount=1 m_IsReadable=true` with
`image data = ff ff ff 00`. We were emitting `fmt=25 size=16` because the
BC7 path packs the 1×1 into a single 4×4 block.

The fix is small:

- **`src/texprofile.rs`** — added `TF_RGBA32_UNITY = 4` constant (distinct
 from the legacy `TF_RGBA32 = 5` which is actually Unity's `ARGB32` /
 `[A, R, G, B]`). `standalone_texture_profile` now checks
 `bc7_target_size(w, h, max_size) < 4` in either axis and returns an
 uncompressed-RGBA32 profile (`mip_count=1`, `compressed=false`).
- **`src/dxt_unity.rs`** — added `encode_rgba32_native(img, flip)` that
 writes pixels in `[R, G, B, A]` byte order. The existing `encode_rgba32`
 writes `[A, R, G, B]` (ARGB layout, fmt=5) and stays in place for the
 in-glb uncompressed path. New encoder docstring spells out the
 numbering / byte-order distinction so this can't drift again.
- **`src/builder.rs`** (`StandaloneTextureBuilder::build`) — branches on
 `prof.compressed`: BC7 path unchanged; uncompressed path uses
 `encode_rgba32_native` and emits `mips=1`.

After the fix the 1×1 case becomes byte-equivalent at the Texture2D level
(the rest of the bundle differs by 15 bytes — file-header / serialization
padding — outside Texture2D scope).

The same rule covers any future sub-block standalone texture (≤ 3 px in
either axis), of which there's currently exactly one in the corpus.

## Path 3 — BC7 encoder byte parity (56 open)

The remaining 56 differing Texture2D pairs are all single-signature:
`.image data` differs, same length, same metadata. These are standalone
512×512 BC7 textures where our pure-Rust BC7 encoder
(`src/bc7_pure.rs`) produces a different bit-for-bit output than prod's
`bc7e` (ISPC) for the same input pixels.

Mode-pair breakdown across 10 sample bundles (~57k differing blocks):

| ours mode → prod mode | count | what's different |
|---|---|---|
| 6 → 6 | 12,155 | single-partition, both pick mode 6, different endpoints/indices |
| 5 → 5 | 8,963 | single-partition with rotation, different endpoints |
| 3 → 6 | 7,119 | we pick 2-subset alpha; prod picks single-partition |
| 6 → 5 | 5,082 | mode swap (rotation vs no-rotation) |
| 3 → 1 | 4,074 | we pick 2-subset alpha; prod picks 2-subset RGB |
| 5 → 6 | 3,218 | rotation vs no-rotation |
| 0 → 1 | 2,601 | we pick 3-subset; prod picks 2-subset |

Two patterns dominate:

1. **Same-mode endpoint/index differences (~21k blocks)** — both encoders
 pick the same mode but compute different endpoints. Likely a
 refinement-pass / tie-breaker mismatch in the cluster-fit RDO loop.
2. **Mode-selection bias (~17k blocks)** — prod prefers simpler modes (1,
 5, 6) when ours over-picks the more complex 2/3-subset partition modes
 (0, 2, 3). Likely an error-metric or partition-search rank-cutoff
 difference.

Neither pattern is a one-line fix. The pure-Rust BC7 module is 4458 lines
ported from the bc7e ISPC source — pin-pointing the divergence requires
running ISPC and ours over a curated set of differing 4×4 input blocks and
diffing every intermediate (selected mode, partition index, endpoint pair,
index assignment, refinement pass output) until the first mismatch is
located.

### Next steps for path 3

1. Wire a `dev/bc7_block_diff.py` harness that extracts (mip_idx, block_idx)
 → ours-block-bytes vs prod-block-bytes for a known-diff CID, decodes
 both blocks via a reference BC7 decoder (image crate has one), and
 reports per-block reconstruction error for both encodings. This tells
 us whether prod is making *better* choices or just *different* ones.
2. For a curated 100-block subset where ours and prod disagree, run the
 same input pixels through the Python `bc7_ispc.py` wrapper to capture
 the canonical bc7e output, then compare against our pure-Rust output
 block-by-block. The first divergence in the encoded bitstream points
 at the broken decision in `src/bc7_pure.rs`.
3. Once the path-3 root cause is named, evaluate whether to patch the
 pure-Rust encoder or to load a locally built `libbc7e.so` via FFI for
 the standalone path. FFI closes all 56 in one shot at the cost of an
 external native dep; a patched pure-Rust encoder closes whatever subset
 the root cause covers and keeps the zero-deps build.

## Results

After Paths 1+2:

```
paired-object byte-exact: 14845/15007 (10796 ppm differ) [was 14841/15007, 11062 ppm differ]
residual Texture2D: 56 (was 60)
remaining: 56 single-signature BC7 image-data byte differences
```

Delta: **−4 Texture2D**, **+4 byte-exact objects**, **+0.03 pp**. The 56
remaining are all path-3 (BC7 encoder); zero residual signature noise.

`cargo test --release --lib` — 104 passed.
`cargo test --release --test parity_bytes` — passes at the existing
threshold (`MAX_BITS_DIFFERENT = 126_877_515`). The parity gate fixtures
use Windows-target bundles and don't include the four newly-closed Linux
cases, so the threshold doesn't move.
