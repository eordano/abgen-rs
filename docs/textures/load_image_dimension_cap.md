# Unity's LoadImage gate is a max-dimension cap (8192), not a megapixel cap

**Why it matters:** `unity_load_image_would_succeed` decides whether a source
image loads through Unity's `Texture2D.LoadImage` path. When it returns false on
a GLB-embedded image, the texture is dropped entirely (no Texture2D emitted);
when it returns false on a standalone image, the mean-color collapse and the
importer-cap fallback change. The predicate used to be "PNG/JPEG and decoded
pixels <= 32 * 1024 * 1024" (32 megapixels). That threshold was wrong: it
rejected large-but-loadable images and produced a different object set than the
reference.

**The evidence (val600 scenes, fork reference):** for GLB-embedded images whose
decoded pixel count exceeds 32 Mpx, the reference still emits the full dual
Texture2D set — a native-resolution uncompressed copy plus a downscaled BC7
streamed copy — as long as the largest dimension is at most 8192:

| source dims | megapixels | reference behavior |
|---|---|---|
| 5856 x 5784 | 33.9 Mpx | loads (full RGB24 + 1024 BC7) |
| 6015 x 6124 | 36.8 Mpx | loads |
| 6600 x 6600 | 43.6 Mpx | loads |
| 7828 x 5221 | 40.9 Mpx | loads |
| 8000 x 6620 | 52.9 Mpx | loads |
| 8192 x 8192 | 67.1 Mpx | loads |
| 12000 x 3744 | 44.9 Mpx | LoadImage fails, collapses to an 8x8 stub |

So 67.1 Mpx (8192 square) loads while 44.9 Mpx (12000 wide) fails: the limit is
not pixel count, it is the maximum dimension. 8192 is Unity's default desktop
`SystemInfo.maxTextureSize`; `LoadImage` creates a texture sized to the source
image and fails when a dimension exceeds that, so the gate is
`max(width, height) <= 8192`.

**The fix:** `unity_load_image_would_succeed` now tests `width <= 8192 &&
height <= 8192` (constant `LOAD_IMAGE_MAX_DIMENSION`) instead of the 32 Mpx
pixel count. PNG/JPEG container check is unchanged.

**Scope / no-regression:** the change only affects images that decode to more
than 32 Mpx but whose largest side is at most 8192 — a rare population. Across a
45-entity no-regression set (30 val300 + 15 val600 scenes), all 1751 bundles are
byte-identical before and after. The over-8192 case (12000 wide) stays dropped,
matching no worse than before; reproducing its 8x8 collapse is a separate,
harder problem.

**How it was found:** the val600 scene
`bafkreicxbegozlgh67pjqk2uxjcvndxhhkcgb64sfnilpumhyn2obwy7t4` (the
`PortionBuilding_3.glb` bundle) showed `extra(ours/ref)=0/2` with a ~31 MB size
deficit. `objalign`/`dump_census` traced the two missing objects to a single
glb image (`concrete_monterrey_bump`, a 5856x5784 JPEG, 33.9 Mpx) that the gate
rejected at the 32 Mpx threshold; the reference emitted both its copies.
