# The converter's texture resize filter

**Why it matters:** When a source image is non-power-of-two, or exceeds the
platform's maximum texture size, the `TextureImporter` resizes it before
compressing. Our earlier resize (a box/bilinear cascade) disagreed with
the reference filter, so every compressed block downstream diverged — this was the
single largest remaining standalone-texture wall, affecting the majority of
non-power-of-two textures across both the modern and legacy standalone
populations. With the correct filter, those textures reproduce the resized
pixels byte-for-byte and the rest of the bundle follows.

## How it was read

The filter was derived black-box, by making the reference's resized pixels
directly readable. The converter normally compresses standalone textures to BC7,
which buries the pixels under the encoder. The probe sidesteps that: a tall, two-pixel-wide
image whose power-of-two target has a dimension below four imports as
**uncompressed RGBA32**, so the resized values land in the bundle verbatim.
Substituting such an image into a real entity, serving it flat over HTTP, and
running the actual converter in batchmode yields the resized column as
plain bytes.

Three families of synthetic content read three independent properties:

- **Impulse** (a single bright row on black) reads the kernel taps and phase
  directly: only one source sample is non-zero, so each destination value *is*
  the normalized kernel weight at that offset.
- **Step edge** (black top half, white bottom half) reads the working domain:
  a cubic kernel ringing across a sharp edge gives different transition values
  depending on whether the convolution runs on raw bytes or on linearized
  light.
- **Ramp** reads the rounding at the end of the pipeline.

## The filter

The reference resize is a **separable cubic convolution**, run independently on each
axis, computed in the **raw byte (sRGB-encoded) domain** — values are *not*
linearized before resizing. The phase is pixel-center, and edges clamp to the
nearest valid sample. The cubic family depends on the direction of each axis:

- **Downscale axis** (target smaller than source): the Mitchell-Netravali
  cubic (the balanced cubic with both shape parameters set to one third), with
  the kernel widened by the scale ratio so it behaves as a proper area
  resample.
- **Upscale axis** (target larger than source): the cubic B-spline (the
  smooth, all-positive cubic), at unit width.

A pass-through axis (target equal to source) copies its samples unchanged.

Because the filter is separable, a tall narrow image with a horizontal
dimension that does not change exercises only the vertical kernel — which is
why the two-pixel-wide probes isolate one axis cleanly.

## The evidence

Resizing a 300-tall column to 256 (a downscale):

- An impulse at source row 150 lands as 22, 192, 5 on destination rows 127,
  128, 129. The widened Mitchell kernel predicts 21.8, 192.1, 5.0 — exact to
  the byte.
- The asymmetric edge cases confirm the phase and the edge clamp: an impulse
  at row 0 lands 211, 5 (rows 0, 1) and one at row 299 lands 5, 211 (rows 254,
  255), both matching the model.
- A black-to-white step edge transitions through 19, 236 on rows 127, 128.
  The byte-domain Mitchell prediction is exactly 19, 236; the linear-light
  prediction is 78, 246 — wrong. This proves the convolution runs on raw
  bytes, with no sRGB linearization.

Resizing a 200-tall column to 256 (an upscale):

- An impulse at source row 100 lands 2, 58, 167, 94, 7 across rows 126–130.
  The unit-width B-spline predicts those five values exactly; the Mitchell
  kernel would introduce negative side-lobes the data does not show.

Every digit above came out of an actual converter-built bundle, so the filter is a
direct observation of the reference, not a fit.

## What it does not cover

This closes the resize wall but not the two other standalone-texture walls
(see `docs/textures/standalone_texture_remaining_walls.md`): the converter's JPEG
decode-path identity (opaque JPEG sources still diverge in their *decoded* pixels, before
any resize), and the small encoder-tiebreak tail on textures that already
match at the pixel level. Power-of-two PNG sources never hit the resize path
and were already correct.
