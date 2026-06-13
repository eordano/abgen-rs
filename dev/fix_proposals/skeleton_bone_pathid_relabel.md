# Skeleton bone PathID relabeling on skin+animation bundles — NEGATIVE FINDING

> **Status: investigated, no code change.** The GameObject/Transform/
> SkinnedMeshRenderer `localIdentifierInFile` (LFID) values for bundles that
> combine a glTF `skin` **and** a glTF `animation` are not reproducible from
> any clean-room pure function of the node name-path. The recycle-name → LFID
> model that is byte-exact for static meshes (and for skinned meshes *without*
> animation) does not reproduce these bundles for **any** recycle string,
> file_type, occurrence index, or hash family tried.

## Scope (val-300 windows)

16 STRUCT bundles carry the relabeling. Per the parity census the
SkinnedMeshRenderer is flagged on 11 glb-wearable; the same balanced
relabeling also hits 7 glb-animated and a handful of "other". The diffs are
**structurally balanced** (`+GameObject/+Transform == -GameObject/-Transform`
with identical names) — the same logical bones exist on both sides, only the
PathIDs differ, which then cascades through `SkinnedMeshRenderer.m_Bones[]`,
`Transform.m_Father/m_Children`, `Avatar.m_TOS`, `AssetBundle.m_Container`
and the PreloadTable.

The single discriminator, verified by inspecting the source glTF of every
STRUCT-SMR bundle and contrasting with the 268 INPLACE (matching) SMR
bundles: **failing iff `skins>=1` AND `animations>=1`.** Skinned meshes
*without* animation match. Bone hierarchies *without* a `skin`
(e.g. the rover.glb case in `landed/empty_scene_name_wrap.md`) match.

## What was ruled out (all confirmed, not assumed)

1. **GUID mismatch — NO.** PrefabPackedIdentifiers packs the asset hash's top
   2 bytes into the result (`prefab_packed_path_id`'s `headerSize<4` branch).
   OURS and REF share those top-2 bytes on every object of every failing
   bundle (`f4a4` for enemyYellow.glb, `7d03` for the Tygloo bundle), so
   `assetHash = Calculate(guid, filePath)` is identical → the deterministic
   GUID `md5(cid)` set by the `abc-deterministic-guids`
   `SetDeterministicAssetDatabaseGuid` IS applied and matches ours.

2. **Node hierarchy / names mismatch — NO.** A bundle tree dumper
   (GameObject name + Transform father/children) shows OURS and REF have
   byte-for-byte identical hierarchies and names, including GLTFast's
   duplicate-sibling uniquification (`Ennemy_2_Finalized_Mesh` +
   `Ennemy_2_Finalized_Mesh_0`). Only the PathIDs differ.

3. **Duplicate-sibling renaming — NOT the cause.** The Tygloo bundle has zero
   duplicate names and still relabels all 112 GO/Transform objects.
   (Aside: abgen's dedup suffix `_dup{n}` from 1 is wrong vs GLTFast's
   `_{i}` from 0 per `GltfImport.GetUniqueNodeName` — a real but separate
   correctness nit that does not move parity because these bundles do not
   match anyway.)

4. **Recycle string — exhausted.** For REF's `Body`/`Hips`/root LFIDs, brute
   force over: every prefix built from up to 5 segments drawn from
   `{"", "Scene", "scenes", "Asset", node-name, "GameObject"}`; the empty
   scene-name forms (`scenes/`, `scenes//Scene`, `scenes//<node>` — i.e. the
   no-Scene-layer form that the GltfImporter single-root path actually
   produces); short_types `{GameObject, Transform}`; file_types `{-1..6}`;
   occurrence indices `{0..4}`; and hash families `{xxh64, md4-lo64,
   md5-lo64, spooky}` over `{"Type:T->rec0", "Type:T->rec", "rec",
   "T->rec"}`. **Zero matches.**

5. **Sequential / small LFID — NO.** Brute force of the raw `localIdentifier`
   integer over `[-5e6, 5e7]` for all sensible file_types reproduces none of
   the REF bone PathIDs, so they are not Unity's small importer-sequential
   ids either.

## Root cause (best supported)

The static-mesh recycle model `prefab_packed_path_id(md5(cid),
xxh64("Type:T->scenes/.../{node-path}0"), 3)` reproduces Unity's
ScriptedImporter recycle-name LFID table for prefab child objects. When the
imported glTF contains **both** a `skin` and an `animation`, Unity assigns the
GO/Transform/SMR `localIdentifierInFile` from a *different* generator than the
recycle-name table. This is consistent with the definitive cross-session probe
in `landed/addobjecttoasset_pathid_probe.md` (717/717 sub-asset LFIDs differ
across two runs of the same project): the importer's fallback id generator is
instance-id / editor-session-state driven and is **not a pure function** of
`(guid, name, type)`. The skin+animation path (Animation component on the
retained wrapper + bone rebinding via `SkinnedMeshRenderer.bones`) routes the
prefab subtree through that fallback, so the LFIDs are not recoverable
clean-room.

## Why it is irreducible here

- The GUID and the full node/name structure already match; the *only* free
  variable is `localIdentifierInFile`.
- That LFID is not a pure function of any name-path string (exhausted above)
  and not a small sequential id, and the only remaining generator is the
  proven-nondeterministic `AddObjectToAsset` fallback. Recovering it would
  require Unity-internal instance-id state — outside the no-disassembly,
  no-per-CID-lookup rules.
- This is the same class of wall as the AssetBundle PreloadTable ordering
  residual: a value Unity emits that is content-indistinguishable from the
  clean-room side.

## Separate finding: the QmRgBic… "other" outlier is a different bug

`QmRgBic6Tpx8433sHntMpVhirHPcwd2i2eMqUr6nyK8DPX /
QmRLmgszChsoahGBW1KPTgYTdhCQ7MA2HTRWBBdBXimSXr` (ours 988 KB vs ref 7 KB) is
NOT the relabeling bug. Its source glb (`skins:0, animations:1`, root node
named `Armature`) makes REF emit a **skeleton-only** bundle: REF strips the
`Armature` wrapper and the mesh, keeping only the `Avatar_*` bone
GameObjects/Transforms (7 KB). OURS keeps `Armature` and embeds the full mesh
(988 KB). This is a converter skeleton-extraction behavior to be handled on
its own, independent of the LFID-hash question above.

## Reproduction notes

- Bundle tree + objalign show identical names, differing PathIDs.
- assetHash top-2-byte equality proves same GUID:
  `struct.pack('<q', pid)[:2]` is constant across all objects of one bundle
  and equal OURS↔REF.
- Recycle/LFID brute force used a throwaway `examples/recycle_recover.rs`
  driver plus an `ABGEN_DUMP_RECYCLE` env hook in
  `builder.rs::finalize_pathids` (both reverted; not shipped).

## Files

- ADDED: this file. No source change — the cap is gated on a
  non-derivable Unity-internal LFID generator.
