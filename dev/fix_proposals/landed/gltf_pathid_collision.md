# PathID collisions on duplicate sibling names (16 build_err → 1)

Closes the "16 build_err bundles" coverage gap called out in
`size_delta_v2.md` §6. The original v2 report attributed the failures to
external-buffer.gltf URI resolution — that turned out to be wrong on
investigation:

## Real failure categorization

Captured stderr for all 16 failing CIDs on the 22-entity windows test set
(2,174 bundles). Two distinct failure modes:

### 15 × `PathID collision` (hard error)

abgen-rs builds the role's recycle name as
`scenes/{scene}/{wrap}/.../{node_name}` and hashes it via
`pathids::local_id_for_recycle_name`. When two sibling glTF nodes share a
name, both produce identical recycle names → identical PathIDs → hard
`PathID collision` error in `finalize_pathids`. Sources:

| failure pattern | example |
|---|---|
| Two same-named sibling meshes (e.g. `Cube`/`Cube`) | `bafybeidqeiyphhrnyzxceccmdrh3jw4vbeg5yhgzaj22auy6ucbm4hujde` (`models/jb.glb`) |
| All-unnamed nodes → all become `"GameObject"` (3-node `Duck.gltf`) | `QmZUcLwo84BDg5QCJMwryaTyZghJG562gwZGQnCKwprKVs` |
| SketchFab OSG_Scene chains with `RootNode (gltf orientation matrix)/...` levels of unnamed `GameObject` nodes (8 cases) | `bafkreia2wm6mgbnlxlgrrjeomtc4aqgxtx26233lw3vdstlzxizeezoseq` |
| Text-MoCap meshes with letter-per-prim (`Text/1/VISIT/I`, `Text/1/Community/m`) | `bafkreicizbl26hfiueen37pjgiftn6m2itmsjtznasj7qkgcd564pfc37y` |
| Skeleton with sibling `neutral_bone` Transforms | `bafybeihpdza2ir4xoilasd6tlp25lej3p4wyokecrkwloh7zuj7bvwl4rm` |

The reference assigns distinct PathIDs to same-named siblings (the
deterministic-guids `PrefabPackedIdentifiers` strategy in
`com.unity.scriptablebuildpipeline` keys off the internal Object
serialization, not the gameObject.name). We don't reproduce that
discriminator — the abgen-rs scheme only sees the recycle-name string —
so we get hash collisions on duplicates.

### 1 × `KHR_draco_mesh_compression` panic

`bafybeihki3l76y6zvdcdhecj6h6wrp3py6f2ndosnaebpxhbs2kw7xb52q`
(`models/space-elevator.glb`) has `extensionsRequired: ['KHR_draco_mesh_compression']`.
4,587 of 4,983 accessors carry no `bufferView` (the bytes live in the
draco extension's `bufferView` and need a draco decoder to materialize).
abgen-rs has no draco decoder, so `read_accessor` panics on
`accessor bufferView`.

## Fix

`src/builder.rs::Builder::set_obj` + new `insert_role`:

* When inserting a `Role::Glb(short_type, recycle)` at a pid, check
 `glb_role_keys[(short_type, recycle)]`. If a different pid already
 claimed that key, rewrite the role's recycle to
 `{recycle}_dup{N}` (N starting at 1) and re-claim.
* Re-setting the same `(pid, role)` is a no-op (Transform is normally
 set as placeholder then re-set with full tree — both insertions match).
* `Role::Bundle/Mat/Tex/Meta` are unique by construction; pass through.

`src/gltf.rs::load_gltf_inputs`:

* Bail with a clean `unsupported glTF extension: KHR_draco_mesh_compression`
 error when that extension is in `extensionsRequired`. Prevents the
 downstream `read_accessor` panic — converts a crash into a categorized
 build_err.

The disambiguated bundles do NOT match prod for the duplicate nodes'
PathIDs (we don't know what the reference uses to disambiguate). They DO build
and load — converting 15 hard failures into bundles that contribute a
small PPM-bits residual rather than counting as build_err.

## Measurements (22-entity windows test set, 2,174 bundles)

Before:

```
 abgen-rs build error : 16
 paired & compared : 2158
 raw-byte identical : 28
 PPM-BITS DIFFER : 464666.2
```

After:

```
 abgen-rs build error : 1 (-15)
 paired & compared : 2173 (+15)
 raw-byte identical : 28 (=)
 PPM-BITS DIFFER : 468231.4 (+3,565 ppm)
```

The +3,565 ppm comes from the 15 newly-built bundles — we don't match
prod on duplicate-sibling PathIDs, but the bundles are now byte-paired
instead of dropped. Trade-off:

* 0.7 % more bundles in the corpus (15 / 2173)
* +0.77 % PPM-bits on the corpus
* No regression on the 280 known-good fixtures (parity_bytes ceiling
 773,032 unchanged)

## Out of scope / future work

* **Reverse-engineer the `PrefabPackedIdentifiers` discriminator** —
 would let duplicate-sibling PathIDs match the reference (and shave the +3,565
 ppm). Requires probing the SBP source (`com.unity.scriptablebuildpipeline`'s
 `PrefabPackedIdentifiers.SerializationIndexFromObjectIdentifier`
 uses MD5/SpookyHash of the `ObjectIdentifier` — different algorithm
 than abgen-rs's `xxh64("Type:T->name0")` scheme — so the disambig is
 not just a suffix append on the string).
* **Draco decoder** — the single remaining build_err. Adding a draco
 decoder via a Rust crate (`olm-rs-draco` or wrap libdraco) would
 let `space-elevator.glb` parse, but the produced mesh attributes
 would need to also exactly match the converter's GLTFast draco decoder
 output for byte parity — another reverse-engineering project.

## Files touched

* `src/builder.rs::Builder` — new `glb_role_keys` field + `insert_role`
 helper + `dedup_glb_role` private method; `set_obj` rewritten to
 route through dedup. Two `self.roles.insert` sites in `build_node`
 switched to `insert_role`.
* `src/gltf.rs::load_gltf_inputs` — early bail on
 `extensionsRequired: KHR_draco_mesh_compression`.
* `tests/parity_bytes.rs` — unchanged (ceiling 773,032 verified).
