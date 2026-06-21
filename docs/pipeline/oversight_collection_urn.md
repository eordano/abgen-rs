# Collection-URN vs per-CID conversion: what diverges and how to validate it

**Why it matters:** All abgen-rs parity validation runs against the per-CID
reference (`ConvertEntityById`). The collection-URN entry point
(`ConvertWearablesCollection`) emits *different bytes* for the same wearable
content, and abgen-rs has an `ABGEN_COLLECTION_MODE` switch that models that
difference. This note maps the concrete divergences and how they were gated.

**Status update (gated against a real corpus).** There IS now a collection
reference on disk: `abc-abgenrs-799967c3-2026-06-20/collection-base-avatars-windows` (547
bundles, the base-avatars collection built by
`ExportWearablesCollectionToAssetBundles`). Build the matching set with
`abgen-corpus --collection-urn urn:decentraland:off-chain:base-avatars
--lambdas-url <lambdas-endpoint> <out> --platform windows` (resolves
member wearables via the local lambdas, flat `<assetHash>_windows` output, sets
`ABGEN_COLLECTION_MODE`). Verify by wrapping both flat dirs under a synthetic
`collection/` entity subdir (abgen-verify walks `<root>/<entity>/<bundle>`).
First gate: 547 bundles, byte-id 78/547 after the DCL_Scene fix below
(was 52 before; the wearable class went 0 -> 26). Residual is value-noise
(median |on-disk delta| = 1 byte, all within +-18B; objalign shows equal-size
DIFF objects = BC7 texel + preload PPtr re-compression) plus one streaming wall
(section 3).

## 1. What the two modes do differently in abgen-rs

`ABGEN_COLLECTION_MODE` (env `ABGEN_COLLECTION_MODE`, set by `abgen-corpus
--collection-mode` / implied by `--collection-urn`) changes exactly one thing in
the builder, plus the corpus driver wires the bundle set up differently:

