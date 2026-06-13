# glb-scene large outliers — 16-bit PNG downconvert (rounding vs the reference's truncation)

> Drill ran against `/tmp/abgen-val300-integrated` vs
> `ad0564d-val300-windows` reference, report
> `/tmp/abgen-val300-integrated-report.json`. All numbers are bytes-on-disk
> for the LZ4HC-compressed bundle. Fix applied + corpus-verified on a 30-bundle
> sample (25 affected + 5 controls).

## TL;DR — SOURCE

For glb-scene bundles whose source GLB embeds a **16-bit-per-channel PNG**,
`image::load_from_memory(r).to_rgba8()` (in `src/gltf.rs`) downconverts the
16-bit samples to 8 bits with **rounding rescale** — `round(v * 255 / 65535)`,
the `image` crate's default. The converter's texture-import path instead
**truncates the high byte** — `v >> 8`. The two rules disagree by exactly ±1 on ~15 % of
pixels. Those ±1 perturbations sit in the **RGBA32 mip-0** of the streamed
texture, propagate through the generated mip chain, and de-correlate the LZ4HC
stream → a consistent **+~1.46 KB positive** scene-size delta.

This is the dominant **by-count** source of the glb-scene "large outlier"
population (the 1–2 KB tier): **104 of the 158 resolvable large-delta scenes
embed a 16-bit PNG, and every one of those 104 is positive-delta** (0
negative). The fix recovers ~98.5 % of each such bundle's delta.

## Evidence chain

Example bundle (Δ +1480):
`bafkreiel25kobqehrdb…/bafybeic6niy…_windows` (`models/Trash_Group.glb`).

1. **Not structural.** `objalign` → 17 vs 17 objects, every per-object size
   identical (MISMATCH_LINES=0). Same for every large-delta scene sampled
   (the top +534 KB has only a 48-byte AssetBundle preload-list diff).
2. **Uncompressed payload is the same size, different bytes.** `dump_decomp`:
   ours and ref both decompress to CAB 117016 + `.resS` 2534112. The CAB is
   byte-identical; the `.resS` differs in **155 928 bytes**.
3. **The diffs are ±1 RGBA32 pixels.** `cmp -l` of the `.resS`: 155 810 / 155 928
   diffs are exactly ±1, spread across all four channels; data at the diff
   offsets is repeating 4-byte RGBA (e.g. ours `01 22 70 ff` vs ref
   `00 22 70 ff`). The first diff is at `.resS` byte 776 — inside mip-0, not a
   mip-generation artifact.
4. **The texture is a 16-bit PNG.** The GLB embeds 2 images:
   `ColorAtlasEmmisiveBlack` (JPEG) and `ColorAtlas_BaseColor`
   (**PNG, 512×512, bit-depth 16, color-type 6 = RGBA16**). The RGBA32 texture
   in `.resS` (at offset 1, post vertical-flip) is this 16-bit PNG.
5. **`image` crate uses rounding; the converter uses truncation.** Decoding the PNG at
   native depth and applying four candidate 16→8 rules, measured against the
   reference mip-0:

   | rule | mip-0 diffs vs ref (of 1 048 576) |
   |---|---:|
   | `v >> 8` (truncate) | **4 421** |
   | `image` `to_rgba8()` ≡ `round(v*255/65535)` | 158 567 |
   | `round((v+128)>>8)` | 343 071 |
   | `floor(v*255/65535)` | 337 723 |

   `to_rgba8()` matches the `scale255/65535` rule on **100 %** of bytes
   (verified: `to_rgba8 == round(v*255/65535)` for all 1 048 576 samples,
   differs from truncation on 155 110). The reference output matches **truncation**.
   The residual 4 421 after truncation is a separate, much smaller secondary
   cause (sRGB linear round-trip edge cases + the JPEG/BC7 sibling texture) —
   already-known BC7-texel / resS noise, treated as irreducible elsewhere.

## Recoverable? YES — trivial, corpus-verified

The decode site is `src/gltf.rs` (the scene/GLB texture path) at
`image::load_from_memory(r).ok().map(|d| d.to_rgba8())`. Replaced with a
reference-matching helper that truncates 16-bit-per-channel samples:

```rust
fn decode_image_rgba8_unity(bytes: &[u8]) -> Option<image::RgbaImage> {
    let d = image::load_from_memory(bytes).ok()?;
    use image::DynamicImage::*;
    let needs_trunc = matches!(d, ImageLuma16(_) | ImageLumaA16(_) | ImageRgb16(_) | ImageRgba16(_));
    if !needs_trunc { return Some(d.to_rgba8()); }   // 8-bit path unchanged
    let src = d.to_rgba16();
    let (w, h) = (src.width(), src.height());
    let out: Vec<u8> = src.as_raw().iter().map(|&s| (s >> 8) as u8).collect();
    image::RgbaImage::from_raw(w, h, out)
}
```

8-bit images take the identical `to_rgba8()` path — **zero behaviour change**
except for 16-bit sources, so there is no regression surface on the 8-bit
majority.

### Corpus verification (30-bundle sample, windows)

Built with the fix via `abgen-corpus` (manifest mode), diffed vs reference:

- **Example bundle: Δ +1480 → +22** (CAB now byte-identical; `.resS` residual
  155 928 B → 818 B, all ±1/±2).
- **25 affected 16-bit-PNG scenes: each ~+1480 → ~+12…+34**, i.e. ~1 400–1 460 B
  recovered per bundle; **35 835 B recovered** across the sample.
- **5 control scenes** (the +534 KB / +130 KB / +102 KB / +3.8 KB / +2.4 KB
  outliers, no 16-bit PNG — BC7-placeholder & other causes): **unchanged**, as
  expected. No regression.
- `cargo test --release --test parity_bytes`: **passes** (2/2).

## Numbers — which objects / how much

- Per affected bundle the entire delta is in **one Texture2D's RGBA32 `.resS`
  payload**; no object-set or per-object serialized-size change.
- Population (val300 windows, glb-scene, |Δ|>1024): **104 affected** bundles,
  all positive, summing to **181 313 B** of the 954 799 B total positive
  large-scene delta. 103 of the 104 are in the 1–2 KB tier; 1 is >20 KB.
- Expected recovery at ~98.5 % per bundle ≈ **~150 KB** across val300 glb-scene
  (and proportionally more on the full corpus). The remaining ~770 KB of
  positive large-scene delta is the **BC7 mode-5 placeholder** pattern
  (`landed/glb_scene_large_outliers.md`) and unrelated causes — untouched by
  this fix.

## Irreducible residual after the fix

~12–34 B per affected bundle, living entirely in the `.resS` as ±1/±2 noise
from (a) the sRGB→linear→sRGB mip-0 round-trip on the same texture and (b) the
JPEG-sourced BC7 sibling. These are the known BC7-texel / sRGB-tie residuals
classified irreducible (or soft) in `dev/PARITY_STATUS.md`; out of scope here.

## Note on the dead `src/png.rs` decoder

`src/png.rs` ships its own PNG decoder with `scale_to_8(16) = (sample+128)/257`
(another rounded rescale). It is **not wired into any decode path** (`grep
crate::png` → no hits) — purely test/reference code. If it is ever made
load-bearing it must adopt the same `>> 8` truncation, or it will reintroduce
this exact delta.
