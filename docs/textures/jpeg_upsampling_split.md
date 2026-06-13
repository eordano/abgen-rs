# Standalone JPEG chroma upsampling: fancy when Unity imports the file, box when the converter pre-resizes it

**Why it matters:** A chroma-subsampled JPEG (4:2:0 or 4:2:2) stores color at a lower resolution than brightness, and the decoder has to upsample it. libjpeg offers two upsamplers: a box filter (each chroma sample replicated) and the default "fancy" triangular filter (chroma interpolated between samples). The two produce slightly different RGB values for almost every pixel of a subsampled image, so picking the wrong one diverges the BC7 encode across most of the texture — small per-pixel error, huge byte-level divergence.

**How it works:** There are two distinct decode points for a standalone JPEG texture, and they use different upsamplers:

- **The converter's pre-resize pass.** When a texture exceeds the platform's maximum size, the converter decodes, downscales, and rewrites the file before Unity sees it. That pass decodes with **box** upsampling, and the rewritten image is what Unity imports. abgen models this as box decode plus the derived resize filter.
- **Unity's own importer.** When the file fits and Unity imports the original JPEG directly, the import decodes with **fancy** upsampling — libjpeg's default, which the earlier box-only model missed.

So the rule keys on whether a resize happens: resized standalone JPEGs keep the box decode, non-resized ones re-decode with fancy upsampling before encoding. For 4:4:4 JPEGs the two upsamplers are identical and the rule is a no-op.

**How it was found and verified:** The residual-tail audit after the quality-knob fix still showed one texture with whole-image low-amplitude divergence — diffs spread smoothly across 55% of pixels with no 4×4 or 8×8 grid structure, which indicts the decoded input pixels rather than the encoder. An input sweep (`examples/jpeg_input_sweep`) over four decoder variants showed fancy upsampling at 96% block identity where the shipped box decode managed 32%. Sweeping every subsampled standalone JPEG in the validation corpus: fancy wins or ties on **all** non-resized files — pure and model-referenced alike — while every byte-identical subsampled JPEG under the box model turned out to be a resize case. Landing the split gained 22 byte-identical bundles with zero regressions, and the worst texture's pixel error dropped from a maximum of 132 levels to 8.

**Tooling caveat from this investigation:** `examples/bc7diff` undercounted differing blocks on this texture family (its per-pid payload walk disagreed with a direct byte comparison of the extracted payloads); use `examples/tex_payload` plus a direct compare when exact differing-block counts matter.