- **Default material forced on (builder.rs) — VERIFIED.** In `Builder::build`
  the unconditional `self.default_material()` emission is gated on
  `v38_compat() || collection_mode()`. That always materialises a `Material`
  object named `DCL_Scene` and pushes a `DCL_Scene.mat` container entry — even
  for glbs that, in per-CID fork mode, would never touch the default. In per-CID
  mode `default_material()` is only reached from `material_inner` for a primitive
  with `material = -1` or an out-of-range index, *and only when `scene.materials`
  is non-empty*. So the modes diverge on the default-material object/container
  entry for the whole class of glbs that have real materials on every primitive
  (the common case) — collection mode emits an extra unreferenced `DCL_Scene`
  Material + `DCL_Scene.mat` entry that per-CID does not.

  **This is now confirmed against the reference, not a guess.** All 309
  glb-wearable bundles in `collection-base-avatars-windows` carry a `DCL_Scene`
  Material; abgen emitted ZERO before the gate was extended to `collection_mode()`
  (it had a stale `if v38_compat()` gate plus an "unverified — collection mode
  must not force the default" comment, which the corpus disproves). Mechanism:
  the converter's `GltfImportWrapper.defaultMaterial` is a lazy-*creating* getter
  (`if (importer.defaultMaterial == null) importer.defaultMaterial =
  materialGenerator.GetDefaultMaterial();`), so
  `ExtractEmbedMaterialsFromGltf`'s `if (gltfImport.defaultMaterial != null)`
  gate is ALWAYS true and `Materials/DCL_Scene.mat` is written for EVERY glb,
  then swept into the bundle by the folder-level marking. This is the same
  mechanism as production-v38. (Curiously the per-CID FORK corpus — val300 — does
  NOT carry it on zero-material glbs; the asymmetry between the fork's own
  per-CID and collection runs is not yet fully explained, but the collection
  behavior is what the reference shows and what abgen now reproduces.)

- **Bundle set is flat, per content-hash (abgen-corpus.rs `from_collection_urn`).**
  Per-CID groups bundles under `<out>/<entity_id>/<bundle_name>`; collection mode
  forces `--flat`, writing every bundle as `<out>/<hash>_<platform>` with a global
  `seen_bundle` dedup across all wearables in the collection. One bundle is emitted
  per glb and per listed image, deduped by content hash. This mirrors the
  converter flattening every wearable's representation contents into one
  `rawContents` list (`WearablesClient.GetMappingPairs`) and the AB pipeline
  deduping by hash.

- **Naming.** Collection bundles are always `<hash>_<platform>`; the per-entity
  scene manifest (`manifest::write_scene`) is *not* written in collection mode.

Everything else — mesh/texture/material encoding, externals, typetree — runs
through the identical `build_bundle` path. The `entity_type` passed per bundle in
collection mode is hardcoded to `"wearable"` (see divergence #2 below).

## 2. Known / likely divergences from the real collection reference

### 2a. AnimationMethod: abgen-rs models the WRONG one (likely real byte divergence)

This is the load-bearing finding. In the upstream converter
(`AssetBundleConverter.GetAnimationMethod`, Apache-2.0, clean-room read):

```
if (entityDTO == null) return AnimationMethod.Legacy;
if (isWearable)       return AnimationMethod.None;
if (isEmote)          return AnimationMethod.Mecanim;
return settings.AnimationMethod;
```

`entityDTO` is `conversionParams.apiResponse`. The per-CID/per-pointer entry
points set `apiResponse = apiResponse[0]` (a real entity DTO with a `type`
field). **`ConvertWearablesCollection` sets NO `apiResponse`** — it builds
`ConversionParams { rawContents = mappings }` only. So in the collection path
`entityDTO == null`, and `GetAnimationMethod` returns **`AnimationMethod.Legacy`
for every glb in the collection.**

Note the trap: `ConvertWearablesCollection` sets `settings.isWearable = true`,
but that flag is consumed only by the Sentry error-reporter tag. The
`isWearable`/`isEmote` locals in the conversion loop are recomputed from
`entityDTO.type` (null here) — `isWearable` is therefore `false`, and `isEmote`
falls back to the `Utils.IsEmoteFileName(fileName)` filename heuristic. With
`entityDTO == null` the very first line short-circuits to `Legacy` regardless.

Consequence:

- **Per-CID wearable** (validated path): `type:"wearable"` → `AnimationMethod.None`
  → GLTFast imports no animation, the single scene root collapses
  (`useFirstChild`) → skeleton-only bundle, no AnimationClip/Animation objects.
  abgen-rs models this exactly (`is_wearable` suppresses `has_anim`,
  builder.rs ~1342).
- **Collection wearable** (unvalidated path): `entityDTO == null` →
  `AnimationMethod.Legacy` → a wearable glb that carries a glTF `animation` would
  get Legacy AnimationClip + Animation(111) objects emitted and the root
  wrapped/animated.

**abgen-rs gets this wrong in collection mode.** `from_collection_urn` hardcodes
`entity_type: Some("wearable")` on every BundleSpec, and the builder turns that
into `is_wearable = true` → `AnimationMethod.None` behavior. To match the real
collection reference it would need to pass `entity_type: None` (so the builder
takes the `Legacy` branch — `!is_emote && !is_wearable`, builder.rs ~1476), and
fall back to the `_emote.glb` filename heuristic for emotes, exactly as the
converter does when `entityDTO == null`. This divergence only changes bytes for
wearable glbs that actually carry a glTF animation; static wearables (the
majority) are unaffected, which is probably why it has gone unnoticed.

### 2b. Default-material emission: plausibly an over-emit

abgen-rs collection mode forces `DCL_Scene` / `DCL_Scene.mat` for every glb.
Upstream, the default material is emitted only when
`gltfImport.defaultMaterial != null` (`ExtractEmbedMaterialsFromGltf`), and that
field is populated lazily by GLTFast only when a primitive lacks a material. The
per-CID rule abgen-rs already documents (builder.rs ~930-945, verified against
the val300 windows reference: `DCL_Scene` in 1 of 10,675 bundles) is that the
default appears *only* for material-less primitives in a glb that still has a
non-empty `materials` array. There is no clean-room evidence that the collection
path changes that GLTFast gate — `isWearable=true` does not touch material
extraction. So the unconditional `default_material()` in collection mode is
likely an **over-emit** that would produce a spurious extra Material object +
container entry on most collection bundles. This is the second concrete suspect
for a byte/object-count divergence; it should be re-derived against a real
collection reference rather than assumed correct.

### 2c. The bundle-count gap is NOT a code divergence (confirmed negative)

Documented in `urn_bundle_gap_session.md`:
building the base-avatars collection by URN emits fewer bundles than the Unity
reference. This is **content-version drift**, not a missing emit rule. Bundles
present only in the reference are stale content hashes superseded since the
reference was generated; bundles only in abgen-rs are the newer content. The
embedded-texture hypothesis (parse glb image URIs, emit an extra standalone
texture per resolved image) is empirically false — every embedded image URI
already resolves to a top-level DTO content key, and the converter builds its
content map purely from representation `contents` lists
(`WearablesClient.GetMappingPairs`), never enumerating embedded URIs. abgen-rs
mirrors this (one bundle per glb + per listed image). No source change closes
the gap; only re-pinning the original collection DTO snapshot does.

## 3. What validating the collection path would require

There is no collection-URN reference corpus (only the per-CID validation
references). To actually validate:

1. **Generate a collection reference with the SAME converter build** used for the
   per-CID corpus, driving `ConvertWearablesCollection` on a chosen collection
   URN. Must run under the converter project's pinned Unity editor version
   (newer editor lines auto-upgrade the project and break the converter).

2. **Pin the collection DTO snapshot.** Because of the 2c drift, the reference and
   abgen-rs must see the *same* lambdas `collections/wearables` response. Capture
   the DTO JSON at generation time and replay it (the corpus driver already reads
   the same shape via `--collection-urn` / `from_collection_urn`). Otherwise the
   bundle sets won't even share names and parity intersection is empty.

3. **Have the referenced content in the local store.** `from_collection_urn`
   skips any hash not present (`store.exists`), so the content root must hold every
   glb + image hash in the pinned DTO.

4. **Fix the two suspected divergences before scoring, or the scoreboard will be
   dominated by them:**
   - Pass `entity_type: None` (Legacy) instead of `"wearable"` in
     `from_collection_urn`, with the `_emote.glb` filename fallback (matches
     `entityDTO == null`). Re-check against the reference whether Legacy
     AnimationClip/Animation objects appear.
   - Re-derive whether the unconditional `default_material()` in collection mode
     is correct or an over-emit, against the real reference.

5. **Wire collection bundles into the parity scoreboard/gate** (currently per-CID
   only). The byte-compare harness already intersects by bundle name, so once the
   names match (step 2) the matched bundles can be scored directly.

`ABGEN_COLLECTION_MODE` is now **validated against
`collection-base-avatars-windows`**: the bundle-set wiring (flat, per-hash,
dedup), the Legacy AnimationMethod, and the unconditional `DCL_Scene` default
material are all confirmed correct. 78/547 byte-id at the first gate; the
remaining residual is the texel/preload value-noise wall plus the streaming wall
in section 3.

## 3. Standalone-texture streaming wall (NOT yet derivable)

After the DCL_Scene fix, the only *structural* (raw-length-different) residual
in the collection corpus is 34 standalone-texture bundles where the **reference
stores the texture INLINE** (`m_IsReadable=true`, `image data` populated,
`m_StreamData.size=0`) but abgen **streams it to `.resS`**
(`m_IsReadable=false`, `m_StreamData.path=archive:/.../CAB-....resS`).

abgen's rule is `do_stream = model_referenced && format==BC7` (builder.rs
`StandaloneTextureBuilder::build`): a texture referenced by a glb is streamed.
The 34 mismatches are all model-referenced PNGs (e.g. `cool_hair/Image_0.png`,
facial-feature `*_Eyes_*` / `*_Eyebrows_*` / `*_Mouth_*` textures). The fork's
`ImportTextures` (AssetBundleConverter.cs ~1504) sets `isReadable=true` for
standalone content textures, which forces inline data — but only for **some** of
them: in the same corpus 127 other model-referenced standalone PNGs are streamed
in the reference, at the **same dimensions** (512x512 appears in both the
inline and streamed sets) and even the same content key
(`Avatar_FemaleSkinBase.png` shows up inline for one wearable, streamed for
another). The `_00` base variants of eyes/eyebrows/mouth stream while `_01+`
inline. No filename / dimension / colorspace rule separates the two sets.

This is the per-asset Unity `m_StreamingMipmaps` / streaming-decision wall (same
family as the AssetBundle preload-ordering wall) — a build-pipeline state baked
at authoring time, **not derivable from glb/content**. Cross-tab: 0 false-inline
(ref-stream/ours-inline=0), so abgen never wrongly inlines; the only error
direction is streaming-when-it-should-inline. Attribution is clean; no abgen
defect, nothing to fix here without a per-asset table.

## 4. Done / superseded TODOs

The original "fix the two suspected divergences" and "wire into the scoreboard"
items below are LANDED: collection mode uses Legacy (`entity_type: None`) and the
DCL_Scene default is gated on `collection_mode()` and verified correct. The
historical notes are kept for context.
