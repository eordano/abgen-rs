# standalone-texture mid-size deltas (1k–64k) — SOURCE diagnosis

> **Status: research / negative finding.** The 1628-bundle mid-size cluster is
> **99.9 % LZ4 compression noise downstream of BC7 texel values** (irreducible
> in this area; the known soft wall). Exactly **2 bundles** (one unique source,
> a `.psd`) are structural — and that one is a decoder-coverage gap, not a
> texture-size/mip/split bug.

Data: `/tmp/abgen-val300-integrated` (ours) vs
`unity-reference-ab/ad0564d-val300-windows` (truth);
`/tmp/abgen-val300-integrated-report.json`.

## Cluster definition and split

`kind == standalone-texture`, `1024 < |ours_bytes - ref_bytes| <= 65536`:

| group | count | evidence |
|---|---:|---|
| **mid-size cluster total** | **1628** | report filter |
| ↳ raw uncompressed payload **byte-identical in length** (LZ4 noise) | **1626** | `dump_decomp` raw-size census |
| ↳ raw uncompressed payload **different length** (structural) | **2** | same census |

The 1626 split both directions — **981 ours>ref, 645 ours<ref** — which alone
rules out a single-directional structural bug (a wrong-length field or a
mis-set split threshold would skew one way). Total on-disk delta carried by
the 1626 noise bundles: **10.06 MB**, mean |Δ| 6187 B, median 4034 B.

## Method — raw-size decomposition (the decisive test)

For every bundle the brief's decomposition rule was applied: decompress to the
raw CAB block with `examples/dump_decomp` and compare **uncompressed** sizes.

- **equal raw size + different on-disk size ⇒ compression noise**
- **different raw size ⇒ structural**

Census over all 1628 (`/tmp/decomp_census.py`):
`raw_equal = 1626, raw_diff = 2, missing = 0`.

### Worked examples (the two the brief names)

```
bafkrei…ttxy / bafkreiad4mn…xble_windows   Δ disk +1588
  on-disk: ours 11574  ref  9986
  raw CAB: ours 92588  ref 92588   (IDENTICAL)
  objalign: TextAsset 80=80, AssetBundle 348=348, Texture2D 87580=87580 (DIFF)
  raw byte-diffs: 9500 bytes, all in offsets 44445–92561 (the image-data region)

bafkrei…ttxy / bafkreibtr3k…p37a_windows   Δ disk +1525
  on-disk: ours 11482  ref  9957
  raw CAB: ours 92592  ref 92592   (IDENTICAL)
  objalign: AssetBundle 348=348, Texture2D 87580=87580 (DIFF), TextAsset 80=80
  raw byte-diffs: 9374 bytes, all in offsets 26910–92481 (the image-data region)
```

In both, every object length matches exactly (Texture2D included), the raw
SerializedFile is byte-length-identical, and the raw byte differences sit
entirely in the back-half BC7 image-data block. A 40-bundle random sample
(`/tmp/region_census.py`) confirmed **0/40 have any diff in the first 4 KB**
(SerializedFile header + typetree + object metadata) — **40/40 are
image-data-region-only**.

## THE SOURCE (for the 1626-bundle majority)

The byte size of a standalone-texture bundle is set by, in order:

1. `m_Width/m_Height`, `m_MipCount`, `m_TextureFormat`, `m_CompleteImageSize`,
   `m_IsReadable`, and the inline-vs-`.resS` split — **all already
   byte-correct** (re-confirmed here: every object length matches, raw payload
   length matches, header region byte-identical; consistent with the prior
   `standalone_texture_size_session.md` field census of 0 mismatches).
2. **The BC7-encoded texel bytes themselves**, then **LZ4HC** compressing those
   bytes to the final on-disk length.

The divergent component is **(2) BC7 mode/partition/endpoint/selector choice
per 4×4 block**. abgen-rs's pure-Rust encoder (`src/bc7_pure.rs`, driven from
`src/builder.rs::encode_texture_bc7`) produces *different texel bytes* than
the converter's bc7e-ISPC output (bc7e is the open-source Intel/GameTechDev
encoder the converter invokes) for the same source pixels — same dimensions, same
mip chain, same total byte count, **different block contents**. LZ4HC then
compresses those differing texels to a different length, which is the entire
on-disk delta. This is the same single-signature `image data`-only residual
documented in `landed/texture2d_followup.md` and
`landed/texture2d_residual_v3.md §"What's left" #1` (bc7e-ISPC vs pure-Rust
refinement-pass ordering / tie-break).

The `.resS` split threshold, mip count, and `m_CompleteImageSize` are **not**
the source — they are provably correct here.

## RECOVERABLE vs IRREDUCIBLE

