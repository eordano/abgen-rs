# Standalone-texture mid-size deltas — BC7-to-LZ4 noise, plus a PSD gap

**Why it matters:** The modern standalone-texture path has a large mid-size
on-disk delta cluster. Knowing whether these are structural (a wrong-length
field, a mis-set split threshold) or downstream encoder noise decides whether
they are worth chasing for byte-identical output.

**How it works:** This is a negative finding. Decompressing each pair and
comparing the raw uncompressed payload lengths shows they are byte-length
identical on both sides; the deltas split both larger and smaller, which alone
rules out any single-directional structural bug. Every object length matches —
including the Texture2D — the serialized-file header and typetree regions are
byte-identical, and the byte differences sit entirely in the back-half BC7
image-data block. So `m_Width`/`m_Height`, mip count, texture format,
`m_CompleteImageSize`, and the inline-vs-`.resS` split are all already correct.
The divergent component is the BC7 mode/partition/endpoint/selector choice per
4x4 block: our pure-Rust encoder produces different texel bytes than the converter's
bc7e output for the same pixels, same dimensions, same mip chain, same total length —
and LZ4HC then compresses those differing texels to a different on-disk length.
That is the entire delta, and it is the same encoder-parity wall owned by the
BC7 workstream, not a size problem fixable here.

A small number of bundles are structural and recoverable, all from one source
asset: an Adobe Photoshop PSD (`8BPS` magic). The source-extension detector
recognizes only PNG and JPEG and falls through to PNG, so the decoder fails and
no Texture2D is emitted while the converter's importer decodes the PSD natively. The fix is to
detect the PSD magic and decode its flattened composite to RGBA via a clean-room
PSD reader. It is low-value at this corpus size; measure corpus-wide PSD
frequency and verify against the source before investing.
