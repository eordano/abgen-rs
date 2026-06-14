# Corpus manifests — stratified entity samples for parity validation

Two JSON files, each a list of Decentraland catalyst entity IDs to feed to
Decentraland's `asset-bundle-converter` for reference-bundle generation:

| File | Count | Purpose |
|---|--:|---|
| `test_entities.json` | 21 | Small significative sample for quick iteration |
| `validation_entities.json` | 302 | Broad anti-overfit sample |

Both are stratified for coverage:

- **scenes** by NTILE bucket of `content_files` count (10 buckets × 20 for validation, 4 buckets × 3 for test). Scene file count spans 2 to 4,977 with p50=4, p90=48, p99=184 — bucketing is necessary to avoid sampling only the small tail.
- **wearables, emotes, profiles** by MD5-of-(entity_id || seed) ordering (the count distribution is tight enough that uniform random is enough).
- **draco-bearing scenes** force-included (rare extension, would be missed by random sampling).

Both samples are disjoint (different `md5` seed suffixes: `val` vs `test`).

## Coverage targets

The selection aims to exercise every AssetBundle kind that
`abgen-verify` classifies:

| Bundle kind | Triggered by |
|---|---|
| `standalone-texture` | `.png` / `.jpg` files in entity content (any kind) |
| `glb-scene`, `glb-scene-collider`, `glb-scene-empty` | scene `.glb`s |
| `glb-wearable`, `glb-with-morph` | wearable `.glb`s |
| `glb-emote` | emote `.glb`s with `_emote.glb` suffix or `entity_type=emote` |
| `glb-animated` | wearable `.glb`s with non-emote animations |
| `draco` | `.glb`s with `KHR_draco_mesh_compression` (force-included) |
| `bundle-empty` | header-only AssetBundles (uncommon residual) |

## Composition

```
test_entities.json: 21 entities
 by_type: scene=13 wearable=4 emote=2 profile=2
 draco: 1 entity
 file_count buckets: 1-3=5 4-10=12 11-50=2 200+=1 (51-200=0)

validation_entities.json: 302 entities
 by_type: scene=202 wearable=50 emote=30 profile=20
 draco: 2 entities
 file_count buckets: 1-3=96 4-10=143 11-50=41 51-200=16 200+=4
```

Profile entities contribute `body.png` + `face256.png` pairs → standalone-texture
bundles, broadening the texture-format mix that scene/wearable contents may
not exercise.

## Regeneration workflow

Point `ABGEN_CONTENT_ROOT` at your catalyst content store
(`<root>/<sha1(cid)[:4]>/<cid>` layout).

To rebuild a corpus from scratch:

1. **Pick entities** — adapt the SQL queries in `_regen.sql`. Stratify by
 file-count bucket for scenes; uniform random for wearables / emotes /
 profiles. Force-include any extension you want guaranteed coverage of
 (e.g. draco).

2. **Run the converter** — feed the entity list to the
 [`abc-deterministic-guids`](https://github.com/decentraland/asset-bundle-converter/tree/abc-deterministic-guids)
 fork of `asset-bundle-converter`. Recommended commit: `fefe44e` or
 later (post-`SetDeterministicSubAssetIds` patch so re-runs are
 byte-equal). Output lands under
 `<corpus-dir>/<entity_id>/<cid>_<platform>`.

3. **Build ours** — `abgen-corpus --from-reference <corpus-dir> <out-dir>`
 reads the reference layout and parallel-builds.

4. **Verify** — `abgen-verify <out-dir> <corpus-dir>` reports per-AB-kind
 ppm + size-delta + byte-identical count.

## When to re-sample

- **Test set**: re-sample if the parity-fixture caps in
 `tests/fixtures/parity/index.json` stop reflecting useful regression
 signal (e.g. all caps go to 0, or one kind dominates).
- **Validation set**: re-sample if the catalyst's entity distribution
 shifts materially (new heavy-asset categories land, new format
 extensions become common).
- Use distinct MD5 seeds (`val`, `test`, plus a new salt) so successive
 samples are disjoint and old-vs-new comparisons are meaningful.
