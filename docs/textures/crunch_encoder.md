# Crunch (CRN) encoder for BC5 normal-map textures

**Why it matters:** Prod ships BC5 normal-map textures (Unity's DXT5Crunched format) not as raw block data but Crunch-compressed, identifiable by a magic prefix in the streamed payload. Emitting raw BC5 instead inflates the streamed bytes and, because the raw block layout compresses differently under the bundle's LZ4 pass, actually makes the bundles diverge further from prod — so a pure-Rust BC5 encoder alone is a regression, not a fix. Matching prod's texture size requires producing the Crunch container.

**How it works:** The fix vendors the BinomialLLC crunch encoder and exposes it through a thin C-ABI wrapper and a Rust shim. BC5-classified images build their RGBA mip chain (after the normal-map pack and repack to X-in-R, Y-in-G) and hand it to the encoder using crunch's native DXN/XY format, which targets BC5. The resulting CRN stream lands at the streamed-data offset and carries the expected magic prefix.

The format policy is corpus-derived: only DXT5Crunched normal maps are Crunch-wrapped (triggered when a normal texture is also used as a base-color or emissive map); DXT1 specular-color maps and the default BC7 textures stay raw, because that is what prod emits for them. The byte-parity fixtures all stay within their per-bundle caps.
