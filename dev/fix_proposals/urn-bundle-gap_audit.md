# Audit of the collection-URN bundle-gap negative finding

**Verdict: the prior negative finding (`urn_bundle_gap_session.md`,
commit df802f7) is CORRECT.** The 547-vs-568 base-avatars URN gap is
content-version drift between the Jun-1 production reference and current lamb2
collection content — NOT a missing glb-embedded-texture emit rule. Every
number in the prior finding reproduces independently, and two additional
checks (clean-room upstream converter + exhaustive embedded-uri scan) confirm
there is no implementable converter-logic fix. No code change.

## Reproduction (independent)

Built with lamb2 (:5142):
`./target/release/abgen-corpus --collection-urn urn:decentraland:off-chain:base-avatars /tmp/ours-urn --platform windows -j 16`
→ **547 bundles, 0 errs** (`276 wearables` per lamb2 paging; DTO has 282 entries).

Reference `ad0564d-windows-urn-baseavatars`: **567 flat `<hash>_windows` files
+ 1 `dcl/` dir entry = 568** (matches the reference `chain.log`: `bundles=568`).

Set diff (flat files only, via `comm` on sorted `ls`):

```
ours 547   ref 567   matched 443   REF_ONLY 124   OURS_ONLY 104
```

All churn is in `bafk` (raw CIDv1) bundles; all 51 `bafy` bundles match.

## 1. REF_ONLY (missing) — 124, classification

By source-content magic byte (CID resolved through the sha1-prefix store path,
exact CID only — no scanning):

```
REF_ONLY 124 = 102 glb + 22 png   (all 124 source files present in store)
```

These are NOT a distinct "kind". They are **stale content hashes** that were the
collection's content on Jun 1 and have since been superseded:

- **0 / 124** appear in the CURRENT lamb2 DTO content set.
- **124 / 124** appear in the reference `unity.log`
  (`ad0564d-windows-urn-baseavatars.unity.log`), proving they WERE the
  collection content at reference-generation time.
- The unity.log shows the reference resolved the collection from
  **production** `https://peer.decentraland.org/lambdas/collections/wearables?collectionId=urn:decentraland:off-chain:base-avatars`.

## 2. OURS_ONLY (extra) — 104, explanation

```
OURS_ONLY 104 = 102 glb + 2 png   (all 104 source files present in store)
```

These are **newly-deployed content** absent from the Jun-1 reference:

- **104 / 104** are in the CURRENT lamb2 DTO content set (key extensions:
  202 glb-keys + 4 png-keys across representations, deduped to 102 glb + 2 png).
- **0 / 104** appear in the reference `unity.log` — they did not exist when the
  reference was generated.

Not a naming bug, not a DCL_Scene.mat rule, not per-CID vs collection emit
difference. Our emit naming (`<content-hash>_<platform>`) is byte-identical to
the reference's for the 443 matched bundles.

## 3. Embedded-texture hypothesis (RESEARCH_AREAS #1) — EMPIRICALLY FALSE

Hypothesis: missing bundles are images embedded via `images[].uri` inside a glb
that the lambdas DTO does not list as top-level content, so the converter should
emit an extra standalone-texture bundle per resolved embedded image.

Exhaustive scan of every glb in the current collection (parse GLB JSON chunk,
enumerate `images[]`, url-decode + resolve each `uri` against that wearable's
`representations[].contents[]` keys):

```
embedded image refs (file-uri):                937
  data: URIs (inline base64):                    0
  bufferView images (no uri):                    0
  embedded URIs UNRESOLVED against DTO content:  0   <-- key result
```

**All 937 embedded image references resolve to a top-level DTO content key.**
There is no "embedded texture the DTO omits" class in base-avatars. None of the
22 missing PNGs are still in the current DTO (0/22), so they cannot be recovered
by any embedded-uri rule either — they are drift, full stop.

### Clean-room cross-check against the open-source converter

`asset-bundle-converter/.../Wearables/WearablesClient.cs::GetMappingPairs`
builds its content mapping **purely** from
`wearableData.data.representations[].contents[]` (`file = content.key`,
`hash = content.url after last '/'`). It never enumerates glb-embedded
`images[].uri` to create additional bundles. abgen-rs's
`from_collection_urn` (src/bin/abgen-corpus.rs) mirrors this exactly: it emits
one bundle per glb and per image in `content_items`, where `content_items`
come straight from `representations[].contents[]`. There is no emit rule to add.

## 4. Per-wearable drift

```
wearables total:                                   282
drifted (>=1 current hash absent from reference):   79
fully matching:                                     203
```

Examples of drifted wearables: cool_hair (Hair_Cool.glb), double_bun
(Hair_DoubleBun.glb), f_skull_earring, hair_f_oldie, red_bandana. These were
redeployed between Jun 1 and now.

## Conclusion

The prior negative finding holds on every metric. The gap is not tractable as
an abgen-rs source change:

- Our emit set == the DTO content set, exactly (547 = 309 glb + 238 img).
- There is no embedded-texture (or any other) emit rule that produces the 124
  missing bundles; they are stale CIDs no longer in the collection.
- Matching 568 would require feeding the converter the **same content the
  reference used** — pinning lamb2/the corpus driver to the Jun-1 production
  snapshot, or replaying a pinned `collections/wearables` DTO. That is a
  data-provisioning task, not a converter fix; reconstructing it from the
  unity.log CID list would be a forbidden hardcoded per-CID table.

The byte-parity work that IS tractable on this collection lives in the 443
matched bundles (standalone-texture / glb-wearable BC7 + mesh classes), already
tracked in PARITY_STATUS.md — not in the bundle SET.

Parity gate (`cargo test --release --test parity_bytes`): **green (2/2)**.
No code change.
