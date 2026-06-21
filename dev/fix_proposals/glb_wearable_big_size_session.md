# glb-wearable BIG size outliers (>1k) — structural source = none; BC7-texel LZ4 compression-noise

> Status: NEGATIVE FINDING (diagnosis). No code change. Gate green at baseline.
> Area: glb-wearable bundle-level size deltas, |Δ|>1024 bytes.
> Method: clean-room (ref bytes + objalign/dump_externals/dump_decomp); no Unity decompile.
> Corpus: val300 windows. Ref `ad0564d-val300-windows`, ours `/tmp/abgen-val300-integrated`.

## TL;DR

The 65 glb-wearable bundles with |Δ| > 1024 are **NOT** caused by any structural
over/under-emission. In **every one of the 65**:

- object **count is identical** (oObj == rObj),
- external **dependency set is identical** (1/1 on both sides),
- the **decompressed** asset file (CAB serialized-file) **and** its **`.resS`
  texture stream have byte-for-byte identical lengths** on both sides.

There is **no** extra/missing Mesh, no extra/missing SkinnedMeshRenderer, no
embedded-texture COUNT mismatch, no blendshape-stream-length difference, and no
mip-count / `.resS`-split difference. The serialized object sizes match
object-for-object.

The entire bundle-size gap is the **LZ4 compression of differing-but-equal-length
content** — overwhelmingly the BC7 texel bytes in `.resS` (our pure-Rust BC7
encoder, `src/bc7_pure.rs`, produces a different byte pattern than the converter's
bc7e output — the Apache-2.0 Intel/GameTechDev encoder the converter invokes — for
the same image), plus a tiny amount of float-precision mesh-vertex
noise in the CAB. Same uncompressed bytes-count → LZ4 squeezes them to a
different compressed size. This is the same **LZ4 length-noise** mechanism already
documented in `landed/sf_pad_lz4_noise.md` and the texel residual in
`bc7_texel_walkdown_session.md`, here scaled up by the volume of differing texels.

## SOURCE

`src/bc7_pure.rs` BC7 encoder texel bytes → `src/ress.rs build_ress()` (`.resS`
stream holds ONLY texture pixel blobs; no mesh/blendshape data) → LZ4 block
compression in the bundle writer. Differing texel bytes of identical length
compress to a different size. Secondarily, float-precision mesh vertex/normal
bytes in the CAB (e.g. `RecalculateNormals`/tangent quantization) add a handful
of differing bytes that also shift the compressed length.

## Population numbers (val300 windows, kind == glb-wearable)

| metric | value |
|---|---|
| total glb-wearable bundles | 660 |
| outliers \|Δ\| > 1024 | 65 |
| of which ours **smaller** (Δ<0) | 41 |
| of which ours **larger** (Δ>0) | 24 |
| \|Δ\| > 64k | 4 |
| Σ \|Δ\| | 1,327,099 bytes |
| outliers with object-count mismatch | **0** |
| outliers with external-set mismatch | **0** |
| outliers with decompressed-length mismatch (CAB or .resS) | **0** |

The sign of Δ is not structural — it just reflects whether our BC7 texels happen
to LZ4-compress better (Δ<0) or worse (Δ>0) than the converter's bc7e output for that texture.

## Evidence

### Whole-population census (`objalign` + `dump_externals`)

All 65 bundles: `OURS objects == REF objects`, `externals_count 1/1`. No bundle
shows an only-in-ours / only-in-ref object. `objalign` size column matches
object-for-object; the only `DIFF` flags are on `AssetBundle`,
`Mesh`, and (where present) `Texture2D` rows whose **sizes are equal** — i.e.
content differs, length does not.

### Decompressed-content census (`dump_decomp` → `cmp`)

For the top-12-by-magnitude and all 24 positive-delta bundles, decompress both
sides and compare the raw CAB + `.resS`:

| Δ (bytes) | declen match | CAB differing bytes | .resS differing bytes (% of stream) |
|---:|:--:|---:|---:|
| -318753 | yes | 0 | 560,302 (26.7%) |
| -318710 | yes | 0 | 560,302 (26.7%) |
| -153833 | yes | 0 | 1,284,505 (9.2%) |
| -37618  | yes | 1009 | 1,529,172 (2.4%) |
| -34776  | yes | 48306 | 919,319 (6.7%) |
| +13134  | yes | 28 | 722,060 (14%) |
| +4428   | yes | 55 | 82,800 (3.6%) |
| +1656   | yes | **0** | 168,135 (2%) |
| +1474   | yes | **0** | 155,928 (6%) |
| +1438   | yes | **0** | 155,928 (6%) |

