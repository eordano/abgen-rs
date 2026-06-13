# Collection-URN bundle-count gap (547 vs 568) — negative finding

**Status**: drilled; the stated hypothesis is disproven. The gap is a
**content-version drift between the reference and the live lamb2 collection
content**, not a missing embedded-texture emit rule. No code change.

## What the brief claimed

> The extra bundles are largely glb-EMBEDDED textures (image uri's inside a
> glb) that the lambdas collections/wearables DTO does not list as top-level
> content; parse each glb's images[].uri, resolve against the wearable
> content, emit a standalone-texture bundle per resolved image.

## What the bytes actually show

Built ours with `--collection-urn urn:decentraland:off-chain:base-avatars`
(lamb2 on :5142) → **547 bundles, 0 errs**. Reference
`ad0564d-windows-urn-baseavatars` has **567 flat files + 1 `dcl/scene_ignore_windows`
= 568**.

Set diff (flat files only):

```
ours 547   ref 567   matched 443   missing(ref-only) 124   extra(ours-only) 104
```

The churn is entirely in `bafk` (raw-CID) bundles; **all 51 `bafy` bundles match**.
By magic-byte sniff: missing = 102 glb + 22 png; extra = 102 glb + 2 png.

### The embedded-texture hypothesis is empirically false here

Parsed every glb's `images[].uri`, resolved each against its wearable's
content map (sha1-prefix store path, exact CID only — no scanning):

```
embedded image refs resolved to a content hash: 163 distinct
embedded refs that were data-uri / bufferView (no file): 0
of the 124 MISSING bundles that are embedded-resolved textures: 0
of the 104 EXTRA bundles that are embedded-resolved textures: 2
```

Every embedded image reference inside every glb already appears as a
top-level content item. There is no "embedded texture the DTO omits" class in
this collection. Total top-level content = 309 glb + 238 img = **547** = exactly
our emit count. We emit precisely the DTO content set, deduped.

### Root cause: reference content is stale vs current lamb2

```
124 MISSING bundles in current collection content: 0 / 124   (all stale)
124 MISSING bundles present in content store now:  124 / 124  (bytes exist)
124 MISSING bundles present in the reference unity.log: 124 / 124
443 matched bundles == the 443 ref bundles that ARE current collection content
104 EXTRA bundles ARE all current collection content (e.g. Hair_DoubleBun.glb,
    EmeraldRing.glb) but absent from the reference
```

Per-wearable: **79 of 282 wearables drifted** (≥1 content hash not in the
reference); 203 fully match. The reference `unity.log`
(`ad0564d-windows-urn-baseavatars.unity.log`) shows the converter resolved the
collection from **production** `https://peer.decentraland.org/lambdas/collections/wearables?collectionId=urn:decentraland:off-chain:base-avatars`
on 2026-05-31. Between then and now, ~79 base-avatar wearables were redeployed,
so production/lamb2 now returns newer content hashes. All 567 reference hashes
appear in the unity.log; the bytes are still in the local store, but they are no
longer the collection's current content.

## Why no fix lands here

- It is **not a converter-logic gap**. Our emit set == the DTO content set,
  exactly. There is no missing emission rule (only glb/png content types exist;
  thumbnails are not in the content list; no jpg/other).
- The only way to make counts match is to feed the converter the **same content
  the reference used** — i.e. pin lamb2 (or the corpus driver) to the
  May-31 production snapshot of this collection. That is a corpus/data
  provisioning task, not an abgen-rs change, and reconstructing it from the
  per-CID hash list in the unity.log would be a hardcoded per-CID table
  (explicitly forbidden).
- `abgen-verify` already intersects to the 443 matched names, so the drift does
  **not** pollute byte-parity numbers — it only inflates the raw count gap.

## Baseline numbers (443 matched bundles; unchanged — no code edit)

```
kind                 bundles  byte-id  smaller  larger   pair-bits    diff-bits   ppm
glb-wearable             207       10       78      71    76479024     12650333  165409.2
standalone-texture       236        2      176      53   207132488    103203427  498248.4
TOTAL                    443       12      254     124   283611512    115853760  408494.6
```

Parity gate (`cargo test --release --test parity_bytes`): **green** (2/2).

## Recommendation

Close RESEARCH_AREAS #1's embedded-texture framing for base-avatars. The real
follow-ups, if a count match is wanted, are operational:
1. Regenerate the URN reference against current lamb2 content (then counts match
   by construction), or
2. Snapshot the production `collections/wearables` DTO at reference-gen time and
   replay it through the corpus driver (`from_collection_urn` accepts a lambdas
   URL; point it at a pinned snapshot server).

Neither is an abgen-rs source change. The byte-parity work that IS tractable on
this collection lives in the 443 matched bundles (standalone-texture 498K ppm,
glb-wearable 165K ppm) — i.e. the BC7 / mesh classes already tracked in
PARITY_STATUS.md, not the bundle SET.
