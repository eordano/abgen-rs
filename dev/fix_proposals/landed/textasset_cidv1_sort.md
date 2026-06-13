# TextAsset CIDv1 — `dependencies` array alphabetical sort

> **Status: landed (this commit).** Closes the last CIDv1 TextAsset
> `metadata.json` residual on the 22-entity windows v10 corpus.

## Background

After commit `9d33fdc` closed the legacy CIDv0 (`Qm…`) case by gating
metadata-TextAsset emit entirely, the remaining `TextAsset` residual on
`workdir/pathid_rt_v10_windows/` (CIDv1 entities only) measured
**25,942 ppm-bits** (33,266 bits over 160,288 prod-bytes of TextAsset
across 19 entities, ~2,160 bundles). All of the differing bundles had
the right `version` (`"8.0"`) and the right *set* of `dependencies`
strings — only the *order* was wrong.

## Root cause

`Builder::fill_textures` accepted `metadata_dependencies` from
`BuildOpts` in caller-supplied order, unioned `ext_bundle_files`
collected during build, and serialized **without reordering**
(per the prior contract documented in
`dev/fix_proposals/landed/textasset_close_3.md`).

That contract held on the 280-bundle `_linux` URP-v7 test corpus
because glTF `images[]` iteration order happened to coincide with
alphabetical order on those bundles.

It breaks on the windows URP-v10 corpus: prod orders the
`dependencies` array **lexicographically** (Unity's
`AssetBundleManifest.GetAllDependencies` returns sorted-set
semantics). Verified empirically across the corpus's three offender
bundle families:

- `bafkreia67htob…` / `models/tastatur/{6,8,a,...}.gltf` — 40 bundles
 whose 4 deps differ only by ordering of two `bafkrei…` and two
 `bafybei…` deps. Prod = `[kr_aog, kr_dar, yb_b4f, yb_d4g]`, ours
 (insertion order) = `[kr_dar, yb_d4g, kr_aog, yb_b4f]`. Sorting
 ours alphabetically yields prod.
- `bafkreihy2pq…` / `models/outerlayers/level_3_v2.gltf` — 8 deps,
 same pattern; sorted prod matches when ours is sorted.
- `bafkreibwoex…` / `models/doge_1/scene.gltf` — 2 deps; sorted match.

Total across the corpus: every TextAsset divergence resolves to
"deps set is identical, deps order is wrong" — no missing entries,
no extra entries, no format / version / mainAsset divergence.

## Fix

`src/builder.rs::fill_textures` metadata-TextAsset emit (the glb path,
~line 1268): add a single `deps.sort_unstable` after the dedup
union, before the JSON formatter runs.

The standalone-texture builder is untouched — it always emits
`dependencies:[]` and was already correct.

`BuildOpts.metadata_dependencies` rustdoc updated: the builder
**does** reorder now (sorted-set semantics matching Unity's
`AssetBundleManifest.GetAllDependencies`). Caller order doesn't
matter. The `textasset_close_3` contract ("builder does not
re-order") is overridden — sort is a strict refinement, idempotent
on already-sorted inputs, so the 280-bundle linux corpus stays
byte-stable (its insertion order was already sorted).

## Measurement (CIDv1 entities, prod-bytes denominator)

Before:
```
TextAsset bits= 33,266 prod-bytes=160,288 ppm=25,942.4
```

After:
```
TextAsset bits= 0 prod-bytes=160,288 ppm= 0.0
```

Per-entity forensic on the three offender entities:

| Entity (CIDv1)                       | bundles | TextAsset diffs before | after |
|---|---:|---:|---:|
| `bafkreia67htob…` (tastatur scene)   | 215     | 40                    | 0     |
| `bafkreihy2pqlk4…` (overlay scene)   | 557     | 19                    | 0     |
| `bafkreibwoex6cy…` (asset-packs)     | 215     |  1                    | 0     |

All other diff signatures (`miss=…,extra=…`, version-string,
mainAsset, etc.) — **zero** instances across all 19 CIDv1 entities.

## Test bars

- `cargo test --release --lib` → 116 passed.
- `cargo test --release --test parity_bytes` → 2 passed at 773,032
 bits-different ceiling (unchanged — parity fixtures use empty
 `metadata_dependencies` so the sort is a no-op there).
- All other per-class ppm-bits unchanged (Mesh, Texture2D,
 AssetBundle, Material, MeshFilter, MeshRenderer, …) — sort only
 affects `TextAsset` object bytes.

## No-regression for callers

`BuildOpts.metadata_dependencies = &[]` paths are byte-identical
to before (empty Vec sorts to itself in zero ops). Callers that
pre-sorted are byte-identical. Callers that passed iteration-order
deps that happened to coincide with sort order (e.g. the 280-bundle
`_linux` corpus) are byte-identical. Only the new-corpus CIDv1
bundles where iteration order diverged from alphabetical see a
change — and that change moves them from divergent to byte-exact
with prod.
