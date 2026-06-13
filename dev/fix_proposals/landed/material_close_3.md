# Material residuals — close-out report

Targets the Material objects that survived the per-sampler-Texture and
material-index-by-glTF-array-index passes. Closes the only Material residual
that wasn't already mechanically closeable through the existing cross-bundle
resolver, and confirms the resolver-aware corpus metric now reports zero
Material residuals.

The headlines (per `dev/measure_full_vs_prod.py` and its `_x` resolver-aware
sibling, isolated binaries to rule out cross-worktree binary races):

| Metric                       | Before | After | Delta |
|------------------------------|------:|------:|------:|
| `measure_full_vs_prod`   Material residuals | 3 | 2 | -1 (KHR closed) |
| `measure_full_vs_prod_x` Material residuals | 1 | 0 | -1 (KHR closed; zero remain) |

The remaining 2 in the no-resolver path are the cross-bundle PPtr cases
already closeable from inside `ab-build-local` once a `--content-map` is
supplied — the protocol and binary flags landed in commit `6700ccc`;
they vanish in the resolver path. The only metric-side
gap was `measure_full_vs_prod.py` ignoring `ABGEN_BIN`, which this patch
also fixes so the binary picked up matches the one the caller actually
built.

## Residual signatures going in (from `dev/fix_proposals/bitwise_residuals.md`)

| Cases | Field path | Example |
|------:|---|---|
| 2 | `m_TexEnvs[*][*].m_Texture.{m_FileID, m_PathID}` | `bafkreidfir2nfh3` / `bafkreifwecpxcwv` material_0 |
| 1 | `m_TexEnvs[*][*].m_Texture.m_PathID` only        | `bafybeiczim5cqrv` material |
| 1 | `m_TexEnvs[*][*].{m_Offset.y, m_Scale.y}` + `m_InvalidKeywords` | `bafybeif66iyagwk` material_0 |

Buckets:
- 3 of 4 are **cross-bundle PPtr** — every glTF image whose URI resolves to a
 sibling content hash (a standalone `.png` content entity) should yield a
 `(file_id, path_id)` pointing into an externals slot, not the local
 `(0, local_pid)`. Already implemented (commit `6700ccc`); needs the
 resolver flag to be wired in the measurement.
- 1 of 4 is **`KHR_texture_transform`** — per-textureInfo `(scale, offset)` that
 Unity stores as `(m_Scale, m_Offset)` on the `m_TexEnvs` entry, plus
 `_TEXTURE_TRANSFORM` in `m_InvalidKeywords`. Never read by `abgen-rs` until
 this patch.

The 4th 'case' in `bitwise_residuals.md`'s count is `bafybeiczim5cqrv` —
a separate cross-bundle case that turned out to already match in the current
binary (verified per-bundle: `ALL MATCH (98 materials)` in
`tmp/material_residuals.log`), so the corpus headline before this patch was
**3 Material residuals, not 4**.

## P1 — `KHR_texture_transform` per-slot UV transform

**Witness** (UnityPy of prod
`bafkreihh2mgaqi42kz2…/bafybeif66iyagwk…_linux/material_0`):

```
m_TexEnvs[_BaseMap]:
 m_Scale = (x=1.0, y=1.2000000476837158)
 m_Offset = (x=0.0, y=2.8999998569488525)
m_InvalidKeywords = ['_TEXTURE_TRANSFORM']
```

**Source glTF** (`bafybeif66iyagwkslk356sp7dl5flzihq2scrdzal5osfpck52jhqd4ade`,
`extensionsUsed=['KHR_texture_transform']`):

```
materials[0].pbrMetallicRoughness.baseColorTexture.extensions
.KHR_texture_transform = {offset:[0, -3.0999999046325684], scale:[1, 1.2000000476837158]}
```

**Conversion** (UV is flipped `v -> 1 - v` at parse time, so the equivalent
Unity transform satisfies):

```
m_Scale.x = gltf_scale.x
m_Scale.y = gltf_scale.y
m_Offset.x = gltf_offset.x
m_Offset.y = 1 - gltf_offset.y - gltf_scale.y (in f32, to match Unity)
```

Numeric check (f32): `1.0_f32 - (-3.0999999_f32) - 1.2000000_f32 = 2.8999999`
— byte-identical to prod.

**Implementation**:

1. **`src/scene.rs`** — add `TexTransform { scale: [f64;2], offset: [f64;2] }`
 with `identity` / `is_identity`. Add `tex_transforms:
 BTreeMap<String, TexTransform>` field on `Material` (BTree because the map
 is iterated for `_TEXTURE_TRANSFORM` detection — kept Ord for build
 determinism). Missing entry = identity (default `(1,1)/(0,0)`).

2. **`src/gltf.rs`** — `tex_transform(tex_info)` reads
 `extensions.KHR_texture_transform.{offset, scale}` and applies the
 Unity-space conversion at parse time. Identity transforms are skipped
 (`None`) so they don't trip the `_TEXTURE_TRANSFORM` keyword. Plumbed in
 the materials loop for `_BaseMap`/`_BumpMap`/`_MetallicGlossMap`/
 `_OcclusionMap`/`_EmissionMap`.

3. **`src/materials.rs`** — `build_material_tree` reads `m.tex_transforms` and
 passes the per-slot `(scale, offset)` to `set_tex`. `material_keywords`
 takes a new `has_tex_transform` bool that appends `_TEXTURE_TRANSFORM` to
 `m_InvalidKeywords` (preserving the alphabetic sort).

