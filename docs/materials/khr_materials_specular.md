# KHR_materials_specular textures and the DXT1 encoder

**Why it matters:** A glTF material can route a texture through the
`KHR_materials_specular.specularColorTexture` extension into Unity's
`_SpecColorMap` slot. abgen-rs only understood the standard PBR slots plus
`KHR_materials_pbrSpecularGlossiness` and `KHR_texture_transform`, so an image
reachable *only* through this extension was never assigned a slot and no
Texture2D pair was emitted for it. The bundle came out short one texture pair
versus the reference, which cascades into every downstream pointer and breaks
byte-identical output.

**How it works:** Each material slot must be wired end to end — the glTF parser
records the extension's texture reference on the material, the slot table gains a
`_SpecColorMap` entry (linear color space), and the texture pipeline emits a
Texture2D pair for it like any other slot. The non-obvious part is the
*streamed* variant of this slot: the reference compresses the specular-color
texture as DXT1 (BC1), not the BC7 used for every other slot, and tags that DXT1
variant as sRGB even though the uncompressed in-glb copy is linear. The slot is
the discriminant: the same image fed through any other slot would be BC7. abgen-rs
classifies an image as DXT1 only when it is referenced exclusively via
`_SpecColorMap`, then runs a pure-Rust DXT1 encoder (PCA endpoint selection,
pinned to opaque 4-color mode) to build the mip chain.

The uncompressed half of the pair is reproduced bit-for-bit. The DXT1 payload is
structurally identical to the reference's (block count, mip count, format and
color-space tags all match) but not bit-identical, because the converter's BC1
compressor is a rate-distortion-optimizing encoder whose exact heuristics are not
reproduced. Matching the structure is the load-bearing requirement — a wrong
format byte or mip count would crash Unity's GPU upload — so the residual is
limited to the compressed payload of this one slot.
