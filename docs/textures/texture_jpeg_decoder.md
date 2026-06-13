# Unity's JPEG decoder

**Why it matters:** Opaque JPEG-sourced textures diverged from the reference on
nearly every compressed block, because Unity's *decoded pixels* differ from
what a default JPEG decoder produces — before any resize or compression. This
was one of the last standalone-texture walls (see
`docs/textures/standalone_texture_remaining_walls.md`). The divergence is in the decode,
so it had to be read out of the reference directly.

## How it was read

The same probe technique that cracked the resize filter applies here, but with
a cleaner readout: the converter's texture importer was temporarily forced to
**Uncompressed** for a probe run, so a JPEG imports as raw `RGB24`/`RGBA32` —
Unity's exact decoded pixels land in the bundle verbatim, with no BC7 encoder
and no resize in the way (the sources are already power-of-two). Substituting a
probe JPEG into a real entity, serving it flat over HTTP, and running the actual
converter in batchmode yields those pixels as plain bytes.

Each decode stage is separable, so analytic probes read one stage at a time:

- **Single-coefficient grayscale JPEGs** (4:4:4, quant table all ones so the
  coefficients pass through unquantized) read the **inverse DCT** one basis at a
  time. A DC-only block reads the DC scaling and level shift; a block with a
  single AC coefficient reads exactly one IDCT basis function.
- **4:2:0 chroma step and ramp JPEGs** (constant luma, the chroma planes
  carrying only a per-block DC) read the **chroma upsampling filter and phase**:
  the only variation in the output comes from how the half-resolution chroma is
  stretched to full resolution.
- **Saturated color patches** read the **YCbCr→RGB matrix and rounding**.

## The decoder

Unity uses **two different JPEG decoders depending on where the image comes
from**, and they differ only in chroma upsampling:

| Source | Importer | IDCT | Chroma upsampling | Matrix |
|---|---|---|---|---|
| Standalone texture (entity content image) | editor `AssetImporter` (FreeImage 3.18.0's bundled **IJG libjpeg 9c**) | islow | **box** (nearest) | JFIF |
| glb-embedded / glb-referenced image | glTFast (system libjpeg-turbo path) | islow | **fancy** (triangle) | JFIF |

- **IDCT = islow** (the accurate integer inverse DCT). Every single-AC probe —
  DC, low-frequency AC, and the highest-frequency AC (zig-zag index 63) —
  reproduces libjpeg's `JDCT_ISLOW` output byte-for-byte. The fast integer IDCT
  (`ifast`) is falsified everywhere: it diverges by up to 96 on the index-63
  basis.
- **Chroma upsampling = box for standalone, fancy for glb.** The 4:2:0 step
  probe reads a hard box step at every chroma-block boundary on the standalone
  path — the output jumps directly from one chroma block's value to the next,
  with no interpolation. The ramp probe confirms it: four chroma blocks at
  `70,110,150,190` upsample to flat runs `70…70,110…110,150…150,190…190`, never
  the interpolated `70,80,100,110,…` that libjpeg's fancy upsampling produces.
  The glb path, by contrast, keeps glTFast's fancy upsampling.
- **YCbCr→RGB = the standard JFIF / BT.601 full-range matrix**, with the usual
  rounding. Saturated-chroma probes match it exactly; BT.709 and studio-range
  variants are falsified.

## The evidence

- DC=100 grayscale → flat `141` (`100/8 + 128`, level-shifted), exact on islow.
- A single AC at index 1 (value 80) → the horizontal cosine
  `145,143,138,131,125,118,113,111`, exact on islow; `ifast` reads
  `…130…` where islow reads `131`.
- A single AC at index 63 (value 400) → `132,117,144,109,147,112,139,124`,
  exact on islow; `ifast` is off by up to 96.
- A 4:2:0 chroma step (`Cb 128→188`, `Cr 128→78`) decodes on the standalone
  path to a hard transition `128…128 | 234…234` at the block boundary — box.
  libjpeg's fancy would ramp `128→143→173→188` across the seam.

## The exact codec — IJG libjpeg 9c, not libjpeg-turbo

The ±1-LSB tail on busy JPEGs was *not* an islow rounding difference, and the
standalone codec is *not* libjpeg-turbo. FreeImage 3.18.0 (the editor's image
loader) bundles its **own** JPEG codec, and its `Source/LibJPEG/jversion.h`
identifies it precisely:

```
#define JVERSION "9c  14-Jan-2018"
```

That is the classic **Independent JPEG Group libjpeg, version 9c** — a pure-C
reference codec under the permissive IJG license, not turbo. Reproducing it:

- **The islow IDCT is byte-identical across every libjpeg lineage.** A
  grayscale JPEG (IDCT-only, no chroma path) decodes byte-for-byte the same
  under IJG 9a / 9c / 9e / 8d *and* libjpeg-turbo 3.1.4. So the divergence was
  never in the inverse DCT.
- **The chroma path is where the lineages split.** On busy *color* JPEGs the
  IJG 9-series (9a/9c/9e) all agree with each other but disagree with both 8d
  *and* turbo, in a handful of chroma-edge pixels (max |Δ| ≈ 23). turbo tracks
  the older 8/6b box-upsample + YCbCr→RGB merge rounding, not the 9-series.
  Those pixels were exactly abgen's residual standalone-JPEG tail.

abgen now vendors IJG 9c's decode-side sources (`third_party/libjpeg9c`,
permissive IJG license) and routes the **standalone** box path through it
(`src/ffi.rs` `decode_jpeg_rgba_box`). The vendored decoder is byte-faithful to
the upstream IJG 9c reference build. `ABGEN_JPEG_TURBO_BOX=1` restores the old
libjpeg-turbo box path for comparison.

This landed **+4 byte-identical bundles on val300 windows, zero regressions** —
only 4 bundles were diverging *solely* on the decode; the rest of the 94 JPEG bundles whose
9c-vs-turbo decode differs also carry the irreducible BC7 AVX2-vs-ISPC encoder
noise, so they still differ after the fix (the codec removes the decode
component, the encoder wall remains).

**The glb path stays on libjpeg-turbo.** glTFast's editor glb import does *not*
use FreeImage: routing glb-embedded JPEGs through IJG 9c fancy upsampling
(`ABGEN_JPEG_GLB_9C=1`) regresses val300 5139 → 4860 (gains 0, loses 279). The
standalone-vs-glb decoder split is confirmed: FreeImage 9c box for standalone,
libjpeg-turbo fancy for glb.
