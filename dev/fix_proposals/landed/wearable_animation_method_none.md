# Wearable glbs use `AnimationMethod.None` — skeleton-only bundle emission

> **Status: LANDED + byte-verified.** Fixes the 3 "other"-kind over-emission
> outliers (ours 300 KB-988 KB vs ref ~7 KB). All three now byte-exact; the
> entire `other` kind is byte-identical (38/38, 0 diff-bits) with zero
> regression to any other kind.

## TL;DR

A handful of avatar/emote entities are stored as `type:"wearable"` (old
`emoteDataV0` items) but their glb carries `meshes:0, skins:0, animations:1`
and a single root node named `Armature`. The **reference** emits a
*skeleton-only* bundle (~7 KB): just the bone GameObject/Transform hierarchy,
**no** AnimationClip, **no** Animation component, and the `Armature` root
collapsed into the hash-named bundle root. OURS was emitting the full legacy
`Animation` + `AnimationClip` (the glb's 1 animation, ~1-3 MB uncompressed)
*plus* a redundant `Armature` GameObject/Transform → 300 KB-988 KB bundles.

## The converter rule (SOURCE)

`asset-bundle-converter/Assets/AssetBundleConverter/AssetBundleConverter.cs`
`GetAnimationMethod(bool isEmote, bool isWearable)`:

```csharp
if (isWearable) return AnimationMethod.None;   // <-- checked FIRST
if (isEmote)    return AnimationMethod.Mecanim;
return settings.AnimationMethod;               // Legacy
```

`isWearable = entityDTO.type.ToLower().Contains("wearable")`. Because the
`isWearable` branch is checked **before** `isEmote`, an emoteDataV0 item
stored with `type:"wearable"` (and whose filename is not `*_emote.glb`, so
`Utils.IsEmoteFileName` is false too) is converted with
`AnimationMethod.None`.

`AnimationMethod.None` propagates into GLTFast's `unity-gltf`
`Editor/Scripts/GltfImporter.cs`:

```csharp
var hasAnimation = false;
#if UNITY_ANIMATION
    if (importSettings.AnimationMethod != AnimationMethod.None
        && (instantiationSettings.Mask & ComponentType.Animation) != 0) {
        var clips = m_Gltf.GetAnimationClips();
        if (clips != null && clips.Length > 0) hasAnimation = true;
    }
#endif
...
// SceneObjectCreation.WhenMultipleRootNodes && !multipleNodes:
useFirstChild = !hasAnimation;   // single root -> collapse it as the prefab root
```

So with `AnimationMethod.None`:

1. No `GetAnimationClips()` path runs → **no AnimationClip / Animation**
   objects are created.
2. `hasAnimation == false` for a single-root scene → `useFirstChild == true`
   → the single root node (`Armature`) **becomes** the prefab root (and is
   renamed to the bundle hash), rather than being wrapped under a synthetic
   scene GameObject. The `Armature` name disappears.

Result: a pure bone skeleton (root container + 64 `Avatar_*` bones), ~7 KB.

## Recoverable verdict + evidence

**Fully recoverable, clean-room.** The rule is a pure function of
`entity.type`, which abgen already has (`BuildOpts.entity_type`,
populated from the catalyst entity JSON `type` field).

Object census (via a throwaway `examples/listobj`):

| side | GO | Transform | Animation | AnimationClip |
|---|---:|---:|---:|---:|
| REF  | 65 | 65 | 0 | 0 |
| OURS (before) | 66 | 66 | 1 | 1 (3.19 MB) |

The extra OURS GameObject is `Armature`; the 988 KB bulk is the AnimationClip.

**No-regression proof:** across all 577 wearable REF bundles in val-300,
**zero** contain any `Animation`/`AnimationClip`/`Animator`/`AnimatorController`
object. The converter strips animation from *every* wearable, so suppressing
wearable animation in abgen cannot regress any wearable bundle.

## The fix

`src/builder.rs`:

1. New `Builder.is_wearable` field (threaded like `is_emote`), set in
   `build_glb_with_overrides`:
   ```rust
   let is_wearable = matches!(opts.entity_type, Some(t) if t.eq_ignore_ascii_case("wearable"));
   ```
2. `has_anim` is forced false for wearables — this also flips `wrap` to false
   for the single-root case, so `build_node` collapses the single root and
   renames it to the bundle hash (the GLTFast `useFirstChild=true` behavior).
3. The legacy AnimationClip+Animation emission branch is gated on
   `!self.is_wearable` as well as `!self.is_emote`.

No `proto`/template change, no ppm-cap change.

## Before / after (val-300 windows, full corpus, `abgen-verify`)

| kind | bundles | byte-id before | byte-id after | diff-bits before | diff-bits after |
|---|---:|---:|---:|---:|---:|
| other | 38 | 35 | **38** | 16,453,077 | **0** |

All other kinds unchanged byte-for-byte. Corpus totals:
byte-identical 2318 → **2321** (+3), diff-bits 5,923,804,602 →
**5,907,351,525** (−16,453,077).

The 3 fixed bundles (SHA256-equal to reference):

- `QmRgBic6Tpx8433sHntMpVhirHPcwd2i2eMqUr6nyK8DPX/QmRLmgszChsoahGBW1KPTgYTdhCQ7MA2HTRWBBdBXimSXr_windows` (988 KB → 7317 B)
- `QmZeaKv235kQYxJYTLG4E9nRxRj4wyYnBK1kw3zxFiBPvh/QmNToefuspk33pzgDb2umLkGrmDgEo9Nix2CWF6jt4Px5o_windows` (742 KB → 7338 B)
- `QmQfQJQomTAo4LHk2nmhXzi1VKq8Dy59jnRopr7uMHfuP1/QmPUHGuKNSNfnHxNqNYBNBMgjA9Bna4vRba8FmVrKiF2u2_windows` (338 KB → 7342 B)

`cargo test --release --test parity_bytes` → 2 passed.

## Relation to prior docs

This supersedes the "Separate finding" tail of
`skeleton_bone_pathid_relabel.md` (which mis-attributed the outlier to a
"full mesh embed" — there is no mesh in these glbs; the bulk was the
AnimationClip). The skin+animation LFID wall documented in that file is a
*different* class of bundle (`skins>=1 AND animations>=1`) and is untouched.

## Files

- `src/builder.rs` — `Builder.is_wearable` field + constructor param;
  `has_anim` and the legacy-AnimationClip branch gated on `!is_wearable`;
  `is_wearable` computed from `opts.entity_type` in
  `build_glb_with_overrides`.
- ADDED: this file.
