# standalone-texture-LEGACY mid-size deltas (1k–64k) — diagnosis

> Status: NEGATIVE FINDING (1991/1992 irreducible) + one separable structural
> bug (1/1992: PSD source not decoded). No production src change proposed for
> the size-class itself; one targeted feature-gap fix proposed separately.
> Area: standalone-texture-legacy path (PNG/JPEG/PSD-sourced single-texture
> bundles). Method: clean-room — decompress pairs, compare RAW block sizes &
> object PathID alignment, read reference bytes + open-source converter only.

## TL;DR

The legacy-texture mid-size cluster (`kind == standalone-texture-legacy`,
`1024 < |Δ on-disk| <= 65536`, **1992 bundles**) is the single biggest
size-diff cluster, but it is **almost entirely LZ4HC compression noise**:

- **1991 / 1992** bundles have **byte-identical decompressed block sizes** on
  both sides (serialized object file *and* `.resS` raw lengths equal). The
  on-disk delta is purely LZ4HC re-compressing **differing-but-equal-length
  BC7 texel bytes**. This is the same irreducible BC7-encoder texel wall already
  documented in the `bc7_*` landed proposals — **not recoverable** without a
  bit-exact BC7 encoder match.
- **1 / 1992** is genuinely structural and **separable**: the source asset is a
  **Photoshop PSD** (`8BPS` magic), which the `image` crate cannot decode, so
  abgen emits an **empty texture bundle** (1130 B, AssetBundle wrapper only)
  while Unity decodes the PSD and ships an 87568-B inline Texture2D. This is a
  **missing-decoder feature gap**, not a size-class problem.

## What sets the legacy bundle byte-size

Path: `StandaloneTextureBuilder::build` (`src/builder.rs:2281`).

1. `image::load_from_memory(raw)` decodes the source (PNG/JPEG via the `image`
   crate). **PSD is not handled** → `decoded == None` → no Texture2D emitted.
2. `texprofile::standalone_texture_profile_named` (`src/texprofile.rs:199`)
   picks the format: for any target ≥ 4×4 it is **always `TF_BC7` (25)**;
   < 4×4 falls back to `TF_RGBA32_UNITY`. Mip count = `default_mip_count`,
   target size = `standalone_target_size` (cap = platform max).
3. Encoding: `encode_texture_bc7(pil, mips, srgb, Bc7Profile::Basic)` on
   windows (`builder.rs:2345-2352`) via `bc7_pure`. The `.resS`/inline split is
   driven by `do_stream = model_referenced && format==BC7` (`builder.rs:2367`);
   for these standalone bundles it is inline unless model-referenced.

So format / dimensions / mip count / streaming-split are all **deterministic
functions of the source dimensions** — and they **match the reference** (see
below). The only divergent component is the **BC7-encoded texel payload**.

## Evidence — RAW block sizes (structural vs noise)

Decompressed every one of the 1992 pairs (`examples/dump_decomp`), compared the
sorted tuple of raw block sizes:

```
NOISE  (equal raw block sizes on both sides): 1991
STRUCTURAL (differing raw block sizes):           1   (the PSD case)
```

Object-level PathID alignment (`examples/objalign` across all 1992):

```
object-count mismatches: 1   (QmRHyMyjF.../QmUdhYmM..._windows: ours 1 obj, ref 2)
```

`examples/class_diff_census` over all 1992:

```
class  name        objs_diff  objs_samesize_diff  bytes_diff   bundles
28     Texture2D       1815             1815       107865107      1815
142    AssetBundle        1                0             156          1
STRUCTURAL (PathID one side only): only_in_ours=0 only_in_ref=1 (Texture2D)
```

Every diffed Texture2D is **same-size** (in-place texel divergence). Zero are
structural in `ours`. The lone `only_in_ref` Texture2D is the PSD case.

### Example decompositions

**Δ−2164** `QmbrqTTbf6.../QmPLdeHkHb..._windows` (ours 45637, ref 47801):
- serialized object file `00`: **byte-identical** (4744 B both).
- `.resS`: **349552 B both** (= BC7 512×512 full mip chain, `512²·4/3 ≈ 349525`).
- `.resS` texel bytes differ at **55394 / 349552 (15.8%)**, first diff @ byte 705,
  spread across all mips, heaviest in the small mips (region 7 = 44%).
- ⇒ identical structure; −2164 is LZ4HC of different-but-equal-length texels.

