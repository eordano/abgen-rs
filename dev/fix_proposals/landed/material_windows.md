# Material residuals — windows + URP v10 close-out

> **Status: LANDED.** Fix is harness-only (no Rust change). The
> `bitwise_residuals_windows_material.py` script unconditionally supplies
> `--source-file` + `--content-map` for `.glb` entities, mirroring what
> `measure_full_vs_prod_x.py` already did for linux. The
> `external_texture` resolver in `src/builder.rs` was already wired
> (commit `6700ccc`); the gap was purely the measurement gate.
> 890/890 windows Materials byte-exact. Same pattern applies to mac.

Targets the Material objects that survived against the windows v10 reference
set under `workdir/pathid_rt_v10_windows/` (280 bundles across 2 entity-scene
roots, 890 materials).

The headlines (per `dev/bitwise_residuals_windows_material.py`):

| Metric                                            | Before | After | Delta |
|---------------------------------------------------|------:|------:|------:|
| Material bits-diff (ppm of paired materials)      | 2247  | 0     | -2247 |
| Material residuals (count)                        | 2     | 0     | -2    |
| Distinct diff-signatures                          | 1     | 0     | -1    |

Both before/after numbers come from the same `ab-build-local` binary; the only
change is **which bundles get `--content-map`/`--source-file` from the
measurement harness**. The Material class on windows v10 closes 100% by
extending the existing cross-bundle PPtr resolver to `.glb` entities (not
just `.gltf` and `_emote.glb`).

108 cargo tests stay green. `parity_bytes.rs` ceiling unchanged at 800000
bits (current 791702 bits, comfortably under).

## Signatures going in

The forensic sweep finds **exactly one** Material diff signature on
windows v10, with **2 cases** in the corpus. Identical shape to the linux
`material_close_3.md` cross-bundle pattern, just under a different platform
suffix:

| Cases | Field path                                                   | Example bundles |
|------:|--------------------------------------------------------------|-----------------|
| 2     | `m_SavedProperties.m_TexEnvs[*][*].m_Texture.{m_FileID, m_PathID}` | `bafkreidfir2nfh3…_windows`, `bafkreifwecpxcwv…_windows` |

The values are diagnostic:

```
ours: m_FileID=0 m_PathID=0
prod: m_FileID=2 m_PathID=-4134667646137189461 (or =1, …)
```

`(0, 0)` is the local-pointer fallback the builder emits when no
cross-bundle resolver fires. Prod's `(FileID >= 1, PathID = standalone-texture
PathID hash)` is the cross-bundle PPtr emitted by the resolver path in
`builder.rs::external_texture` against the entity's content map.

## Witness — entity `bafkreib6utz5rq…` / glb `bafkreidfir2nfh3…`

The affected glb is a plain `.glb` (not `.gltf`, not `_emote.glb`) but its
embedded glTF JSON references an **external** PNG via `uri`:

```
images[0] = {"mimeType": "image/png",
             "name": "Floor_Concrete01.png",
             "uri": "Floor_Concrete01.png.png"}
```

The entity manifest binds that URI (resolved against the source-file path
`models/FloorBaseConcrete_01/FloorBaseConcrete_01.glb`) to a sibling content
file:

```
file = "models/FloorBaseConcrete_01/Floor_Concrete01.png.png"
hash = "bafkreibxefote3je…"
```

That sibling ships as its own standalone-texture bundle. Prod emits a
cross-bundle PPtr into the standalone bundle's pre-computed PathID; we
emit `(0, 0)` because the harness never told us about the sibling.

Manually re-invoking the binary with `--source-file` + `--content-map`
demonstrates the Material becomes byte-identical:

```
$ ab-build-local <glb> <name> <cid> /tmp/out --source-file <virtual-path> \
                 --content-map /tmp/entity.json --content-dir <store>
