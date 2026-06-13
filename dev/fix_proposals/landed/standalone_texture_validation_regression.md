# Standalone-texture validation regression — e4fd1f2 mean-color over-fires on `LoadImage`-failing sources

**Status**: drilled, root cause identified, patch scoped.
**Corpus**: validation_windows (2339 bundles, 208 standalone-texture).
**Headline**: top-3 outliers = 85.4M bits = **52.6 %** of all standalone-texture diff bits; closing them shaves **~332 K ppm** off the standalone-texture class and **~6,073 ppm** off the whole corpus.

---

## 1. Per-bucket distribution

208 standalone-texture bundles, bucketed by source-image max axis:

| max(w, h) | n | mean-color fired (ours) | bits_diff | ours<ref | ours==ref | ours>ref |
|---:|---:|---:|---:|---:|---:|---:|
| ≤ 512        | 137 | 0  |  31.3 M | 78 | 18 | 41 |
| 513-1024     |  37 | 0  |  45.8 M | 16 |  0 | 21 |
| 1025-2048    |  25 | 25 |   7.3 K | 0  | 22 |  3 |
| 2049-4096    |   5 | 5  |  31.7 M | 1  |  4 |  0 |
| > 4096       |   4 | 4  |  53.7 M | 2  |  1 |  1 |

Source format split: **193 PNG · 11 JPEG · 4 WebP** (extension lies; sniffed by magic bytes).

Size ratio (ours / ref):
- 145 within ±10 %
- 45 byte-equal
- 15 between 10× and 0.1×
- **3 worse than 100× or better than 0.01×** ← the regression

### Mean-color gate audit (the e4fd1f2 fix)

34 bundles have source dim > 1024 → our `mean_color_image` stub fires for all 34. Of those 34:
- **31** ref-side bundles are ~5-8 KB ← **stub correct** (ref also collapsed).
- **3** ref-side bundles are 3.3-3.96 MB ← **stub wrong** (ref kept full 2048² content).

The 3 mis-fires account for **85,397,818 bits_diff** (52.6 % of all standalone-texture diff).

---

## 2. Top-10 outlier inspection

Source decoded via magic-byte sniff; reference Texture2D inspected with `examples/dump_tex.rs`.

| # | bits_diff | bundle src CID | fmt | src dim | src bytes | ours bytes | ref bytes | ref tex | ours tex |
|---:|---:|---|---|---|---:|---:|---:|---|---|
| 1 | 31,661,159 | bafkreifvxc…vsem | webp | 2800×2800 |   200 KB |     7,855 | 3,961,511 | **2048², 5.59 MB BC7, real content** | 1024², 1.4 MB BC7, mean-color |
| 2 | 27,096,800 | bafybeihleq…xa74 | jpeg | 8256×6192 |  17.8 MB |     7,943 | 3,390,515 | **2048², 5.59 MB BC7, real content** | 1024², 1.4 MB BC7, mean-color |
| 3 | 26,639,859 | bafybeih7cpf…6amu | jpeg | 6192×8256 |  13.5 MB |     7,936 | 3,333,663 | **2048², 5.59 MB BC7, real content** | 1024², 1.4 MB BC7, mean-color |
| 4 |  5,203,588 | bafybeifevb…m4au | png  | 1024×1024 |   1.4 MB | 1,345,018 | 1,345,042 | 1024², 1.4 MB BC7 | 1024², 1.4 MB BC7 — **BC7 long-tail noise** |
| 5 |  3,080,505 | bafybeicg7y…tw2a | png  | 1024×1024 |   1.2 MB |   809,848 |   810,091 | 1024², BC7 | 1024², BC7 — **BC7 long-tail noise** |
| 6 |  2,867,306 | bafkreietnwv…ibnu | png  |  768×768  |   227 KB |   723,934 |   719,987 | 768², BC7 | 768², BC7 — **BC7 long-tail noise** |
| 7 |  2,089,503 | bafybeihauwh…x3ga | png  |  894×553  |   905 KB |   528,295 |   527,971 | 1024×512, BC7 | 1024×512, BC7 — **BC7 noise** |
| 8 |  1,994,127 | bafkreicxihb…orhu | png  | 1024×1024 |   179 KB |   503,314 |   496,968 | 1024², BC7 | 1024², BC7 — **BC7 noise** |
| 9 |  1,912,322 | bafybeiahpz…26iy | png  | 1024×561  |   787 KB |   491,858 |   488,189 | 1024×512, BC7 | 1024×512, BC7 — **BC7 noise** |
|10 |  1,727,076 | bafybeiasf3n…zq6e | png  |  903×466  |   397 KB |   436,902 |   430,473 | 1024×512, BC7 | 1024×512, BC7 — **BC7 noise** |