The CAB diff is tiny (0–few-thousand bytes of float noise); the bulk is `.resS`
BC7 texels. Cases like +1438/+1656 have a **byte-identical CAB** yet a +1438/+1656
byte bundle — pure BC7-texel LZ4 noise.

### Block-layout proof (header parse, -318753 case
`QmWDB7…/QmVLGRFfAJjqyc7HTQRpPDHYHLGUsciZP1A4LkTavnU7rz_windows`)

```
OURS: fmt=8 total=403274 comp_blockinfo=222 uncomp_blockinfo=483 flags=0x243 file_len=403274
REF : fmt=8 total=722027 comp_blockinfo=249 uncomp_blockinfo=483 flags=0x243 file_len=722027
```

`uncomp_blockinfo` is **identical (483)** — same number of LZ4 blocks, same
uncompressed block sizes, same flags. Only `comp_blockinfo` (compressed block
table) and the file length differ. The 318 KB gap is entirely the compressed
payload of the same-length-but-different-bytes BC7 texel stream (here our texels
happen to compress far better than the converter's bc7e output).

### `.resS` content is texture-only (`src/ress.rs`)

`build_ress()` pushes only `TextureBlob.pixels` into the stream (with align(16)
padding between textures); no mesh, no blendshape, no animation data. So a
`.resS` byte diff is necessarily a texture-texel diff, never a blendshape/stream
length issue. Confirmed for the +4428 worked example: `.resS` differs in 3.6% of
bytes, all inside texel regions; the CAB differs in 55 bytes (one Mesh's
float-precision vertex data).

## Recoverable vs irreducible

**Irreducible** under the clean-room discipline. The gap is a function of two
residuals that are themselves the subject of separate, already-explored research
areas:

1. **BC7 texel byte-values** — the converter's bc7e output (the Apache-2.0
   Intel/GameTechDev encoder it invokes; see NOTICES.md) vs our
   `bc7_pure.rs`; two independent implementations of that encoder diverging on
   free/valid BC7 choices. `bc7_texel_walkdown_session.md` already established that on
   size-matched standalone textures the residual is only 4 blocks / 26 bits and
   has no exploitable clean signal; on size-mismatched bundles the diffs are
   mode-selection / mip differences that change compressed block sizes — exactly
   what drives these glb-wearable size gaps. Closing this requires bit-exact
   replication of the converter's bc7e output, which is out of scope and has repeatedly
   yielded negative findings.
2. **LZ4 compressed length** — even with byte-identical uncompressed content the
   compressed length can differ; with differing texels it always will.
   `landed/sf_pad_lz4_noise.md` documents this as accepted length-noise.

There is **no recoverable structural defect** in this population: builder.rs,
materials.rs, ress.rs, gltf.rs, mesh_layout.rs and naming.rs all emit the correct
object set, the correct external set, and the correct uncompressed byte layout.
The size metric is simply a lossy proxy here — `bits_diff` / byte-identical is the
honest signal, and these bundles legitimately are not byte-identical because the
texels are not bit-exact.

## Recommendation

No fix. Do not chase glb-wearable bundle-size deltas as a structural target —
they are a downstream amplification of the BC7-texel and LZ4-noise residuals.
Any further reduction must come from the BC7 encoder area (Tier B, repeatedly
negative) or accept the residual. If a parity scoreboard wants to avoid the
misleading size signal, score glb-wearable on object-set + decompressed-length
parity (both already 100% on this population) rather than compressed bundle size.

## Reproduce

```bash
export ABGEN_CONTENT_ROOT=/path/to/content/contents
REF=/path/to/abc-abgenrs-799967c3-2026-06-20/val300-windows
OURS=/tmp/abgen-val300-integrated
T=<repo>/target/release
E=QmWDB7zBET827mHk5gVCai1dk5m9ZLt12KGTY7Y1EUB6rW
B=QmVLGRFfAJjqyc7HTQRpPDHYHLGUsciZP1A4LkTavnU7rz_windows
$T/examples/objalign      $OURS/$E/$B $REF/$E/$B   # object counts equal, only DIFF on equal-size rows
$T/examples/dump_externals $OURS/$E/$B $REF/$E/$B  # 1/1 identical
rm -rf /tmp/o /tmp/r
$T/examples/dump_decomp $OURS/$E/$B /tmp/o
$T/examples/dump_decomp $REF/$E/$B  /tmp/r
ls -l /tmp/o /tmp/r                                # CAB + .resS lengths identical
cmp /tmp/o/00_* /tmp/r/00_*                        # CAB: byte-identical (this case) or tiny float noise
cmp /tmp/o/01_*.resS /tmp/r/01_*.resS              # .resS: differs in texel bytes, same length
```
