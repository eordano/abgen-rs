# Full-corpus decode audit against the production CDN

**Why it matters:** Byte parity is only measurable against the validation reference corpus, which samples a few hundred entities. To verify texture correctness across *everything*, the only corpus-wide ground truth is the production asset-bundle CDN (a local mirror of the deployed bundles, built by several generations of the official converter). Decoding our textures and production's to pixels and comparing both against the original source image catches whole classes of systematic defect that the validation set never samples.

**How it works:** Build the complete active set plus worlds, pair every bundle with its production counterpart by entity and content hash (deduplicated by hash), decode every texture's top mip on both sides, and compare. Pairs whose dimensions differ (older production generations used smaller size caps) are bucketed separately. For every pair in the worst tail, a referee step decodes the *source* content file and scores each side's distance to it — whichever side is far from the source is the wrong one, regardless of who diverges from whom.

**Findings of the first full audit (~135,600 unique-hash pairs, ~67,300 same-dimension comparisons):**

- Roughly half of all compared textures decode bit-identically to production, and two thirds are within a couple of 8-bit levels — despite production being a different converter generation.
- A few hundred pairs scored **production wrong**: production's pixels are far from the source while ours match it. The biggest family is palette PNGs that an older production converter mangled.
- The pairs that scored **ours wrong** all reduced to behaviors where abgen is *faithful to the current converter fork* and production's older generation differed:
  - **Dual-bound textures** (one image used as both a color map and a normal map in the same glTF): the fork's import pipeline types the file as a normal map — Unity's importer type is sticky and never downgrades — so the standalone bundle ships normal-map-swizzled. Older production shipped it as a regular color texture. The validation corpus contains exactly one such texture, and its reference bundle confirms the fork swizzles it; abgen reproduces that. An attempted "fix" to veto normal typing on any color use regressed that reference bundle and was reverted.
  - **EXIF-oriented JPEGs** (a handful in the whole corpus): Unity ignores EXIF orientation, so the fork — and abgen — ship them unrotated; older production rotated them.
  - **Pure-normal-bound standalone textures**: the fork types them as normal maps (swizzled, linear); older production did not.

  All three are candidates for upstream fork changes if matching the deployed product's appearance matters more than current-fork behavior; none is an abgen defect.

**Method caveats:** the source-referee comparison resizes the source bilinearly to the bundle's dimensions, which puts a floor of a few RMSE units under each score — use it to classify sides as right/wrong, not to measure encoder quality. Orientation-sensitive bugs require scoring each side against both the raw and the EXIF-transposed source separately; taking the per-side minimum hides rotation mismatches.
