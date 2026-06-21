# Texture ±1–4 byte on-disk deltas + the LZ4HC compression-size question

**Status:** DIAGNOSIS — no production change. The tiny on-disk texture deltas
are **LZ4HC compressed-size noise downstream of BC7 texel-byte differences**
(irreducible without closing the BC7 encoder gap). They are **not** a
header/field-length difference and **not** an LZ4HC compressor divergence.
Quantified over the full val300 windows texture corpus (6,258 bundles).

## Question

For standalone textures (~144 modern + the tiny legacy ones) that differ by
only ±1–4 bytes on disk: are these deltas

- **(A)** LZ4HC compressed-size noise downstream of different BC7 texel bytes
  — i.e. our BC7 encoder produces slightly different blocks, the raw
  uncompressed sizes match exactly, and LZ4HC of slightly-different input
  yields a slightly-different output length — **irreducible** here, or
- **(B)** a real header/field length difference (structural) — **recoverable**?

And: does our `src/lz4.rs` LZ4HC actually match liblz4 byte-for-byte, or is
the on-disk delta a compressor divergence?

## Method

`dump_decomp` extracts each bundle's raw decompressed dir-node bytes (CAB +
any `.resS`). For both example pairs and then for **every** texture bundle in
val300 I compared:

1. raw decompressed **total size** (ours vs ref), and
2. raw decompressed **byte content** (XOR/diff count).

If raw size matches but content differs → the on-disk Δ is pure LZ4 noise over
different input bytes (case A). If raw size differs → structural (case B).

Tools: prebuilt `target/release/examples/{dump_decomp,objalign}` + a custom
`examples/tex_probe.rs` (corpus-wide decompress-and-compare; also a per-mip
breakdown of where the texel diffs land in the BC7 image data).

## The two example pairs — drilled to the byte

| pair | on-disk Δ | raw size ours/ref | raw bytes differ | header (172 B) eq |
|------|----------:|-------------------|-----------------:|:-----------------:|
| `bafkreie2e6lwz…` (Δ+1) | +1 | 354736 / 354736 | 28 | **yes** |
| `bafkreiai3lwjh…` (Δ-2) | -2 | 354732 / 354732 | 31 | **yes** |

Both are 512×512, `m_TextureFormat=25` (BC7), 10 mips,
`m_CompleteImageSize=349552`, `m_IsReadable=1`, inline (`m_StreamData.path=""`).
The AssetBundle container, PreloadTable and the entire **172-byte Texture2D
header are byte-identical**. Every differing byte is inside the 349,552-byte
inline BC7 image data:

```
pair1 (Δ+1): mip0 (512²) 15 diff bytes, mip7 (4×4 block) 13 diff bytes
pair2 (Δ-2): mip0 (512²) 15 diff bytes, mip7 (4×4 block) 16 diff bytes
```

i.e. a couple of BC7 4×4 blocks in mip0 plus the tiny 4×4 mip7 block encode to
different bytes. Raw object size is identical (349724 / 349724); only the texel
*content* diverges by ~28–31 bytes out of 349,552. LZ4HC then re-compresses
that slightly-different input to a length that differs by ±1–2 bytes. **Case A,
unambiguously.**

## Corpus-wide result (val300 windows, both texture kinds)

| kind | pairs | raw-size-EQ & content-EQ | raw-size-EQ & content-NEQ (LZ4 noise) | raw-size-NEQ (structural) |
|------|------:|-------------------------:|--------------------------------------:|--------------------------:|
| standalone-texture        | 3,139 | 321 | 2,816 | 2 |
| standalone-texture-legacy | 3,119 | 333 | 2,782 | 4 |
| **total**                 | **6,258** | **654** | **5,598** | **6** |

Of the **5,549 on-disk-differing texture pairs** (delta≠0):
**5,598 (≈100%) are pure LZ4 noise over different BC7 texel bytes; 6 (0.11%)
are structural** (different raw length).

### The small-delta class specifically (the prompt's target)