**Top 3 are the regression. Entries 4-10 are BC7 long-tail (handled by separate proposals).**

---

## 3. Root cause

**The e4fd1f2 mean-color stub fires unconditionally when `max(w, h) > max_texture_size_for(target)`. But the prod-side equivalent only fires when `Texture2D.LoadImage(image)` succeeds.**

Walk through the prod path
([`AssetBundleConverter.cs:1585-1616`](https://github.com/decentraland/asset-bundle-converter/blob/main/asset-bundle-converter/Assets/AssetBundleConverter/AssetBundleConverter.cs#L1585-L1616)):

```csharp
private void ReduceTextureSizeIfNeeded(string texturePath, float maxSize) {
    byte[] image = env.file.ReadAllBytes(texturePath);
    var tmpTex = new Texture2D(1, 1);
    if (!tmpTex.LoadImage(image)) {          // <— gate
        Object.DestroyImmediate(tmpTex);
        return;                              // file left untouched
    }
    /* … resize via Graphics.Blit → Texture2D.ReadPixels → EncodeToPNG → overwrite on disk … */
}
```

`Texture2D.LoadImage` returns false for:

1. **Non-PNG / non-JPEG sources** — `LoadImage` only accepts PNG (`89 50 4E 47`) and JPEG (`FF D8 FF`). WebP / GIF / BMP / KTX / DDS all return false → file stays on disk in original format → Unity's TextureImporter then imports the **original** (which it can or can't handle on its own); its **own** `maxTextureSize` default = **2048**.
2. **Decoded buffer exceeds Unity's hard limit** — empirical: an 8256×6192 JPEG (200 MP, 200 MB allocated buffer) does not survive the LoadImage path under `-batchmode -nographics` (probably a Mono out-of-memory or a `Texture2D` allocation cap). The 13.5 MB / 17.8 MB JPEG sources land here; both end up as 2048² in ref.

In both branches the source file on disk remains **untouched** by `ReduceTextureSizeIfNeeded`. Unity's TextureImporter subsequently imports the original file with its own `maxTextureSize = 2048` cap, producing a 2048² Texture2D with real content (not collapsed).

Our `mean_color_image` stub doesn't model this gate — it fires on every source whose dimensions exceed 1024 (windows) / 1024 (mac) / 512 (linux/webgl). The 3 mis-fires:

| src CID                       | fmt  | dim         | src bytes | why `LoadImage` returns false |
|-------------------------------|------|-------------|-----------|--------------------------------|
| `bafkreifvxc…vsem` (`rd.png`) | webp | 2800×2800   |  200 KB   | WebP magic, not PNG/JPEG |
| `bafybeihleq…xa74` (`dajoy1.jpeg`)   | jpeg | 8256×6192   | 17.8 MB | 200 MP > Unity LoadImage cap |
| `bafybeih7cpf…6amu` (`dastyle.jpeg`) | jpeg | 6192×8256   | 13.5 MB | same |

(The 17.8 MB file is a Pentax 645Z raw-export JPEG that hashes by CID but lies about extension; the 200 KB WebP is `images/rd.png` that also lies about extension.)

The five oversize JPEGs that ARE correctly collapsed (3584×5376, 2497×3329, 1920×1080×2, 1366×2048 — all ≤ 1.6 MB src) confirm the gate fires by **decoded-bitmap memory**, not source-bytes — but a conservative source-bytes / max-axis proxy is sufficient.

---

## 4. Concrete patch proposal

**Files**: `src/builder.rs` (~10 LOC) + `src/texprofile.rs` (~5 LOC) — no other touch.

### 4.1 Predicate

In `src/texprofile.rs`, add a single function that mirrors Unity's `Texture2D.LoadImage` outcome:

```rust
// Returns true iff Unity's `Texture2D.LoadImage(bytes)` would have decoded
// the source on the prod converter — i.e. ResizeTexture+EncodeToPNG ran,
// producing a 1024²-clamped PNG that subsequently BC7-compressed to a
// mean-color stub under `-batchmode -nographics`.
//
// False cases (3 observed in validation_windows, all sharing entity
// `bafkreig6t2…mf6i`):
// • container ∉ {png, jpeg} (e.g. WebP, GIF, BMP, KTX, DDS)
// • decoded pixel count > 32 M (≈ 128 MB ARGB32) — Unity batchmode
// LoadImage cap; safe lower bound is 32 MP (3 misfires are 53/51/8 MP,
// all collapsed cases are ≤ 19 MP).
pub fn unity_load_image_would_succeed(src: &SourceImage) -> bool {
    let container_ok = matches!(src.container.as_str(), "png" | "jpeg");
    let pixels = (src.width as u64) * (src.height as u64);
    container_ok && pixels <= 32 * 1024 * 1024
}
```

(`SourceImage::container` already carries `png`/`jpeg`/`webp`/etc — see `detect_container` callers; the WebP source in test #1 is correctly classified.)

### 4.2 Gate the mean-color stub

In `src/builder.rs:2435-2442` (StandaloneTextureBuilder) and `src/builder.rs:918-924` (in-glb path), broaden the existing oversize check with the new predicate:

```rust
// before
let img: &RgbaImage = if w > max_size || h > max_size {
    stubbed_buf = mean_color_image(img);
    &stubbed_buf
} else {
    img
};

// after
let img: &RgbaImage = if (w > max_size || h > max_size)
    && texprofile::unity_load_image_would_succeed(&src)
{
    stubbed_buf = mean_color_image(img);
    &stubbed_buf
} else {
    img
};
```

### 4.3 Raise the cap for LoadImage-failed sources

Still in the same path, when the stub does **not** fire but the source is still oversize (the 3 cases), produce a 2048² resized texture instead of a 1024² one — because Unity's downstream `TextureImporter.maxTextureSize` default is **2048**. The cleanest place to do this is `standalone_texture_profile` (`src/texprofile.rs:210`): accept an extra `effective_max: u32` argument and have the caller pass `2048` when `unity_load_image_would_succeed` is false. The current `npot_ti` rounding stays correct (2800 → 2048 by midpoint-up rule, 8256 → 2048 by cap, both confirmed against the dumped ref Texture2D).

Caller (`src/builder.rs:2431`):
```rust
let cap = if texprofile::unity_load_image_would_succeed(&src) {
    texprofile::max_texture_size_for(self.target)  // 1024
} else {
    2048
};
let prof = texprofile::standalone_texture_profile(&src, cap);
```

### 4.4 Expected impact

- **standalone-texture**: 632,445 ppm → ~300,090 ppm (**−332 K ppm**).
- **whole validation_windows corpus**: 610,693 ppm → ~604,620 ppm (**−6,073 ppm**).
- Risk: zero on the 31 currently-collapsing oversize PNGs (predicate evaluates true for them, behaviour unchanged).
- New BC7 long-tail exposure on the 3 fixed cases: ours produces a 2048² BC7 that won't be byte-exact with ref (BC7 encoder noise) — but the residual will be ~5 % of the current ~85 M-bit gap, not ~100 %.

### 4.5 Validation pinset

Add to `tests/fixtures/standalone_texture/`:

```
bafkreig6t2vwzhg6nbht5znoxmy2biyiei6pgi5ib2igwvwkg2y6nwmf6i/bafkreifvxc453wmsotcjlenre3adcbgedt5ebxpdrdrkxp55ogd2azvsem_windows # webp gate
bafkreig6t2vwzhg6nbht5znoxmy2biyiei6pgi5ib2igwvwkg2y6nwmf6i/bafybeihleq44565wwfrk4ywccallc2ywnuxskx6m3qsx22toxgi6qkxa74_windows # huge-jpeg gate
bafkreig6t2vwzhg6nbht5znoxmy2biyiei6pgi5ib2igwvwkg2y6nwmf6i/bafybeic6lfqrgrt4kz23ntqq5n747or4naxsad4agbsvevqwu3cvgbu56i_windows # 1366×2048 jpeg ≤ 32 MP — must stay collapsed (regression guard)
```

The third pin guards against widening the predicate (e.g. dropping the jpeg-pixel-count limit and letting 4-MP jpegs leak through to 2048² ref output).