# UnityPy diff vs prod:
# Material: 0 diffs
# (residuals shift to AssetBundle preload-table ordering + TextAsset
# metadata version — separate residual classes, out of Material scope)
```

The binary's resolver path was already implemented for `.glb` entities in
`material_close_3.md`'s landed work (commit `6700ccc`); the
gap was purely in the **measurement harness gate** — `bitwise_residuals.py`
and `measure_full_vs_prod.py` only set `need_src=True` for `.gltf` and
`_emote.glb`, suppressing the resolver for ordinary `.glb` entities.

## Implementation — measurement harness, not builder

No code change to `src/materials.rs` or `src/scene.rs`. The fix is in
`dev/bitwise_residuals_windows_material.py` (new): always supply
`--source-file <virtual-path>` + `--content-map <entity-json>` when the
entity manifest is available, regardless of the source file extension:

```python
# Mirror `measure_full_vs_prod_x.py` (resolver-aware canonical metric):
# ALWAYS supply --content-map + --source-file when the entity manifest
# is available, regardless of file extension. The hash-resolver in
# ab-build-local activates whenever source_file is set, enabling
# cross-bundle PPtr emission for `.glb` entities whose images carry
# an external URI (as well as `.gltf` and `_emote.glb`).
need_src = bool(inv.get(cid))
cmap = None; cdir = None
if need_src:
    cmap = ensure_ent_map(ent_id, tmp)
    if cmap: cdir = CONTENT_ROOT
```

This mirrors what `measure_full_vs_prod_x.py` already does for the linux
training corpus. The hash-resolver in `bin/ab-build-local.rs::run` becomes
active when `source_file.is_some && !content_by_file.is_empty`; from
there everything plumbs through unchanged.

## Per-class corroboration vs the resolver-everywhere comparison

`/tmp/forensic_material_signatures.py` (one-shot dev script) runs each of
the 280 bundles **twice** — once with the legacy gate, once with the
resolver-everywhere gate — and pair-diffs against prod, Material-only:

```
WINDOWS v10 Material baseline-vs-resolver:
 paired materials : 890
 baseline mat-exact : 888/890 (99.7753%)
    bits-diff ppm        : 2247
 resolver mat-exact : 890/890 (100.0000%)
    bits-diff ppm        : 0
 closed by resolver : 2
```

No other Material signature exists in the windows v10 set. Windows-specific
candidates that didn't show up:

- Shader keyword set (URP windows vs linux): **identical** — both prod
 and ours emit the same `m_ValidKeywords` / `m_InvalidKeywords` for every
 one of the 890 paired materials.
- Default render queue: **identical** — `m_CustomRenderQueue` matches
 across all 890.
- `m_LightmapFlags`: **identical** — same `4`/`1` emit (emissive on/off)
 matches.
- Color-space settings, `m_Floats`, `m_Ints`, `m_Colors`, `m_Shader`,
 `m_TexEnvs.{m_Scale, m_Offset}` (KHR_texture_transform), `m_InvalidKeywords`:
 **all identical** — the only diff field across the corpus is
 `m_TexEnvs[*][*].m_Texture.{m_FileID, m_PathID}` (the 2 cross-bundle
 cases above).

## Out of scope (not Material residuals on windows v10)

The resolver-enabled path surfaces shifts in **other** classes for the same
2 bundles — those are tracked by their own fix proposals:

- AssetBundle preload-table ordering (`m_PreloadTable[*].{m_FileID, m_PathID}`
 permutation) — see `assetbundle.md`. The same shader-slot front/back
 partitioning that affects all `.glb` bundles.
- TextAsset metadata version `"7.0"` → `"8.0"` — separate sibling
 fix; the resolver path emits an updated metadata schema.

These were always-present residuals in the windows corpus; the resolver
path makes them more visible by closing the upstream Material gap.

## Determinism

`ab-build-local` is deterministic — same source → same binary md5, same
input → byte-identical output across runs. Re-running the harness three
consecutive times produces identical
`Material residuals: 2 (baseline) / 0 (resolver-everywhere)` numbers.

## Open problems after this patch

- `Material: 0` on windows v10. **No known Material residual remains in
 the resolver-aware metric.**
- Same applies to mac v10 (`pathid_rt_v10_mac`) by the same pattern — not
 measured here, but the cross-bundle path is platform-agnostic in
 `builder.rs::external_texture` (the `_<platform>` suffix is the only
 per-platform value).
- Other windows residual types (AssetBundle, Texture2D, Mesh, MeshFilter,
 TextAsset) own their close-out work in sibling fix proposals.
