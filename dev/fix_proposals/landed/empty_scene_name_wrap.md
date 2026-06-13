# Empty scene-name wrap layer — windows test set (pathid_rt_v10_windows)

> **Status: landed.** Bundles whose source glTF has
> `scenes[idx].name = null/missing` were emitting a wrap_path of
> `scenes//<empty>/...`, which mismatched the reference's actual prefix
> `scenes//Scene/...`. The fix substitutes the literal `"Scene"` for the
> wrap-layer GameObject component when the scene name is empty.

## TL;DR

When a glTF file's scene is unnamed, the converter wraps the
scene under a GameObject named with the literal `"Scene"`. Our previous
code repeated `scene_name` for both the `scene_path` and the wrap layer
— with an empty scene name this collapsed to `scenes//<empty>/`, which
hashed to a completely different recycle path than the reference's `scenes//Scene/`.

For the `bafkreieitvafkjz5jhgjtcnmoj35rsblo2nnd3gyonlupy2mxstmarzfly/...rover.glb`
bundle (67 GameObjects, scene = `{nodes:[0,5]}` with no `name`), this
broke **every** GO/Transform PathID — 0/67 paired before fix, 67/67
after.

## Per-class ppm-bits delta (windows v10, 22 entities, 2158 paired bundles)

| Class                | Before (ppm) | After (ppm) | Δ        |
|----------------------|-------------:|------------:|---------:|
| MeshRenderer         |      150,634 |     123,615 |  -27,019 |
| Transform            |      139,387 |     116,569 |  -22,818 |
| GameObject           |      151,205 |     129,594 |  -21,611 |
| SkinnedMeshRenderer  |       11,310 |          19 |  -11,291 |
| MeshCollider         |       60,831 |      37,169 |  -23,662 |
| MeshFilter           |      134,477 |     109,209 |  -25,268 |
| Combined (3-class)   |      441,226 |     369,778 |  -71,448 |

## Recycle-path probe (verified bit-exact via xxh64 + spookyhash chain)

For `bafybeihq3jwh5457cnpfwwccnz4dyykwdzlreskba4htgjhk33dsvxx2ga` (rover.glb):

| GO              | gltf-node path                                  | prod recycle                                  | match |
|-----------------|-------------------------------------------------|-----------------------------------------------|:-----:|
| root (cid name) | `(scene root)`                                  | `scenes//Scene`                               | ✓     |
| `Geometry`      | `Geometry`                                      | `scenes//Scene/Geometry`                      | ✓     |
| `Hip`           | `Root/Hip`                                      | `scenes//Scene/Root/Hip`                      | ✓     |
| `Chest`         | `Root/Hip/Spine1/Spine2/Chest`                  | `scenes//Scene/Root/Hip/Spine1/Spine2/Chest`  | ✓     |
| `Head`          | `Root/Hip/Spine1/Spine2/Chest/Neck/Head`        | `scenes//Scene/Root/Hip/...`                  | ✓     |
| `Briefcase`     | `Root/Briefcase`                                | `scenes//Scene/Root/Briefcase`                | ✓     |

All 5 derived PathIDs reproduced their prod counterparts byte-exact via
the standard recycle-name → PathID chain
(`local_id_for_recycle_name("GameObject", recycle) →
prefab_packed_path_id(guid, lid, FILE_TYPE_META_ASSET)`).

## Implementation

`src/builder.rs:1094` — the `wrap` branch in `build_scene`:

```rust
// before
let wrap_path = format!("{scene_path}/{scene_name}");
// after
let wrap_inner: &str = if scene_name.is_empty { "Scene" } else { scene_name };
let wrap_path = format!("{scene_path}/{wrap_inner}");
```

Bundles with named scenes (`"defaultScene"`, `"AuxScene"`, `"OSG_Scene"`,
etc.) keep the existing `{name}/{name}` doubling — verified by
re-running the per-class measurement: bundles like
`QmUwuAD.../QmbJrR8MtMQtBz1ZZqfdLpjoS3q5KhdR3kRDJSWRZVRDvF` (with
`scene.name = "defaultScene"`) maintain their 25/25 paired GameObjects.

## Remaining residual

The trio's residual after this fix (~370k ppm) is dominated by two
classes of bundles this PR does **not** address:

1. **Empty-name intermediate nodes** in FBX-via-glTF chains (`bafkreibwoex...`
 bird.glb and similar): nodes 3 and 23 have `name=""`. Our code uses
 `"GameObject"` as fallback; prod uses something else (brute-force
 probe over alphanumeric strings of length 1–3 + many Unity defaults
 did not surface the literal). This is the next-largest cluster
 (~10 of the top-25 bundles by unpaired bits).

2. **Multi-primitive node merging** in scene bundles (`QmUwuAD3p...`
 museum scenes; entity13228.gltf has 5 nodes with 6–7 primitives each).
 Prod merges all primitives of a node into ONE Mesh with multiple
 sub-meshes and ONE MeshRenderer with `m_Materials.length =
 primitive count`. We emit child GameObjects (one per `prim[1..N]`),
 which doubles GO/Transform/MR counts and produces unpaired residuals.

## Tests

```
$ cargo test --release --lib → 116 passed, 0 failed
$ cargo test --release --test parity_bytes → 2 passed
```

The parity fixtures all have `scene.name = "Scene"` (non-empty), so the
new branch is exercised only on the v10_windows corpus, not on the
golden fixtures.
