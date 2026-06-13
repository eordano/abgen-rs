# blindspot_queue — coverage blind-spot oversample

`blindspot_queue.txt` is a deduped list of catalyst entity CIDs (one per line)
curated for a **future** Unity asset-bundle reference run. Unlike
`test30_queue.txt` and `validation300_queue.txt` — which mirror the production
entity distribution and therefore under-sample rare glTF features — this list
**deliberately over-samples the converter's coverage blind spots** so those
code paths can be byte-validated.

It is curated for a tractable single Unity batch (low-hundreds of entities, the
list itself is on the small end of that range). It is a mix of avatar entities
(wearable / emote / outfits) and scene entities, leaning scene-heavy because
authored scene glbs carry most of the rare extensions; the avatar entries make
sure the SkinnedMeshRenderer / blendshape side of the pipeline is exercised too,
not just the scene MeshRenderer / collider side.

## What it over-samples

Each entity was selected because at least one of its `.glb` files exercises a
target feature, detected by parsing the glTF JSON chunk directly from the
content store (read-only). Selection is greedy with a per-feature quota and a
per-pipeline (avatar vs. scene) split, preferring glbs that knock out several
still-needed features at once, so the list stays compact while every feature
clears its quota.

Features covered, with rough relative depth:

- **morph targets / blendshapes** — solid coverage. Mesh primitive `targets`.
- **KHR_draco_mesh_compression** — solid coverage (rare in production; this is
  the headline blind spot).
- **KHR_materials_pbrSpecularGlossiness** — solid coverage. Legacy spec-gloss
  material model.
- **KHR_texture_transform** — solid coverage. UV scale/offset/rotation on a
  texture reference.
- **KHR_materials_transmission** — solid coverage. Transparent/refractive
  materials.
- **multi-material meshes** — well covered. A single mesh whose primitives
  reference two or more distinct materials.
- **multi-clip animations** — solid coverage. Two or more animation clips in
  one glb.
- **high-joint-count skins** — well covered. Skins with large `joints[]` arrays
  (deep avatar rigs).
- **vertex color** — well covered. Primitives with a `COLOR_0` attribute.
- **UV1** — well covered. Primitives with a `TEXCOORD_1` second UV set.

Where a feature lives almost exclusively in one pipeline (e.g. draco and
spec-gloss are overwhelmingly scene-authored, high-joint skins overwhelmingly
avatar), the list pulls from whichever pipeline actually carries it.

## Features deliberately NOT targeted

- **Sparse accessors** (`accessor.sparse`) — not separately mined; effectively
  absent from this corpus in practice. Exporters used for Decentraland content
  do not emit sparse accessors, so there are no real entities to validate
  against. Expected absent.

## Regenerating

Read-only mining script: `build_blindspot.py` (reuses the glTF-parsing helpers
in `build_test_corpus.py`). It enumerates unique avatar + scene glbs from the
`content` postgres DB, resolves each CID under `ABGEN_CONTENT_ROOT`
(`<root>/<sha1(cid)[:4]>/<cid>`), classifies features, and writes the deduped
entity list plus a `blindspot_queue_detail.json` per-entity feature map.

```bash
export ABGEN_CONTENT_ROOT=$CONTENT_ROOT   # your content snapshot
python3 build_blindspot.py <avatar_glb_limit> <scene_glb_limit> blindspot_queue.txt
```

The script does **not** run the converter or abgen — generating the reference
bundles from this list is a separate, future step.
