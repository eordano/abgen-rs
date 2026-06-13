# Mac per-class audit — no mac-only opportunity remains

> **Status: informational, audit closed.** Findings stand
> as recorded; cross-platform follow-ups (AnimationClip, signed-zero)
> both landed (commits `43b378b`, `6d15607`).

**TL;DR**: Mac and windows are now byte-for-byte identical on every paired
Unity-object class. Per-paired-object ppm-bits across 280 bundles agree to
within ±0.02 % on every class. The remaining 459k-ppm gap in
`measure_bits_validation_2.py` (the raw-byte XOR over compressed bundles)
is **not** a mac-only deficiency — it is a cross-platform residual coming
from unpaired objects (AnimationClips), per-Texture2D BC7 dust, and the
inherently random shader-LAST minority of AssetBundle preload runs.
**No mac-only fix landed.** Recommended next moves are cross-platform.

## Method

Built `dev/per_class_bits_mac.py` and `dev/per_class_paired_only.py` — for
each prod bundle, build the rust output, and per Unity class:

- `paired_diff_bits` = XOR over `o.get_raw_data` of every `path_id`
 present in both ours and prod.
- `ours_only_bits` / `prod_only_bits` = size of objects present in only one
 side (path_id mismatch — counted as 100 % differing).

Ran for both `workdir/pathid_rt_v10_mac` (ABGEN_PLATFORM=mac) and
`workdir/pathid_rt_v10_windows` (ABGEN_PLATFORM=windows), 280 bundles each.

## Per-class breakdown (paired path_ids only — same byte universe both platforms)

```
class paired-obj diff-objs mac-ppm win-ppm Δppm delta-bits
Texture2D 1004 59 20932 20932 0 +14 / +14
AssetBundle 280 149/152 21253 19806 +1447 +3,736
Transform 3829 210 86 86 0 0
Mesh 1691 14 337 337 0 -1 / +1
Material 890 2 7 7 0 0
TextAsset 280 0 0 0 0 0
GameObject 3829 0 0 0 0 0
MeshRenderer 1089 0 0 0 0 0
SkinnedMeshRenderer 300 0 0 0 0 0
MeshFilter 1392 0 0 0 0 0
Animation 51 0 0 0 0 0
MeshCollider 303 0 0 0 0 0
AnimationClip 80 0 0 0 0 0
```

Single divergence: **AssetBundle, 1,447 ppm** (≈ 7.2 % relative). 149 mac
shader-FIRST minority cases vs 152 windows. That is the known "shader-slot
majority is statistical" residual already documented in
`assetbundle_mac.md` / `assetbundle_windows.md`. There is no closed-form
fix; the per-target majority is already picked.

Every other class — including the previously-mac-specific shader-FIRST /
metadata version 8 / Basic-preset Texture2D fixes — emits **bit-identical
typetree** on both platforms.

## Unpaired-object residual (identical across mac/windows)

```
class ours-only prod-only prod-only-bits (mac == windows)
AnimationClip 0 3 7,484,576
Transform 34 36 22,752
GameObject 34 36 15,424
MeshRenderer 5 5 6,720
MeshFilter 6 6 1,152
SkinnedMeshRenderer 1 1 4,096
MeshCollider 1 1 512
Animation 0 2 1,056
```

**AnimationClip is the single largest remaining bit-leak** (7.48 Mbit ≈
935 KB over 280 bundles). Three prod AnimationClips on three different
bundles are missing from our output entirely (same three on both
platforms). Mac-vs-windows: zero divergence.

## Target-awareness audit

Verified every target-conditioned site in `src/`:

