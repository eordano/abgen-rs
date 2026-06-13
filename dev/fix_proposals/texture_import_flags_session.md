# Texture2D import flags beyond m_ColorSpace — negative finding

> **Status: negative finding.** The Texture2D header import-flag cluster
> (m_IsReadable, m_StreamingMipmaps, m_LightmapFormat, m_ColorSpace,
> m_IgnoreMipmapLimit, m_TextureSettings: filter mode / aniso / wrap U/V/W /
> mip bias) is **already byte-exact** vs the `ad0564d-windows` reference for
> standalone-texture AND glb-embedded Texture2D objects. No header-flag fix is
> available on this corpus; the entire standalone-texture residual is BC7
> image-data encoding + resize + format/size selection, all covered by other
> landed work.

## Baseline (abgen-verify, ad0564d-windows, full corpus, j=16)

`standalone-texture` kind (my target):

```
kind                bundles  byte-id  smaller  larger    pair-bits     diff-bits     ppm
standalone-texture     2440      233     1212     988   2307829392    1138407360  493280.6
```

TOTAL across all kinds: 4243 bundles, byte-id 747, diff-bits 4,013,834,953, ppm 384,481.3.

## What I tested

The brief says: derive header import flags from the referencing slot
(albedo/normal/metallic/emissive) and DCL importer defaults, find size-MATCHED
standalone-texture bundles whose only diff sits in the Texture2D header region.

### Step 1 — partition the standalone-texture residual by size match

```
total standalone-texture     : 2440
byte-identical               :  233
size-matched but differing   :    7   (sum bits_diff = 170,484)
size-mismatched & differing  : 2200   (the other ~1.138 billion bits)
```

The header-flag hypothesis can only touch SAME-SIZE bundles (a flag flip
doesn't change the serialized size for these scalar fields). That is 7 bundles,
total 170,484 bits — 0.015% of the standalone-texture residual. The other
99.98% is size-mismatched: different m_Width/m_Height/m_TextureFormat/
m_CompleteImageSize/m_StreamData, i.e. resize + format + streaming selection,
which are NOT import flags and are owned by other landed areas (BC7 long-tail,
resize, streaming gate).

### Step 2 — locate the diff bytes in the 7 size-matched bundles (dump_decomp)

For every one of the 7, the differing byte offsets sit deep in the BC7 image
data region near the end of the CAB, never in the Texture2D header (which is at
the front). Examples (CAB size vs first diff offset):

| bundle (abbrev) | bits | CAB size | first diff offset |
|---|---:|---:|---:|
| bafkreicw4pgi… | 6 | 92,588 | 92,553 |
| bafkreigqaf6e… | 10 | 10,668 | 8,575 |
| bafkreidaid5g… | 20 | 92,588 | 92,547 |
| bafkreifniucl… | 73 | 92,592 | 90,301 |
| bafkreia4fonp… | 125 | 92,588 | 24,397 (mip data) |
| bafkreigkgkxs… | 6,400 | 354,732 | 334,973 |
| bafkreie3jlg5… | 163,850 | 354,732 | 202,477 |

All in image data, all BC7 per-block tie-break noise (the same residual class
documented in `landed/texture2d_followup.md` §"What's left" and
`landed/bc7_tiebreak_v2.md`).

### Step 3 — direct typetree field comparison (examples/dump_tex.rs, new probe)

Parsed every Texture2D (class 28) object's scalar import-flag fields for ours
vs ref and compared per-pid:

- **All 7 size-matched standalone bundles: header-field-diffs = 0.** Every flag
  (m_IsReadable, m_StreamingMipmaps[+Priority], m_LightmapFormat, m_ColorSpace,
  m_IgnoreMipmapLimit, m_IsAlphaChannelOptional, m_TextureSettings.{m_FilterMode,
  m_Aniso, m_MipBias, m_WrapU/V/W}) matches byte-for-byte.
- **12 random size-mismatched standalone bundles: 0 import-flag diffs.** Their
  diffs are confined to m_Width/m_Height/m_TextureFormat/m_CompleteImageSize/
  m_StreamData + image data — resize/format/streaming, not import flags.
- **8 random glb-wearable/glb-scene bundles: 0 import-flag diffs** (pid-aware).
  An initial field-name-keyed pass falsely flagged m_IgnoreMipmapLimit and
  m_FilterMode, but that was a comparison artifact from collapsing two
  Texture2D objects (one inline RGBA32 fmt=3 with IgnoreMipmapLimit=true/
  FilterMode=2, one BC7 fmt=25 with false/1) under the same field key. Re-run
  pid-aware: every per-pid value is identical between ours and ref.

## Why the flags already match

`src/texprofile.rs` already derives the cluster correctly:

- `standalone_texture_profile_named` sets color_space / lightmap_format from
  `is_normal` (filename heuristic), plus a per-material-slot `color_space_override`
  threaded through `StandaloneTextureBuilder` (the recent
  "m_ColorSpace from material-slot usage" commit).
- The in-glb path (`Builder::texture_tree_with_wrap`) derives filter_mode and
  wrap modes from the glTF sampler (`sampler_filter_mode`, `sampler_wrap_mode`),
  m_IgnoreMipmapLimit from the format/size path, and m_IsReadable via the
  streaming gate (`do_stream = !m_IsReadable`, `landed/textures_streaming.md`).
- m_StreamingMipmaps is uniformly 0/false and matches; m_Aniso=1, m_MipBias=0
  are DCL importer defaults and match everywhere observed.

The "what's left" item #3 in `landed/texture2d_residual_v3.md` (cs=0 / lm=3
standalone variants where the profile hardcoded cs=1/lm=0) has since been
closed by the filename-normal heuristic + slot-derived color-space override.
No residual remains in that subclass on `ad0564d-windows`.

## Conclusion / next step

Texture2D import flags are a closed area on the current corpus. The only way to
move the standalone-texture row is the BC7 image-data residual (encoder
tie-break vs bc7e ISPC + resize precision), which is explicitly a different
area (`landed/texture2d_followup.md` §"Open paths", `landed/bc7_*`). No code
change made; no fixture/threshold touched. Gate stays green by construction
(no source change).

New probe left in the worktree: `examples/dump_tex.rs` — dumps Texture2D
import-flag scalars + m_TextureSettings + m_StreamData for two bundles
side-by-side (use for any future per-pid header-flag audit).
