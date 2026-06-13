# val-300 corpus coverage map

Clean-room analysis of which input feature paths the val-300 reference corpus
(the windows reference built from `tests/corpora/validation300_queue.txt`,
roughly 300 entities) actually exercises, so we know where a parity bug could
hide because the scoreboard never touches that path. Derived by parsing each
entity manifest and its source glbs/textures directly from the content store
(read-only). Proportions only — no precise counts.

## Entity mix

- **Wearables dominate** (~60% of entities), **scenes** next (~one third),
  **emotes** a small slice (~7%), plus a single **profile**. There is no
  standalone-emote-heavy sampling — emotes are thin.
- Wearables ship male+female glb pairs, so glb-level tallies double-count a
  single authored asset. The per-entity view below is the reliable signal.

## Well-covered (safe — a bug here would be caught)

- **Skinned meshes** — present in the large majority of entities (nearly all
  wearables + emotes). Joint counts span a wide range (single-digit up to
  ~150). This path is very well exercised.
- **Multi-primitive meshes** and **multi-mesh glbs** — common; mesh counts
  range from 1 up to several hundred in heavy scenes. Good spread.
- **Animations** — common; about a quarter of entities animate, and many carry
  *multiple* clips (distribution runs from 1 up into the dozens, with a cluster
  around 15-17 clips). Multi-animation is well sampled.
- **Emissive materials** (emissiveFactor / emissiveTexture set) — common across
  both scenes and wearables.
- **UV1 / second UV set** — present in a meaningful minority of entities;
  adequately sampled (most assets are UV0-only, which is also the common case).
- **Vertex color (COLOR_0)** — present in a solid minority of entities. OK.
- **Texture formats**: PNG is overwhelmingly the norm; **JPEG** is a real but
  secondary slice (well sampled); both are exercised.
- **Texture sizes**: a full ladder from tiny up through 2048 is present, with a
  large mass at 512 and 1024. Plenty of textures sit **at or above the 1024
  platform cap**, including some 2048/4096/8192 — the downscale/cap path is
  well exercised.
- **KHR_materials_specular / _ior / _clearcoat / _emissive_strength** — each
  appears across a moderate set of distinct entities (roughly 1-in-15 to
  1-in-12). Reasonable, though not deep — see below.
- **KHR_materials_unlit** — present in a handful of entities; lightly but
  genuinely sampled.

## Rare (thin — a bug here is easy to miss)

- **KHR_draco_mesh_compression** — only a few distinct entities (and they are
  draco-*required*, so the decompress path is load-bearing for them). Parity on
  the draco materialize path rests on a very small sample.
- **KHR_materials_pbrSpecularGlossiness** — a small number of entities, mostly
  marked required. The spec-gloss → metallic-rough conversion path is thinly
  sampled.

## Rare-to-absent (essentially blind spots)

- **Morph targets / blendshapes** — exercised by **exactly one** authored
  wearable (its male+female pair). Morph weight/name ordering, sparse-vs-dense
  morph accessors, and multi-target meshes are effectively untested. **Highest
  risk.**
- **KHR_materials_transmission** — **one** scene entity. Volume/sheen are
  **absent** entirely (zero occurrences).
- **KHR_texture_transform** — **one** scene entity. UV transform math is
  almost entirely unverified.
- **Sparse accessors** — **none observed** in the corpus, even though the parser
  has a dedicated sparse path. Completely unexercised here.
- **Explicit LODs (MSFT_lod)** — **none**. (Scene-collider/LOD bundle kinds in
  the scoreboard come from naming/conventions, not glTF LOD extensions.)
- **Non-PNG/JPEG textures** — PSD appears a handful of times, WEBP once; these
  decode paths are barely touched. KTX/KTX2 absent.
- **Malformed glb inputs** — two manifest-declared `.glb` files are zero-byte-
  prefixed / not valid GLB. The importer's tolerance for broken source assets
  is an untested-by-design edge (both happen to have a valid sibling variant, so
  feature coverage isn't lost, but error-path parity is unknown).

## Prioritized under-sampled paths for a future corpus

1. **Morph targets / blendshapes** — 1 asset today. Add several wearables/emotes
   with varied target counts, named targets, and sparse morph accessors. This is
   the single largest blind spot relative to parser surface area.
2. **Sparse accessors** — 0 today, but the parser has a full sparse branch.
   Any silently-wrong sparse decode would be invisible. Add assets that use them
   (often co-occurs with morph targets).
3. **KHR_texture_transform** — 1 today. UV offset/scale/rotation needs a handful
   of scenes to validate the transform-to-Unity-UV mapping.
4. **KHR_materials_transmission / _volume / _sheen** — 1 / 0 / 0. Add a small
   set so the extension-to-shader fallback is checked.
5. **KHR_draco** and **pbrSpecularGlossiness** — each only a few entities and
   both on load-bearing required paths; deepen the sample so a regression isn't
   masked by one or two assets happening to match.
6. **Non-PNG/JPEG textures** (PSD/WEBP/KTX2) — broaden so the alternate decode
   paths get real coverage.
7. **Emote-specific paths** — emotes are a thin slice (~7%); their property
   curves / prop animations deserve more samples than the corpus gives today.