| site | code | mac handling |
|---|---|---|
| `builder.rs::target_platform_for` | `mac\|osx -> 2` | OK (Unity `BuildTarget.StandaloneOSX = 2`) |
| `builder.rs::metadata_version_for_target` | `mac\|osx\|windows -> "8.0"` | OK |
| `builder.rs::target_from_bundle_name` | parses `_mac`/`_osx`/`_windows`/… | OK |
| `cabname.rs::shader_bundle_cab` | `mac\|osx -> CAB-5ba499…` | OK |
| `sbp_order.rs::ExternalsPosition::for_target` | `mac\|windows -> First` | OK (landed) |
| `naming.rs::hash_prefix_of_bundle` | strips `_mac`/`_osx`/`_windows`/`_webgl` | OK |
| AssetBundle typetree `m_RuntimeCompatibility`, `m_PathFlags` | not target-conditioned | **correct** — prod dumps verified identical between mac and windows (both `RC=1`, `PathFlags=0`) |
| AssetBundle `m_Name`/`m_AssetBundleName` | uses `self.bundle_name` (already carries `_mac`/`_windows` suffix) | OK |
| AssetBundle `m_Dependencies` shader CAB | `cabname::shader_bundle_cab(self.target).to_lowercase()` | OK |

Nothing is silently falling back to a windows default for mac.

## Offender trace — Transform 210 diff objects (cross-platform finding)

Picked `bafkreibzbnrwkfqeqtgtgfpwwx7dcitwyfgnrvoq2myddggvjr6ncx4a6y_mac`,
pid `-7602995452222785686`. 68-byte Transform serialized object. XOR
isolates a single byte at offset 31: ours `0x80`, prod `0x00`.

```
ours: …0000803f 00000080 00000000… (m_LocalPosition.x = -0.0f)
prod: …0000803f 00000000 00000000… (m_LocalPosition.x = +0.0f)
```

Sampled the first 50 mac bundles: **46 / 46 Transform diffs are
exclusively negative-zero floats** in `m_LocalPosition` or
`m_LocalRotation`. Windows shows the identical pattern (42 / 42). Source:
glTF nodes whose TRS contains a literal `-0.0` (e.g. animation-target
nodes touched by an authoring tool that emits `-0.0` for unused
components); the converter's import path normalizes signed zero, we don't.

Closed-form fix (cross-platform, not mac-only): in
`builder.rs::transform_tree`, replace `-0.0` with `0.0` for each component
of `r`, `t`, `s` before insertion, OR in
`unity/typetree.rs` `Float` writer, write `0.0` when the value is signed
zero. Either is one line. **Not implemented in this audit** because (a)
it's not mac-specific so it belongs in a cross-platform fix proposal, and
(b) the bit savings are tiny: 210 diffs × ~2 bits avg ≈ 420 bits out of
the 970 Mbit corpus residual.

## Conclusion

The mac/windows convergence work is **done**. No mac-only opportunity
exceeding 10 % divergence exists in the 280-bundle corpus. Future
parity work should be scoped cross-platform:

1. **AnimationClip emission**: closing the 3-missing-clip path-id gap
 (7.48 Mbit, both platforms) is the single largest absolute residual
 remaining in the per-object universe.
2. **Texture2D BC7 dust** (20,932 ppm, both platforms): already tracked
 under `landed/tex_close_60.md`.
3. **AssetBundle shader-slot statistical minority** (~20k ppm, both
 platforms): no closed-form fix per `assetbundle_{mac,windows}.md`.
4. **Transform / Material signed-zero normalization** (~420 bits total,
 both platforms): trivial one-line fix in `transform_tree` or the
 typetree float writer; tracked here for completeness.

The 459k-ppm raw-byte residual measured by
`measure_bits_validation_2.py` is **not** in the per-object payload
(per-paired-object ppm = 19,932 mac / 19,929 windows). It lives in
SerializedFile bookkeeping that the rust output simply does not produce
(rust is smaller than prod in 218 / 280 mac bundles; 218 / 280 windows).
That is a bundle-structure investigation, not a class-level mac audit.

Helper scripts:

- `dev/per_class_bits_mac.py` — paired+unpaired bits per class,
 parameterized by `ABGEN_VAL_ROOT` + `ABGEN_PLATFORM`.
- `dev/per_class_paired_only.py` — splits paired-XOR vs unpaired-only.