- **IRREDUCIBLE in this area: 1626 / 1628 (99.9 %).** Pure BC7-texel → LZ4
  length noise. There is no size/mip/split/length field to fix; the byte count
  is already right. The only way the on-disk size moves is by making the BC7
  texel bytes bit-identical to the converter's bc7e output, which is the separate
  BC7 encoder-parity workstream (`bc7_*` landed docs, `bc7_texel_walkdown_session.md`).
  Closing texels would close these deltas as a side effect, but it is **not
  independently fixable as a "size" problem** and is out of this area's scope
  per the brief.

- **STRUCTURAL & RECOVERABLE: 2 / 1628** — both are the same single source
  asset, `bafybeigqu3tcuf4jgnugdbgrak45ieuvh4qusqhybe5ppwbftm4tk7z5ay_windows`
  (it appears under two entities: `…el25kobq…` and `…fhoa3zff…`, each Δ−26013,
  i.e. ours far *smaller* than ref).

### The 2 structural bundles — root cause

```
objalign  ours objects: 2   ref objects: 3
  TextAsset    80 = 80
  Texture2D     0 vs 87580   REF!   ← present in ref, ABSENT in ours
  AssetBundle 336 vs 348     SIZE   (container shrinks because the texture row is gone)
raw CAB: ours 3008   ref 92588
```

Ours emits **no Texture2D at all**. The source content
(`…/5d85/bafybeigqu3…z5ay`, 372033 B) is an **Adobe Photoshop `.psd` file**
(magic `38 42 50 53` = `8BPS`, 256×256 RGBA), not PNG/JPEG. Unity's importer
decodes PSD natively; abgen-rs's decoder does not.

`src/builder.rs::source_extension` (lines 82–90) recognizes only the PNG and
JPEG magic bytes and falls through to `".png"` for everything else; the PNG
decoder (`src/png.rs`) then fails on the PSD payload and the texture is dropped.
(Note `standalone_key_extension` at line 99 already *lists* `.psd` as a valid
key extension — so the metadata key path half-anticipates PSD, but the actual
pixel decoder has no PSD path.)

This is a **decoder-coverage gap of exactly 1 unique source asset** in val300,
unrelated to the mid-size cluster's compression-noise root cause. It is
mechanically recoverable but low-value (1 asset; corpus-wide PSD frequency
should be measured before investing). Proposal below, not applied.

## Fix proposal (structural outlier only; NOT applied)

Add PSD decoding to the standalone-texture pixel path:

- `src/builder.rs::source_extension`: detect `8BPS` (`raw[0..4] == b"8BPS"`)
  and return `".psd"`.
- Decode path (whatever `src/png.rs` / the standalone builder calls to turn
  `raw` → `RgbaImage`): branch on PSD and decode it. A pure-Rust PSD reader
  (e.g. the `psd` crate, flatten the composite/merged-image section to RGBA) is
  clean-room safe. Many `.psd` exports store a precomposited 8-bit RGBA preview
  that maps directly to the importer's expected pixels.

Acceptance: rebuild the two entities above; `objalign` should show the
Texture2D row present at 87580 with matching mip chain. Whether it reaches
byte-exact then reduces to the same BC7-texel wall as every other texture.

**Do not apply blind**: first run a corpus-wide PSD count
(`grep`/magic-scan source contents) to confirm the reward. In val300 it is a
single asset; the build cost of a PSD decoder may not be justified by 2
bundles.

## Representative numbers

- mid cluster: 1628 bundles; 1626 LZ4 noise (981 larger / 645 smaller,
  Σ|Δ| 10.06 MB, median 4.0 KB) + 2 structural (PSD, Δ−26013 each).
- noise raw-size census: 1626 byte-length-identical raw payloads, 0 missing.
- region census: 40/40 sampled noise bundles diff only in the BC7 image-data
  region; 0 header/typetree diffs.

## Artifacts (read-only probes, no production-src change)

- `/tmp/decomp_census.py` — raw vs on-disk size split over all 1628.
- `/tmp/region_census.py` — header-region vs image-region localization (n=40).
- Prebuilt tools used: `examples/dump_decomp`, `examples/objalign`.

## Relation to prior docs

Confirms and extends `standalone_texture_size_session.md` (which found 0
structural-field mismatches on a *reference-derived* rebuild) on the actual
**production output** `/tmp/abgen-val300-integrated`: structural size is right
for 1626/1628; the residual is BC7→LZ4 noise (the
`landed/texture2d_residual_v3.md` "What's left #1" wall). The one new finding
this session adds is the **PSD decoder gap** (2 bundles / 1 source asset),
which the earlier reference-rebuild census could not surface because it never
exercised the failing decode path the same way.
