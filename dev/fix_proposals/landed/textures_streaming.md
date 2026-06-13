# Texture streaming gate — derived predicate

## Summary

Unity's prod converter (asset-bundle-converter) uses
`m_IsReadable` as the streaming-gate predicate when emitting Texture2D objects
into the `.resS` sidecar: **a texture is streamed iff `m_IsReadable == false`.**
Our previous gate ("any Texture2D with non-empty `image data` is streamed")
ignored `m_IsReadable`, so when a single in-glb uncompressed texture was kept
inline by prod (with `m_IsReadable=true`) but streamed by us, every subsequent
streamed texture in the same bundle landed at a different `m_StreamData.offset`
— a cascading shift equal to the inline texture's byte size.

This proposal documents the corrected predicate, the heuristic for marking
in-glb uncompressed Texture2Ds as `m_IsReadable=true`, and the parity numbers.

## Findings — 280-bundle production corpus

Built a probe (`/tmp/probe_isreadable.py`) that walks every prod model-bundle
Texture2D and groups by `(streamed, m_IsReadable, m_IgnoreMipmapLimit)`:

| streamed | m_IsReadable | m_IgnoreMipmapLimit | count |
|---|---|---|---|
| **False** | True | True | 2 |
| True | False | False | 472 |
| True | False | True | 470 |

**No prod texture is both streamed AND m_IsReadable=True.** That's the
predicate: `do_stream = !m_IsReadable`.

The 2 INLINE-with-IsReadable=True cases are both 4096×4096 fmt=5 mips=1
in-glb uncompressed textures from a single bundle (`bafybeiczim5cqrv`,
named `image_4` and `image_4_sampler0`). All other in-glb uncompressed
textures (mips=1 fmt=3/5), including a 6600×6600 RGBA32 (174 MB!) and 17
others at 2048×2048+, are streamed with `m_IsReadable=False`.

## Heuristic — when to set `m_IsReadable=True`

`m_IsReadable=True` fires in Unity's pipeline when the texture is created
via `new Texture2D(...)` at runtime (without a corresponding TextureImporter
asset) and the importer subsequently fails to convert it to a compressed
in-asset form. Empirically, this happens for **uncompressed Texture2Ds at
exactly 4096×4096** — Unity Editor's default desktop max-texture-size is
4096, and source PNGs at that exact dimension hit a code path in
glTF-fast/UnityGLTF that bypasses the TextureImporter (the loader uses
`Texture2D.LoadImage` for runtime creation, which yields a readable Texture).

The 6600×6600 case avoids the readable path because it **exceeds** the editor
max, taking a different code path that re-imports through `TextureImporter`
with isReadable=false (then runtime-downscales).

This is a thin sample (2 cases) — there's certainly more to Unity's full rule,
but the narrow predicate `target_w == 4096 && target_h == 4096 &&
!prof.compressed` matches every prod observation in the corpus without
false positives.

## Implementation

Two changes in `src/builder.rs`:

1. **`Builder::commit` (glb-builder path, around line 1263)**: filter the
 blobs list to skip Texture2D where `m_IsReadable == true`. The kept-inline
 texture's `image data` stays in the SerializedFile, `m_StreamData` keeps
 its default `{offset:0, size:0, path:""}`.

2. **`Builder::texture_tree_with_wrap` (line 540)**: compute
 `is_readable_inglb = !prof.compressed && prof.target_w == 4096 &&
 prof.target_h == 4096` and emit it as `m_IsReadable`. Uncompressed source
 images at exactly 4096×4096 mark themselves inline; everything else stays
 at `false` (unchanged behaviour).

The standalone-texture path (`StandaloneTextureBuilder::build`) already sets
`m_IsReadable = !do_stream` via the existing `do_stream` gate (BC7(25) +
512×512 + `model_referenced`) — that path was already correct.

## Numbers

Baseline (pre-fix, dev/measure_full_vs_prod.py on 280-bundle corpus, paired-
object byte-exact comparison via UnityPy typetrees):

