# Mean-color collapse must mirror Unity's LoadImage gate, not just dimensions

**Why it matters:** For oversize source images, Unity collapses the texture toward a mean color under headless batchmode, and abgen-rs reproduces this with a mean-color stub. But the stub was firing on any source whose dimensions exceed the platform's max texture size, while prod only collapses when its texture pre-resize step actually ran. A small number of sources triggered the stub when prod kept full content, and those mis-fires dominated the standalone-texture parity gap — a few bundles accounting for the majority of the diff bits in the class.

**How it works:** The prod converter's resize step only runs when `Texture2D.LoadImage` succeeds on the source bytes. `LoadImage` accepts only PNG and JPEG (recognized by magic bytes, not file extension), and it fails when the decoded bitmap exceeds Unity's batchmode memory limit. When `LoadImage` fails, the source file is left untouched on disk and Unity's TextureImporter imports the original under its own default max-texture-size cap — producing a full-content texture at that cap, not a collapsed one.

**The durable rule:** gate the mean-color stub on whether Unity's `LoadImage` would have succeeded — the container must be PNG or JPEG, and the source must be within Unity's LoadImage size limit. Containers like WebP, and images that exceed that limit, fall through this gate: prod keeps their content at the TextureImporter cap, so abgen-rs must produce a full-content texture at that larger cap rather than collapsing. Sources that are PNG/JPEG and within the limit still collapse exactly as before. The container is sniffed from magic bytes because extensions lie.

**Correction (the size limit is dimensional, not a megapixel count):** the limit
was originally modeled as "decoded pixels under 32 Mpx", but that rejected
large-but-loadable images (an 8192x8192 = 67 Mpx source loads fine, while a
12000-wide one fails). The real rule is `max(width, height) <= 8192`, Unity's
default desktop `SystemInfo.maxTextureSize`. See
[`load_image_dimension_cap.md`](load_image_dimension_cap.md).
