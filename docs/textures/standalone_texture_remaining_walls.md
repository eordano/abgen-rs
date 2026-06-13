# Standalone-texture parity after the jump-flood bleed: the three remaining walls

**Why it matters:** With the alpha-bleed pass corrected (see
`docs/textures/alpha_bleed_jump_flood.md`), power-of-two PNG textures with alpha now
reproduce every mip-0 BC7 block of the reference byte-for-byte, but most still
fail the whole-bundle byte comparison on a handful of deep-mip blocks. The
diff mass that used to be blamed on "BC7 within-mode float residual" actually
decomposes into three independent, *upstream* problems plus a small true
encoder tail. Knowing which is which prevents wasted encoder work.

**The decomposition** (block-level diffing of our payload against the
reference, per texture sub-population):

1. **NPOT sources (resize filter) — CLOSED.** Non-power-of-two images (and
   oversized ones) are resized before import. Unity's filter is a separable
   cubic in the raw byte domain: Mitchell-Netravali widened by the ratio on a
   downscale axis, cubic B-spline at unit width on an upscale axis,
   pixel-center phase, edge clamp. Derived byte-exact from uncompressed-RGBA32
   probes; see `docs/textures/texture_resize_filter.md`. This was the largest remaining
   population. (Our earlier box/bilinear cascade entered the wrong pixels into
   the bleed; with the correct resize the resized pixels match and the rest of
   the bundle follows.)
2. **JPEG sources (decoder) — IDENTIFIED, mostly closed.** Unity decodes
   JPEG with the **islow IDCT + box (non-fancy) chroma upsampling + JFIF
   matrix** on the standalone-texture path (glb-embedded images keep glTFast's
   fancy upsampling). Derived byte-exact from analytic single-AC and 4:2:0
   chroma step/ramp probes read out of the real converter via an
   uncompressed-RGBA32 import; see `docs/textures/texture_jpeg_decoder.md`. abgen now
   routes standalone JPEGs through libjpeg-turbo with that filter (was the
   `image` crate). A small ±1-LSB tail remains on high-frequency JPEGs —
   Unity's FreeImage ships a specific libjpeg-turbo build whose islow SIMD
   rounding differs by a hair from the system one abgen links; that is the
   only piece left, and it is a build/version detail, not a filter-family
   question.
3. **The deep-mip tail (CLOSED).** This was never a mip-chain divergence:
   the 2x2/1x2/2x4 mips are smaller than one compressed block, and the block
   is filled by *tiling* the mip image, not by replicating its edge — see
   `docs/textures/bc7_subblock_padding.md`. The "alpha-bearing only" observation was
   sample bias. What remains in this family: occasional single blocks in
   mips 1-2 that decode to *identical pixels* on both sides — true encoder
   tiebreak residuals, the only genuinely encoder-shaped diffs left in this
   kind.

## Resize-filter audit (post-landing): the filter is byte-exact

After the resize filter landed, a corpus-wide audit confirmed it reproduces
Unity's resized pixels exactly, and attributed the rest of the
resize-affected residual to causes that are *not* resize:

- **Uncompressed oracle (ground truth).** Every standalone/in-glb texture
  that imports as an uncompressed format (RGB24 / RGBA32 / ARGB32 — fmt 3/4/5,
  including the sub-4 power-of-two fallback whose resized pixels land in the
  bundle verbatim) was compared pixel-for-pixel against the reference across
  the whole val300 corpus (1826 bundles) and the shibu world corpus. **Every
  one is byte-identical.** This includes a real upscale (816→1024 width with a
  passthrough sub-4 height) and the 256×1 mip-chain cases. The resize filter
  has no edge-case bug: extreme ratios, tiny targets, mixed up/down axes, and
  the npot tie path all reproduce. (`examples/uncomp_digest.rs` is the
  standing oracle — one hash line per uncompressed texture; any mismatch on a
  matching path-id is a resized/decoded-pixel bug.)
- **No cap-path cases in val300.** Among the resized non-identical standalone
  pairs, *zero* sources exceed the 1024 max-texture-size on either axis, so
  the >max cap-resize path is unexercised by the corpus residual (it produces
  byte-identical output where it does fire). `bc7_target_size` computes the
  cap scale from `max(w,h)` then npots each axis; npot ties up (matching
  `Mathf.ClosestPowerOfTwo`).
- **BC7 decode oracle (alpha-weighted).** Decoding mip-0 of every resized BC7
  pair (`examples/decode_dist.rs`, premultiplied/alpha-weighted RGB distance —
  raw RGB under transparent texels is bleed-fill garbage and must be weighted
  out) splits the 1001 resized BC7 textures into **857 pixels-correct
  (encoder-blocked)** and **144 with a structured alpha-weighted distance.**
  None of the 144 is a resize bug: every plain-PNG suspect has a signed-mean
  opaque interior diff in [-0.83, +0.59] (unbiased BC7 encoder + alpha-edge
  noise, *not* the consistent ±N shift a resize-domain error would leave).
  The remaining suspects decompose into the JPEG decoder wall (18) and the
  PNG color-management wall below (the rest). The decode oracle must run
  against `ABGEN_REAL_TEXTURES=1` output — default standalone BC7 is a flat
  mean-color stub, which would otherwise dominate the distance.

## A fourth wall: PNG embedded gamma / ICC color management — CLOSED

Some source PNGs carry a `gAMA`, `iCCP`, `sRGB`, or `cHRM` chunk. The full
per-chunk derivation is now in `docs/textures/png_color_management.md`. Verdict:

- **`gAMA` (non-trivial): applied.** FreeImage decodes against a fixed screen
  gamma of 2.2: `out = round((sample/255) ^ (1/(gamma·2.2)) · 255)`. Landed as a
  256-entry LUT (`apply_png_gamma`, RGB only). On the corpus case
  `gAMA = 0.55531` the BC7-decoded mip-0 distance drops 13.5 → 0.0; the bundle
  stays non-byte-id only on the irreducible BC7 encoder wall.
- **`iCCP`: ignored — including non-sRGB color spaces.** Every iCCP-carrying
  standalone texture (Adobe RGB, Generic RGB, Display-P3, Photoshop, sRGB)
  decodes identically to ignoring the profile (alpha-matched RGB distance median
  0.0). No clean-room ICC transform is needed. (The large raw distances in the
  oracle were all oversized-source stub artifacts, not color transforms.)
- **`sRGB` chunk / `cHRM`: no-ops** (alpha-matched RGB distance median 0.0).

The gate (`png_gamma_to_apply`) is profile-aware: it fires only on a non-trivial
`gAMA` with no `sRGB` override, leaving the identity-gamma textures
untouched. Corpus-wide it fires on a single standalone PNG, with **zero change
and zero regression** in the byte-identical count — the affected bundle was
already BC7-encoder-blocked, so the win is serving fidelity (correct brightness),
not parity. The glb decode path is unaffected: the glb-embedded PNGs that carry
`gAMA` all carry the trivial identity value.

**Instrument for the next session:** the probe-entity technique (substitute a
synthetic image for a real wearable's `image.png`, serve the entity flat over
HTTP, run the reference converter in batchmode) reads Unity's behavior out
directly. For walls 1-3 the highest-leverage probe is an image that imports
as **uncompressed RGBA32** (no BC7 noise): Unity's exact resize, JPEG decode,
and mip pixels become byte-readable. `examples/bc7po2.rs` (BC7PO2_FULL /
BC7PO2_MIPS / BC7PO2_JFA env knobs) and `examples/bc7probe.rs` hold the
scoring and readout harnesses.