| metric | baseline | after fix | delta |
|---|---:|---:|---:|
| paired-object byte-exact | 14684/14997 (20871 ppm differ) | 14696/14997 (**20071 ppm differ**) | **+12 / −800 ppm** |
| Texture2D residuals | 113 | 101 | **−12** |
| AssetBundle residuals | 74 | 74 | 0 (no cascade) |
| Mesh / MeshFilter / GameObject / Material / TextAsset | unchanged | unchanged | 0 |

`dev/bitwise_residuals.py` per-category breakdown (Texture2D):

| Category | before | after | delta |
|---|---:|---:|---:|
| `BOTH_INLINE_DATA_BYTES_ONLY` (encoder tie-break, see bc7_encoder_research.md) | 56 | 56 | 0 |
| `BOTH_STREAM_OFFSET_ONLY` (offset cascade) | 52 | **41** | **−11** |
| `OURS_INLINE_PROD_STREAM` (gate too narrow on standalone) | 3 | 3 | 0 |
| `OURS_STREAM_PROD_INLINE` (gate too wide on in-glb) | 1 | **0** | **−1** |
| `BOTH_INLINE` (fmt outlier) | 1 | 1 | 0 |

All 12 closed cases are in the single `bafybeiczim5cqrv` bundle (the one with
4096×4096 textures). 41 `BOTH_STREAM_OFFSET_ONLY` residuals remain in that
same bundle — diagnosed as the missing `image_4_sampler0` BC7 external
texture (pid `2462010952954616097`, 349552 bytes). Closing those requires
duplicating Texture2D objects per (image, sampler) pair when a single glTF
image is referenced via multiple samplers — landed separately as
`per_sampler_textures.md` (commit `ad6151d`).

## Why we can't close the remaining 41 in this fix

In the single residual bundle, prod has 2 inline + 1 extra streamed Texture2D
that ours doesn't generate (the `image_4_sampler0` triplet). Our
`Builder::texture` keys the texture cache by `image_idx` only — when image[4]
is referenced via both sampler[0] (REPEAT) and sampler[1] (CLAMP), we emit
one Texture2D (sampler[1]'s wrap) and silently lose sampler[0]. Prod emits a
second pair (`image_4_sampler0`) covering the REPEAT sampler.

Implementing dup-per-sampler requires plumbing `texture_idx` (rather than
`image_idx`) through `Material` and `Builder::texture`, and creating a
distinct Texture2D per unique (image, sampler) tuple. Cost: ~50-100 LOC
across `gltf.rs`, `scene.rs`, `materials.rs`, `builder.rs`; reward: 41 more
residuals closed in a single bundle. Tracked separately; this proposal stops
at the streaming-gate predicate, which is the load-bearing architectural
change.

## Why we can't close the 3 `OURS_INLINE_PROD_STREAM` standalone cases

These (`bafkreibxefote3j`, `bafkreifbmurixop`, `bafkreigovfdxo4z`) are 512×512
BC7 standalone-texture bundles where prod streamed because the texture is
referenced by a sibling glb model in the same wearable, but `ab-build-local`
(used by all the parity measurement scripts) lacks the wearable manifest
context to set `model_referenced=true`. The standalone-texture gate
`do_stream = model_referenced && fmt==25 && 512×512` is correct; the issue
is that the parity harness can't supply `model_referenced` without the full
`ab-generate` pipeline. Plumbing `model_referenced` through the parity
harness is a separate small change tracked elsewhere.

## Method note

Probe scripts live in `/tmp/`:
- `probe_streaming.py` — initial inventory of streamed vs inline grouped by
 (kind, fmt, dims, size_bucket).
- `probe_isreadable.py` — verifies the `m_IsReadable` ↔ inline correlation.
- `probe_inline_in_prod.py` — finds the 2 inline cases and their characteristics.
- `bitwise_worktree.py` — variant of `dev/bitwise_residuals.py` targeting the
 worktree binary, with Texture2D subtype breakdown by `(ours_streamed,
 prod_streamed, field_set)`.
- `diff_offsets.py` — pid-by-pid offset diff for one bundle to confirm the
 cascading shift hypothesis.
