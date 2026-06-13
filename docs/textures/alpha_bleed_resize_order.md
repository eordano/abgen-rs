# Alpha-bleed runs before the resize, not after

**Why it matters:** Non-power-of-two textures that carry a real alpha channel
were the largest remaining systematic standalone-texture family — roughly four
hundred of them on val300-windows. Their fully-opaque interior already matched
Unity byte-for-byte, but their transparent and partial-alpha (edge) texels
diverged wildly: decoded transparent RGB sat ~200 levels away from the
reference, and the anti-aliased silhouette edge ~30–100 levels away. Because a
BC7 block spans both opaque and transparent texels along a silhouette, this
poisoned a large fraction of the blocks even though the encoder and the resize
filter were each correct in isolation.

**The cause was ordering, not the filter.** Unity's converter sets
`alphaIsTransparency = true` on the texture importer
(`AssetBundleConverter.cs`), then calls `ImportAsset`. Inside the import, Unity
runs its `alphaIsTransparency` edge-bleed on the **source-resolution** pixels
and only afterward resizes the image to power-of-two and builds the mip chain.
abgen-rs did the opposite: it resized the raw (un-bled) source first and bled
the result. For a power-of-two source the two stages never overlap, so the
order does not matter and those textures were already correct. For a
non-power-of-two source, resizing first convolves the cubic kernel across the
silhouette while the transparent side still holds its *source* RGB — almost
always solid white in sprite/decal art — so the edge picks up white smear and
the deep-transparent fill never gets the bled edge colour. Bleeding first puts
the correct edge colour into the transparent neighbourhood *before* the kernel
sees it, and the resize then carries a coherent colour field across the edge.

**The fix:** in the standalone-texture path of `src/builder.rs`, the alpha
bleed (`alpha_bleed::alpha_bleed_inplace`) now runs on the source-resolution
buffer first, and the resize consumes the bled buffer. Nothing else changed —
same jump-flood bleed (`docs/textures/alpha_bleed_jump_flood.md`), same Unity
cubic resize (`docs/textures/texture_resize_filter.md`). The bleed runs exactly
once; there is no post-resize re-bleed (per-mip wrong-rate stays flat, as the
earlier bleed investigation established).

**Evidence (val300-windows, decoded mip-0, masked by reference alpha):**

| region | resize→bleed (old) | bleed→resize (new) |
|---|---|---|
| fully opaque (a==255) | ~0–1.5 RMSE | ~0–1.5 RMSE (unchanged) |
| partial alpha (0<a<255) | 18–98 RMSE | 0.5–3.7 RMSE |
| transparent (a==0) | 70–230 RMSE | 0.85–11 RMSE |

Byte-identical bundles on the val300-windows gate rose from 4775 to 4952
(+177; standalone-texture +90, standalone-texture-legacy +87) with **zero
regressions**. The residual after the fix is the same BC7 encoder float-order
wall that bounds the opaque population — confirmed because the remaining
transparent/edge disagreement collapses to the same single-level magnitude as
the opaque interior.

**Scope:** only the standalone / model-referenced texture path enables the
bleed. The in-glb (scene/wearable embedded) texture path stays unbled —
`CustomGltfImporter` does not set `alphaIsTransparency` — and is untouched by
this change.
