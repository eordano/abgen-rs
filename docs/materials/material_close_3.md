# Closing the last Material residuals: KHR_texture_transform and cross-bundle pointers

**Why it matters:** A handful of Material objects still diverged from the reference after
earlier per-texture and material-indexing work. Two distinct mechanisms were
responsible, and until both were handled the Material object class could not reach
zero residuals against the corpus.

**How it works:**

*UV transform.* When a glTF texture carries `KHR_texture_transform`, the converter stores
the scale and offset on the corresponding `m_TexEnvs` entry and adds
`_TEXTURE_TRANSFORM` to `m_InvalidKeywords`. abgen-rs never read this extension.
The subtlety is the coordinate conversion: because UV is flipped (`v -> 1 - v`) at
parse time, the AssetBundle-space offset must be derived as `1 - gltf_offset.y -
gltf_scale.y`, computed in f32 to match the reference's exact bytes. abgen-rs now parses
the extension per slot, applies this conversion, skips identity transforms (so
they don't spuriously emit the keyword), and appends `_TEXTURE_TRANSFORM` to the
sorted keyword list when any slot is transformed. Rotation and per-textureInfo
texCoord overrides are deliberately not handled; no corpus material exercises
them, and they would surface as a fresh residual if one ever did.

*Cross-bundle pointers.* A glTF image whose URI resolves to a sibling content
entity (a standalone texture) should serialize as a pointer into an externals
slot, not a local pointer. This path already exists and is exercised when
`ab-build-local` is given a content map; the residuals only appeared because the
no-resolver measurement does not supply one. With the resolver wired in, these
cases match.

The lasting operational note: the measurement script must use the same binary the
caller built. An earlier version hardcoded the binary path, so with concurrent
worktrees rebuilding into a shared target directory it could compare a stale
binary and report phantom residuals. Pinning the binary to an explicit override
makes the metric deterministic. With the resolver-aware metric and these fixes,
no known Material residual remains.
