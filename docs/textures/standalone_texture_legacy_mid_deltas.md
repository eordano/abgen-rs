# Legacy standalone-texture population — same walls as the rest, no distinct rule

**Why it matters:** The corpus splits single-texture bundles into two census
shapes. The "legacy" shape carries only a Texture2D and an AssetBundle object;
the modern shape adds a metadata.json TextAsset. The legacy shape reaches
byte-identical output less often than the modern one, which invites the
suspicion that it is an older bundle generation with a different texture
encoder, different import settings, or different mip rules. It is not. Treating
it as a first-class population and attributing every non-identical pair shows
the gap is the same two texture walls that bound the whole corpus, sampled at a
slightly different ratio.

## The two shapes are the same entity with one object removed

The only thing that separates the two populations is the metadata.json
TextAsset. abgen suppresses that object for content-identifier-v0 (`Qm…`) root
entities to match the reference corpus, and emits it for v1 (`baf…`) roots. The
split is total: every legacy-shape bundle is a `Qm` entity and every modern one
is a `baf` entity. There is no second texture code path — both shapes run the
identical profile selection, resize, alpha dilation, wrap-padded mip chain, and
BC7 encoder. The legacy bundle is the modern bundle minus one TextAsset and its
one preload-table entry.

## Population census

Every legacy reference texture is BC7 (the same compressed format the modern
shape uses), save a handful of sub-block stubs stored uncompressed. Dimensions,
mip counts, the inline-vs-streamed split, and every scalar Texture2D header
field match the reference on every pair — a full scalar-field diff across all
non-identical pairs finds zero differing fields. So the object shape is already
correct everywhere; the entire residual lives in the compressed texel bytes.

The sources are overwhelmingly PNG, with a small JPEG minority and a few Adobe
Photoshop PSD files. The PSD decoder gap noted in the previous version of this
page is closed: PSD-sourced bundles now emit a real texture, not an empty one.

## Attribution of the non-identical pairs

Aligning each pair object-by-object, the only nonzero divergence axis is the
texel payload — never an identifier, float-field, structural, or preload-order
axis. So the legacy population contributes nothing to the structural walls
(preload ordering, sub-asset relabeling, scene-object identifiers) that bound
the glb populations. It is a pure texture problem, and it divides cleanly:

- **Resize wall.** Where the source image's dimensions differ from the bundle's
  power-of-two dimensions, Unity downscaled the image with a filter abgen does
  not yet reproduce bit-for-bit, so the pixels genuinely differ. This is the
  largest single contributor and is the corpus-wide NPOT resize wall, not
  specific to legacy.

- **BC7 encoder float-order wall.** Where the source already matches the bundle
  dimensions, the decoded pixels are identical or near-identical, yet the
  compressed bytes differ. Block-level diffing shows these are dominated by
  endpoint differences (mostly one-unit-in-the-last-place) and equal-quality
  mode choices, with no structural divergence — the documented within-mode
  float-order residual between the pure-Rust encoder and the reference encoder.
  This holds even for fully opaque textures whose per-pixel error sits above the
  alpha-weighted distance threshold: the threshold counts visible-pixel error,
  so an opaque image with uniform low encoder noise reads as "pixels differ"
  while still being the same float-order wall.

A small JPEG minority adds the JPEG-decoder-identity wall (the reference decoder
yields slightly different pixels than ours), again a corpus-wide wall.

## What this rules out

The hypotheses that motivated treating legacy as its own population are all
falsified by the census: no DXT1/DXT5 encoder (legacy is BC7 like everything
else), no different import-settings era (header fields match exactly), no
distinct mip rule (the wrap-padded sub-block tail-mip rule already applies and
the diffs cluster in the top mips, not the tail), and no distinct alpha
handling. The lower byte-identical ratio is explained entirely by legacy
sampling the resize wall and the same-dimension encoder wall at a somewhat
higher rate than the modern shape — not by any rule that abgen is missing for
legacy bundles. There is nothing legacy-specific left to fix here; progress on
this population is gated on the two general texture walls.
