# Spec-color-only textures: default-import DXT5 stub + linear in-glb copy

**Why it matters:** A glTF image referenced exclusively through
`KHR_materials_specular.specularColorTexture` still gets a Texture2D pair in
the bundle (glTFast loads it), but it is bound to no URP shader TexEnv — the
converter's `FixTextureReferences` pass iterates shader TexEnv properties, so
it never visits this texture and never upgrades it to the CompressedHQ/BC7
import every bound texture gets. The extracted compressed copy keeps the
DEFAULT TextureImporter settings: plain DXT5 (format 12), sRGB. The glTFast
uncompressed in-glb copy is created linear (`m_ColorSpace = 0`). abgen was
emitting BC7 + sRGB for both copies.

**The stub:** in the headless reference, the extraction path
(Blit/ReadPixels without a GPU) yields the 0xCD-filled buffer, so the DXT5
payload is one constant block repeated — the same canonical block on every
spec-color texture in the val300 reference, reproduced byte-exactly by
`encode_inglb_dxt5_stub`. Under `--real-textures` abgen instead encodes the
real pixels with a BC3 mip chain (texpresso) so served bundles render the
actual artwork.

**Classification:** `materials::classify_spec_color_only_images` — the image
is in some material's `specular_color_image` and in no other slot. An image
that is also bound to a real TexEnv slot keeps the BC7 path.

This supersedes the DXT1 description in `khr_materials_specular.md`: the
default importer encodes the extracted RGBA PNG as DXT5, and the payload in
headless reference output is the constant stub, which IS bit-reproducible.