**Out of scope** (would surface as a fresh per-slot residual if any corpus
material exercises them): `rotation`, per-textureInfo `texCoord` overrides.
No corpus material in `$ABGEN_PROD_ROOT` triggers them today (sweep
confirmed: only `bafybeif66iyagwk` uses `KHR_texture_transform`, and only on
`baseColorTexture` with `(scale.y, offset.y)`).

**Verification** (direct `ab-build-local` + UnityPy compare on the only
affected bundle):

```
== KHR_texture_transform: bafybeif66iyagwkslk3.. ==
 ALL MATCH (4 materials)
```

## P2 — Cross-bundle PPtr resolver (already plumbed; measurement gap closed)

`ab-build-local --content-map JSON --source-file PATH` is the supported entry
point (commit `6700ccc`). The full-pipeline wiring already exists. The only
gap was `dev/measure_full_vs_prod.py` ignoring `ABGEN_BIN` and the
`--content-map`/`--source-file` flags — `measure_full_vs_prod_x.py` already
exists for the resolver-aware sweep.

This patch makes `measure_full_vs_prod.py` honor `ABGEN_BIN` (matches the
`_x` variant), so the binary the metric is computed against is the one the
caller actually built. Without this, running the measurement from any
worktree silently fell back to the binary in the main repo (which a
parallel investigation may have just rebuilt), surfacing as ghost
Material residuals that vanished as soon as the right binary ran.

## Corpus impact

Measured against `$ABGEN_PROD_ROOT` (280 production bundles), built from
`/tmp/ab-build-local.before` (= tip `6989237` without this patch) and
`/tmp/ab-build-local.after` (= same source plus this patch). Same input set,
same script, same Python sibling, only the binary changed.

**`measure_full_vs_prod.py` (no `--content-map` resolver)** — closes the KHR
case; cross-bundle cases remain because the script doesn't supply a content
map:

```
                                  before          after            delta
Material residuals 3 2 -1 (KHR closed)
AssetBundle residuals 68 68 —
Texture2D residuals 60 60 —
Mesh residuals 21 21 —
MeshFilter residuals 11 11 —
TextAsset residuals 3 3 —
paired-object byte-exact 14841/15007 14842/15007 +1 (the closed Material)
                                  (98.89%)        (98.90%)        +0.01 pp
size-delta rust smaller 202, larger 78, mean |Δ| = 5833 bytes (unchanged)
```

**`measure_full_vs_prod_x.py` (cross-bundle resolver wired)** — closes the KHR
case; cross-bundle PPtr cases were already passing here because the resolver
path was implemented in commit `6700ccc`:

```
                                  before          after            delta
Material residuals 1 0 -1 (the last one)
AssetBundle residuals 67 67 —
Texture2D residuals 60 60 —
Mesh residuals 21 21 —
MeshFilter residuals 11 11 —
TextAsset residuals 1 1 —
paired-object byte-exact 14846/15007 14847/15007 +1 (the closed Material)
                                  (98.93%)        (98.93%)        +0.01 pp
```

The corpus headline going in was therefore **3** Material residuals in the
no-resolver path and **1** in the resolver path — not 4 as `bitwise_residuals.md`
states. The `bitwise_residuals.md` count is stale (predates one of the
cross-bundle materials already matching).

**`tests/parity_bytes.rs`** (the python-vs-rust regression gate) — unchanged
threshold; the python sibling doesn't read `KHR_texture_transform` either, so
the gate keeps measuring exactly what it measured before.

## Determinism

The `ab-build-local` binary is deterministic: same source → same binary md5,
same input → byte-identical output across runs. The variance observed in
earlier numbers (Material counts moving by 1, Mesh by 4 between runs) traced
back to **`dev/measure_full_vs_prod.py` hardcoding `AB_BIN` to
`abgen-rs/target/release/ab-build-local`
and ignoring the `ABGEN_BIN` env var**. With concurrent worktrees racing
to `cargo build` into the main repo, that single path holds whichever
build won the race — so two consecutive `measure_full_vs_prod`
runs may compare different snapshots and produce different residuals
without any source-level change.

Fixed by making `AB_BIN` resolve to a path relative to the script's location
(`os.path.dirname(__file__)/../target/release/ab-build-local`) with
`ABGEN_BIN` override — mirrors `measure_full_vs_prod_x.py`. Three
consecutive runs against the same binary now produce identical
`obj-exact=14848/15007` (`AssetBundle:68, Texture2D:60, Mesh:14,
MeshFilter:11, TextAsset:3, Material:3`) with the stale upstream binary
that's been getting overwritten — and identical `14842/15007`
(`AssetBundle:68, Texture2D:60, Mesh:21, MeshFilter:11, TextAsset:3,
Material:2`) with my isolated binary. The build itself is bit-stable.

## Open problems after this patch

- `Material: 2` (no-resolver) is the floor reachable from `ab-build-local`
 without `--content-map`; closing it requires either wiring the resolver into
 `measure_full_vs_prod.py` too (trivial, but duplicates `_x`), or treating
 `measure_full_vs_prod_x.py` as the canonical metric.
- `Material: 0` (with resolver) is the new corpus floor for this object type.
 No known Material residual remains in the resolver-aware metric.
- Other residual types untouched by this patch (AssetBundle 67–68, Texture2D
 60, Mesh 14–21, MeshFilter 11, TextAsset 1–3) own their own close-out work
 in sibling fix proposals.
