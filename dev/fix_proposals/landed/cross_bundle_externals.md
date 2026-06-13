# Cross-bundle external PPtr emission — protocol & implementation

Companion to `assetbundle.md`. Documents the on-disk format derived by
probing prod bundles with UnityPy and the implementation that mirrors it in
`abgen-rs` (Root Cause A).

## 1. The trigger — which textures go in which bundle

Decentraland's deterministic-guids Unity converter splits an entity's
content across multiple AssetBundles:

- One `<cid>{_<digest>}_linux` bundle per `.glb` / `.gltf`.
- One `<cid>_linux` bundle per standalone image (the
 `StandaloneTextureBuilder` path in `abgen-rs/src/builder.rs`).

When a `.glb` references an image whose **content lives in a sibling bundle**
(i.e. the URI resolves to another entry in the entity's content map), prod
does NOT inline a local `Texture2D` in the `.glb` bundle. Instead it emits a
**cross-bundle PPtr** in the material's `m_TexEnvs` slot:

```
m_Texture = { m_FileID = <slot in external list>, m_PathID = <pid in sibling> }
```

Python's `abgen/builder.py` does this via the `resolve_hash` callback
(`external_texture`, line 198). The Rust port (`Builder::external_texture`)
is a direct line-for-line port.

## 2. The on-disk layout — verified by probing 3 prod bundles

I probed three known cross-bundle cases with UnityPy and collected the exact
externals + cross-bundle PPtrs they ship:

```
bafkreidfir2nfh3*_linux (entity bafkreib6utz5rq3bo5...)
 CAB: cab-0caa4ab8b2f1ebdbf792d08d79a79aa7
 externals (count=2):
    [1] 'archive:/CAB-b11b6af2a97300dee9faa763fb3805f6/CAB-…'   ← shader
    [2] 'archive:/CAB-d5034aaac610e8947c12859753ab9a0d/CAB-…'   ← sibling
 XREF Material.m_Shader fid=1 pid=7645288030342540701
 XREF AssetBundle.m_PreloadTable[8]/[10] fid=1 pid=7645288030342540701
 XREF AssetBundle.m_PreloadTable[11] fid=2 pid=-4134667646137189461
 TextAsset.m_Script.dependencies = ["bafkreibxefote3j…_linux"]
 AssetBundle.m_Dependencies = ['cab-b11b6af2…', 'cab-d5034aaac…']

bafkreifwecpxcwv*_linux (entity bafkreihh2mgaqi…)
 externals[1]=shader, externals[2]='archive:/CAB-22633331…/CAB-22633331…'
 m_Dependencies = ['cab-22633331…', 'cab-b11b6af2…'] ← alphabetic order

bafkreihxu6pmg5u*_linux (entity bafkreihh2mgaqi…)
 externals[1]=shader, externals[2]='archive:/CAB-dd71641…/CAB-dd71641…'
 m_Dependencies = ['cab-b11b6af2…', 'cab-dd71641…'] ← alphabetic order
```

### Key invariants

| Field | Value | How derived |
|---|---|---|
| `externals` count | 1 + N (N = number of distinct sibling bundles referenced) | accumulator |
| `externals[0]` | shader CAB at `archive:/{shcab}/{shcab}` | preserved from template |
| `externals[1..]` | sibling at `archive:/{cab}/{cab}` (cab = `cabname::cab_name({hash}_linux)`) | per first encounter |
| Each external's `guid` | `[0u8; 16]` | zeros, NOT the asset guid |
| Each external's `type` | `0` | not the FILE_TYPE_META_ASSET discriminator |
| Cross-bundle PPtr `m_FileID` | `1 + i` (1-based slot in `externals`) | 1 for shader, 2..N for siblings |
| Cross-bundle PPtr `m_PathID` | `prefab_packed_path_id(asset_guid(<cid>), 2800000, FILE_TYPE_META_ASSET)` | the texture's pid INSIDE its own bundle |
| `m_Dependencies` (string list) | shader + sibling CABs, **sorted alphabetically** | sort applied AFTER building the list |
| `metadata.json` `dependencies` array | sibling bundle filenames in encounter order (`<cid>_linux`) | from `ext_bundle_files` |

Note: `m_Dependencies` is sorted but `externals` is NOT — shader always at
index 0 (matches Python and prod).

## 3. The Python reference — `abgen/builder.py:198-229`

```python
def external_texture(self, scene, img_idx) -> tuple[int, int] | None:
    if img_idx is None or img_idx >= len(scene.image_uri):
        return None
    uri = scene.image_uri[img_idx]
    if not uri or self._resolve_hash is None:
        return None
    if img_idx in self.ext_tex_pptr:
        return self.ext_tex_pptr[img_idx]
    ext_hash = self._resolve_hash(uri)
    if not ext_hash:
        return None
    bundle_file = naming.canonical_filename(ext_hash, ".png", self._target, None)
    file_id = self._ext_bundle_fileid.get(bundle_file)
    if file_id is None:
        file_id = 2 + len(self.ext_bundle_files)         # 1 = shader, 2..N = siblings
        self._ext_bundle_fileid[bundle_file] = file_id
        self.ext_bundle_files.append(bundle_file)
    tex_guid = pathids.asset_guid(ext_hash)
    tex_pid = pathids.prefab_packed_path_id(
        tex_guid, TEXTURE_LOCAL_ID, pathids.FILE_TYPE_META_ASSET)
    pptr = (file_id, tex_pid)
    self.ext_tex_pptr[img_idx] = pptr
    return pptr
```

`material` (`abgen/builder.py:291-322`) tries `external_texture` first,
fall back to local `texture`. The resulting external PPtrs are spliced
into the material's preload run in `_fill_assetbundle` (line 691) AFTER
shader + local textures, then the sibling CABs are appended to
`m_Dependencies` (line 708) and to the metadata TextAsset's `dependencies`
JSON array (line 459-466). On `_commit` (line 757-768) the SerializedFile
externals list is extended with one `FileIdentifier` per sibling CAB.

**Python sort gap:** Python does NOT sort `m_Dependencies`. The Rust port
sorts (matches prod). For the two prod cases where shader's CAB sorts first
anyway this is invisible; for the third (`bafkreifwecpxcwv`) Python would
report `[shader, sib]` while prod has `[sib, shader]`. The Rust port matches
prod.

## 4. The Rust implementation

### 4a. `BuildOpts.resolve_hash` (`src/builder.rs`)

New optional callback: `Option<&'a dyn Fn(&str) -> Option<String>>`. Mirrors
`bin/ab-generate`'s `_resolve_hash`. Returning `Some(hash)` triggers
cross-bundle emission. `None` falls back to the local-texture path (existing
behaviour). Parity gate (`tests/parity_bytes.rs`) leaves this as `None`.

### 4b. `Builder::external_texture` (`src/builder.rs`)

Direct port of the Python method. Computes `file_id = 2 + len(ext_bundle_files)`
on first encounter, allocates the cross-bundle `(file_id, path_id)` PPtr,
caches `img_idx -> pptr` to dedup.

### 4c. `Builder::material` change

Tries `self.external_texture(scene, img_idx)` first. If `Some`, writes the
cross-bundle PPtr to the material's `tex_pid` slot AND records the PPtr in
`mat_external_pptrs[mat_pid]` for later splicing into the preload run.
Falls through to `self.texture(scene, img_idx)` for local images.

### 4d. `materials::set_tex` / `build_material_tree` signature change

`tex_pid: HashMap<String, i64>` becomes `HashMap<String, (i64, i64)>`. Local
pids are stored as `(0, pid)`, externals as `(file_id, path_id)`. The
truthy test for "has texture" (`if tex_pid[slot]`) becomes
`f != 0 || v != 0` so the bool semantics carry over.

### 4e. `Builder::fill_assetbundle` changes

- **Material preload run:** the existing `[shader] + tex_pids` chain is
 extended with `mat_external_pptrs.get(mat_pid)` entries (parity with
 `_fill_assetbundle` line 688-691). `sbp_order::order_run` partitions by
 `file_id` so externals naturally sort to the END of the run.
- **`m_Dependencies`:** shader CAB + one entry per `ext_bundle_files`, **then
 sorted alphabetically** (prod-derived rule, see §2).

### 4f. `Builder::build` change — metadata TextAsset

The `dependencies` JSON array, previously hardcoded to `[]`, is built from
`ext_bundle_files`. Empty list when no externals (preserves the existing
parity-gate output for bundles without cross-bundle deps).

### 4g. `commit_objects` / `ExternalsPolicy::ShaderRef`

`ExternalsPolicy::ShaderRef` now carries `ext_bundle_files: &[String]`. The
SerializedFile externals list is rewritten as `[shader, sib_1, sib_2,...]`
where each sibling is built from a clone of the shader external with
`guid = [0u8; 16]`, `type = 0`, `path = archive:/{cab}/{cab}`.

### 4h. `finalize_pathids` change — remap

`mat_external_pptrs` keys (the material pid) are remapped under `old2new`
like every other internal pid. The values stay verbatim because they're
already in a foreign-bundle namespace (`file_id != 0`).

### 4i. CLI wiring

- `ab-generate.rs`: builds `resolve_hash_fn` from `content_by_file`,
 matching `bin/ab-generate`'s `_resolve_hash`. Always on for the
 full-scene driver.
- `ab-build-local.rs`: optional `--content-map JSON --source-file PATH`
 flags. The JSON is the entity's content array
 (`[{"file":..., "hash":...},...]`). When both are given, cross-bundle
 resolution is enabled. Without them, falls back to no-resolver
 (back-compat with existing test scripts).

### 4j. `dev/measure_full_vs_prod_x.py` (new)

Variant of `measure_full_vs_prod.py` that fetches each entity's content
manifest from a catalyst (defaults to `peer.decentraland.org/content`, also
honors `ABGEN_CATALYST`), caches to `/tmp/abgen_entity_cache/`, and threads
`--content-map` + `--source-file` to `ab-build-local`. The original
measurement script is left intact.

## 5. Verification

### 5a. Per-bundle bit-exact for `bafkreidfir2nfh3`

```
==== OURS ==== ==== PROD ====
CAB: cab-0caa4ab8… CAB: cab-0caa4ab8…
externals (count=2): externals (count=2):
 [1] 'archive:/CAB-b11b6af2…/CAB-b11b6af2…' [1] 'archive:/CAB-b11b6af2…/CAB-b11b6af2…'
 [2] 'archive:/CAB-d5034aaac…/CAB-d5034aaac…' [2] 'archive:/CAB-d5034aaac…/CAB-d5034aaac…'
TextAsset.m_Script.dependencies TextAsset.m_Script.dependencies
 = ["bafkreibxefote3j…_linux"] = ["bafkreibxefote3j…_linux"]
Material._BaseMap.m_Texture = (fid=2 pid=…) Material._BaseMap.m_Texture = (fid=2 pid=…)
m_Dependencies = ['cab-b11b6af2…', m_Dependencies = ['cab-b11b6af2…',
                  'cab-d5034aaac…']                              'cab-d5034aaac…']
PreloadTable len=12 PreloadTable len=12
 preload[8] fid=1 pid=7645288030342540701 preload[8] fid=1 pid=7645288030342540701
 preload[10] fid=1 pid=7645288030342540701 preload[10] fid=1 pid=7645288030342540701
 preload[11] fid=2 pid=-4134667646137189461 preload[11] fid=2 pid=-4134667646137189461
```

All cross-bundle fields byte-identical. The 21-byte size delta remaining is
in Mesh objects (covered by separate fixes — vertex_bytes.md, mesh.md, bones_aabb_morph.md).

### 5b. Corpus impact

Full corpus measurement, before vs after the fix:

| metric | before | after | Δ |
|---|---:|---:|---:|
| paired-object byte-exact | 14684/14997 (20871 ppm differ) | 14689/14997 (**20538 ppm differ**) | +5 / −333 ppm |
| residual AssetBundle | 74 | 73 | −1 |
| residual Material | 4 | **2** | −2 |
| residual TextAsset | 3 | **1** | −2 |
| residual Texture2D | 113 | 113 | 0 |
| residual Mesh | 100 | 100 | 0 |
| residual MeshFilter | 11 | 11 | 0 |
| residual GameObject | 8 | 8 | 0 |
| **MeshRenderer** (regression watch) | 0 | **0** | 0 ✅ |

### 5c. Why not all 3 cross-bundle cases closed

`assetbundle.md §2b` identified 3 real cross-bundle prod cases. The fix
closes 1 of 3 (matches python's behaviour). The other 2 are blocked by
unrelated issues:

- **`bafkreifwecpxcwv*`** — uses the cross-bundle PPtrs correctly, but the
 bundle is also short 2 preload entries because of the
 default-material/MF/MR bug (assetbundle.md §5b — separate fix). When that
 fix lands, this bundle should fall to bit-identical for the AssetBundle.
- **`bafkreihxu6pmg5u*`** — source file is `models/AutoPad.gltf` but the
 glb-or-gltf detector mis-routes the request to the StandaloneTexture
 builder (it's JSON content, not glb bytes, but the.gltf extension should
 win). That path doesn't know about `resolve_hash` so emits no externals.
 Fix is in `is_glb_or_gltf` (assetbundle.md §6 item 2).

## 6. Tests

- `cargo test --release --lib` — all 104 tests pass.
- `cargo test --release --test parity_bytes` — Python parity gate passes
 (resolve_hash defaults to None, so behaviour is bit-identical to the
 pre-fix Rust on the parity corpus).

## 7. Files touched

- `src/builder.rs` — `BuildOpts.resolve_hash`, `external_texture`,
 `mat_external_pptrs`, `ext_bundle_files` / `_fileid`, fill_assetbundle
 splice + sorted `m_Dependencies`, metadata `dependencies` JSON,
 `commit_objects` externals expansion, `finalize_pathids` remap.
- `src/materials.rs` — `tex_pid` HashMap value becomes `(i64, i64)`.
- `src/bin/ab-build-local.rs` — `--content-map` + `--source-file` flags.
- `src/bin/ab-generate.rs` — wires `resolve_hash` callback.
- `dev/measure_full_vs_prod_x.py` — new measurement script that supplies
 the per-entity content manifest.
- `dev/fix_proposals/cross_bundle_externals.md` — this file.
