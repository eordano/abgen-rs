# glb-scene size — 16-bit PNG downconvert: truncate, don't round

**Why it matters:** Scene GLB bundles whose source embeds a 16-bit-per-channel PNG were consistently coming out larger than the reference. The decode path used the `image` crate's `to_rgba8()`, which downconverts 16-bit samples to 8 bits by rounding-rescale (`round(v * 255 / 65535)`). Unity's texture importer instead truncates the high byte (`v >> 8`). The two rules disagree by one step on a meaningful fraction of pixels. Those off-by-one values land in the RGBA32 mip-0 of the streamed texture, propagate through the generated mip chain, and de-correlate the LZ4HC stream, producing a consistent positive size delta. This was the dominant by-count cause of the glb-scene large-outlier population, and every affected bundle skewed in the same positive direction.

**How it works:** The fix replaces the decode site with a helper that detects 16-bit-per-channel source images and downconverts each sample by truncation (`s >> 8`) instead of rounding. Eight-bit sources take the unchanged `to_rgba8()` path, so there is no behaviour change and no regression surface on the 8-bit majority. After the fix the texture's `.resS` payload becomes byte-identical to the reference except for a tiny residual.

That residual is irreducible: a few off-by-one bytes per bundle from the sRGB-to-linear-to-sRGB round-trip during mip generation on the same texture, plus the BC7-encoded sibling texture — the known BC7-texel and sRGB-tie noise classified irreducible elsewhere.

Note: a separate, unused in-tree PNG decoder ships its own 16-to-8 rounded rescale. It is not wired into any decode path, but if it is ever made load-bearing it must adopt the same truncation or it will reintroduce this exact delta.