|d|≤4 on-disk delta, delta≠0: **196 texture bundles** (144 standalone +
52 legacy — matches the prompt's "~144 + legacy tiny ones").

```
small_total = 196
raw-size EQ (=> pure LZ4 noise)      = 196   (100%)
raw-size NEQ (=> structural)         = 0
```

**Every single one of the ±1–4 byte texture deltas has byte-identical raw
decompressed size. Zero are structural. Case A is proven for the entire class.**

## The LZ4HC byte-exactness check

The report's `byte_identical=true` count for textures is **654**, which equals
**exactly** the probe's `raw-size-EQ & content-EQ` count (654). In other words:
**every time the raw decompressed bytes are identical, the on-disk bytes are
identical too** — there is no pair where identical input produced different
compressed output. This independently re-confirms the
`size_delta_v2.md` finding (186/186 prod blocks recompressed byte-equal):
**`src/lz4.rs::compress_hc` matches liblz4 byte-for-byte.** The on-disk delta
is never the compressor.

## The 6 structural outliers — already-tracked, NOT a texture-envelope bug

The 6 raw-size-NEQ cases are all large textures where the raw lengths differ by
~30×–900× (e.g. ours CAB 3,008 B / 2,504 B vs ref 92,588 B / up to 2,800,896 B):

```
bafkreiel25…/bafybeigqu3…  ours_raw 3008    ref_raw 92588
QmV8E9…/Qmemt6…            ours_raw 2504    ref_raw 2800884
QmV8E9…/QmQu5t…            ours_raw 2504    ref_raw 1402772
QmV8E9…/QmZwfg…            ours_raw 2504    ref_raw 2800896
```

Decomposing the first: ref CAB = 92,588 B (pixels inline), ours CAB = 3,008 B
(metadata only — pixels missing / streamed to a `.resS` we did not emit). This
is the **streaming-gate / missing-content** class already documented in
`landed/size_delta_v2.md §6.1` and `landed/textures_streaming.md`
(the `model_referenced && fmt==25 && 512×512` gate is too narrow for larger /
mipped CIDv1 textures that prod streams). It is a content/streaming issue, not
a header field-length bug, and is out of scope for this texture-delta question.

## Conclusion

1. **SOURCE.** The ±1–4 byte standalone-texture on-disk deltas are LZ4HC
   compressed-length noise produced by feeding LZ4HC slightly-different BC7
   texel bytes. The texture envelope (172-byte Texture2D header, AssetBundle,
   PreloadTable, mip layout, `m_CompleteImageSize`, stream flags) is
   byte-identical; only a handful of BC7 4×4 blocks (mostly in mip0 and the
   tiny terminal mips) encode differently.

2. **Recoverable vs irreducible.** **Irreducible at the envelope/compressor
   level.** Evidence: (a) all 196 |d|≤4 deltas have byte-identical raw
   decompressed size; (b) the 172-byte header is byte-identical in the drilled
   pairs; (c) the LZ4HC compressor is byte-exact vs liblz4 (654/654
   identical-input ⇒ identical-output). The only way to shrink these deltas is
   to make the **BC7 encoder** produce the same block bytes the reference does —
   tracked under `landed/bc7_tiebreak_v2.md`,
   `bc7_texel_walkdown_session.md`, and the BC7 mode-rule proposals.

3. **Fix proposal.** None for this topic — there is no header/compressor fix to
   make. The residual is BC7 encoder drift (separate workstream). The 6
   structural outliers belong to the streaming-gate workstream
   (`landed/textures_streaming.md`), also separate.

4. **Numbers.** val300 windows, 6,258 texture pairs: 654 byte-identical,
   5,598 (≈100% of differing) pure LZ4-noise-over-texel-diff, 6 (0.11%)
   structural. The targeted small-delta class: 196 bundles, 196/196 pure LZ4
   noise, 0 structural.

## Repro

```bash
# corpus-wide decompress-and-compare (the table above)
cargo build --release --example tex_probe   # examples/tex_probe.rs
target/release/examples/tex_probe \
  /tmp/abgen-val300-integrated \
  /path/to/abc-abgenrs-799967c3-2026-06-20/val300-windows \
  /tmp/abgen-val300-integrated-report.json \
  standalone-texture            # or standalone-texture-legacy
```
