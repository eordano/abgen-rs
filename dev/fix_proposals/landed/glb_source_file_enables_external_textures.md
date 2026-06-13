# Cross-bundle texture resolution for plain `.glb` scenes — landed

Base: `7a3a9cf`. Fix commit: `808b558`. Closes the headline lead of
`../sf_region_residual_drill.md` ("414 bundles miss cross-bundle
external entries").

## What the drill said

The drill confirmed the SF writer is byte-perfect on bundles where the
externals set, externals order, and per-pid object payloads all match
ref. The remaining residual decomposes into three builder-side issues;
the largest is 414 bundles where the Material `_BaseMap` slot resolves
to a sibling-bundle texture but ours emits `(0,0)` PPtr because the
builder's external walker never gets the chance to map URI → sibling
hash.

## Root cause

`abgen-corpus`'s `from_reference` gates `source_file` on file
extension:

```rust
let source_file = if inv.contains_key(&cid)
    && (glb_file_l.ends_with(".gltf") || glb_file_l.ends_with("_emote.glb"))
{
    Some(glb_file.clone())
} else if is_image {... } else { None };
```

For a plain `.glb` (a scene component model that lives at
`models/Statue_01/Statue_01.glb` inside the entity), `source_file` was
`None`. That made `build_one` disable `resolve_hash` (gated on
`spec.source_file.is_some`) and made the gltf bytes-resolver use a
bogus base path (`{cid}.glb` instead of the real in-entity path).
With both resolvers blind, the image URI `FanstasyPack_TX.png` never
joined to the in-entity key `models/Statue_01/FanstasyPack_TX.png`, so
`content_by_file` lookup missed, `scene.images[idx]` stayed `None`, and
both `external_texture` and `texture` returned `None` — the slot ended
up at `(0,0)` and the sibling CAB was absent from `ext_bundle_files`,
which is the source of SF externals.

`m_Dependencies` had already been patched to union with
`metadata_dependencies` (21fc31c), so the dependency name was present;
but SF externals + the Material PPtr remained broken because they come
from a different code path (`external_texture` → `ext_bundle_files`,
not metadata).

## Fix

Allow `.glb` in the `source_file` gate alongside `.gltf` and
`_emote.glb`. The in-entity layout is identical (URIs are relative to
the file's directory); the only reason the gate excluded `.glb` was a
mistake of inheritance from an earlier code path. Mirrored in the
Python manifest builder under `tests/corpora/`.

## Effect (val corpus, 1,975 bundles, )

```
                   before   after   delta
byte-id total 462 591 +129
glb-scene byte-id 334 462 +128
glb-animated byte-id 50 51 +1
diff-bits 3,183,702,789 3,158,377,188 -25,325,601
ppm 399,301.5 402,503.4 −3,201.9
regressed 0
```

Sample bundle (the drill's worked example): `QmdrZb7Y…/QmZuef…` — SF
externals goes from 1 to 2 entries; the missing
`CAB-096e39549c0bc9b37624ec61f0a1cab9` is now emitted with the
matching Material `_BaseMap` PPtr at
`(m_FileID=2, m_PathID=3681241395579410707)` — byte-identical to ref.

## Validation

- `cargo test -p abgen --release --test parity_bytes` — both subtests
 pass (parity fixtures use `.gltf` + `.png` and are unaffected).
- The pre-existing `bc7_pure::tests::bit_exact_all_vectors` failure on
 main is unchanged.

## Still open from the drill

- **26 bundles same externals, different order** — slot rule in
 `sbp_order::build_preload_and_container_with` accumulates externals
 in `external_texture` call order; ref uses an order we haven't
 reverse-engineered yet. Not addressed here.
- **10 QmVkQnvK4u… 102-vs-3297 object-count mismatch** — pure content
 miss (missing primitive children / AnimationClips). Not addressed
 here.