**Δ+2413** `QmTVaReJE.../QmVJHnWLh..._windows` (39839 vs 37426):
- raw blocks `4740 + 349552` **identical both sides**. ⇒ noise.

**Δ+8631** `QmNsCYT8be.../QmVTuScAk..._windows` (63808 vs 55177):
- **inline** texture (no `.resS`), serialized file `179444 B both`.
- diffs start @ byte 14080 (past the Texture2D metadata header — name / width /
  height / format / mip fields all identical), 42782 differing texel bytes.
- ⇒ identical structure; +8631 is LZ4HC noise.

### The one structural case (separable bug)

`QmRHyMyjF83HzeagrLWb3fiRQaJK7vx5r4gyqPbtEboKMx / QmUdhYmM..._windows`
(ours 1130 B, ref 27082 B):

- `objalign`: OURS = 1 object (AssetBundle only); REF = 2 (AssetBundle +
  87568-B inline Texture2D, PathID 2551810659333879676 missing in ours).
- Source file (`ABGEN_CONTENT_ROOT/.../QmUdhYmM...`, 372033 B) starts with
  `8BPS` = **Adobe Photoshop PSD**, not PNG/JPEG.
- `image::load_from_memory` returns `None` for PSD → `tex_pid = None` →
  empty bundle. Unity's importer decodes PSD and ships the texture.

## Recoverable vs irreducible

| Slice | Count | Class | Recoverable? |
|---|---:|---|---|
| equal raw blocks (BC7 texel noise) | 1991 | compression noise | **No** — same irreducible BC7-encoder wall as `bc7_*` proposals |
| PSD source not decoded (empty bundle) | 1 | structural / feature gap | **Yes** — add PSD decode |

Supporting signals that the 1991 are genuinely the BC7-encoder wall, not some
recoverable structural error:

- The **333 byte-identical** legacy bundles are the **small** textures
  (`ref_bytes ≈ 1.9k–3.4k`), where BC7 block selection is unambiguous — proving
  the encoder *does* match when there is no tie/mode ambiguity.
- Pearson `r(texel bits_diff, |Δ on-disk|) = 0.34` over the mid cluster: more
  texel divergence ⇒ larger on-disk delta (LZ4HC of more-different bytes),
  exactly the noise signature. Metadata is provably identical (serialized `00`
  byte-identical wherever a `.resS` carries the texels; for inline textures the
  first diff is always well past the metadata header).

This matches the prior `dev/fix_proposals/bc7_texel_walkdown_session.md`
conclusion on the *modern* standalone path: the only clean texel signal lives in
the byte-size-matched slice, and the size-mismatched bundles are LZ4-of-texels
noise that cannot be block-aligned for parity. The legacy path is the same wall
with a much larger size-mismatched population.

## Fix proposal

**For the 1991-bundle size cluster: none.** It is irreducible until the BC7
encoder is bit-exact with Unity's. No production change is warranted for the
size class; chasing the on-disk delta here is chasing LZ4HC noise. The lever is
the existing BC7 texel-encoder work, tracked under the `bc7_*` proposals, and is
out of scope for a "size-class" fix.

**For the 1 PSD case: add a PSD source decoder (separable, low risk).** In
`StandaloneTextureBuilder::build` the source is currently routed only through
`image::load_from_memory`. Detect the `8BPS` magic and decode the merged
composite (PSD stores a flattened RGBA composite image precisely so importers
that don't parse layers can still load it) before falling through to the empty
path. This is a clean-room, format-spec-driven addition (Adobe PSD file-format
spec is public) and is **corpus-scoped to a single bundle in val300**, so it
should be implemented behind a verify-gate run rather than landed blind — the
risk is that Unity's PSD composite (premultiply / color-profile handling) does
not match a naive composite read, which would only move this from "empty
bundle" to "texel-noise bundle" without a byte win. **Recommendation: prototype
+ verify against this one CID before landing; do not touch the size-cluster
path.** This single bundle is not worth a production change unless the wider
corpus (val300 is a sample) shows a meaningful PSD population.

## Numbers

- legacy total: 3119; byte-identical: 333; mid (1k–64k): 1992 (1449 ours-bigger,
  543 ours-smaller; mean Δ +1673).
- mid cluster: **1991 LZ4HC-noise + 1 PSD-structural**.
- format: always BC7 (TF_BC7=25) for ≥4×4; size/mips/split are deterministic and
  matched.
