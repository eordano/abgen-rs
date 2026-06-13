# Unified per-primitive emit decision tree

Closes the remaining 8 `GameObject` residuals from `gameobject.md` patterns
2/3/4 and the residual `AssetBundle` `m_PreloadTable` length mismatches from
`assetbundle.md` §5b, without regressing each other (the historical
412-false-positive trap noted in prior framing).

## TL;DR

| class            | before | after | Δ   |
|------------------|-------:|------:|----:|
| GameObject       |      8 |     0 | −8  |
| AssetBundle      |     74 |    68 | −6  |
| Mesh             |    100 |   100 |  0  |
| MeshFilter       |     11 |    11 |  0  |
| MeshRenderer     |      0 |     0 |  0  |
| SkinnedMeshRenderer | 0   |     0 |  0  |
| Material         |      4 |     4 |  0  |
| Texture2D        |    113 |   113 |  0  |
| TextAsset        |      3 |     3 |  0  |
| **paired-object byte-exact** | **14684 / 14997** (20871 ppm differ) | **14706 / 15005** (19927 ppm differ) | **−944 ppm** |

`cargo test --release --test parity_bytes` still passes.

## The discriminator (oracle: 0 mispredictions over 1001 parent-prim cases)

Every glTF primitive that attaches to a node's GameObject (prim[0]) falls
into exactly one of four outcomes, determined by **four bits** derived
directly from the glTF data — no Unity-import-order replay, no manifest
heuristics, no per-bundle overrides:

```
is_collider = node.name.lower.contains("_collider")
is_skinned = node.skin is Some AND prim has JOINTS_0+WEIGHTS_0
has_morph = any primitive in this node's mesh has morph targets
is_parent_prim = the call is for prim[0] (vs prim[1..N] which become child GOs)
```

Decision (in evaluation order):

```
becomes_smr = is_skinned OR has_morph

1. is_parent_prim AND is_collider AND becomes_smr → TRANSFORM_ONLY
       (no MF, no MR, no MC, no SMR — parent GO keeps just its Transform.
        Mesh + material are still allocated because prod includes them in
        the bundle's preload table even though no GO references them.)

2. becomes_smr → SkinnedMeshRenderer
       (Unity drives both `m_Bones`/skinning AND `m_BlendShapeWeights` via
        SMR. For morph-only meshes with no glTF skin, the SMR ships empty
        `m_Bones` and `m_RootBone = (0,0)` — verified on craftNitro.)

3. is_collider → MeshFilter + MeshCollider
       (Allocate the explicit material as an orphan when
        `prim.material_index` is `Some` — 3 cases: Vase_05_collider,
        muscleDogePose_collider, marsDodge_collider.)

4. otherwise → MeshFilter + MeshRenderer
       (Default DCL_Scene material when `prim.material_index` is `None` —
        builder.py's pre-existing `self.material(scene, None)` already
        returns the default; no early-return.)
```

### Oracle coverage (from `dev/primitive_class.csv`)

```
is_collider is_skinned has_morph mat=None → outcome count
False False False False → NORMAL 530
False False False True → NORMAL 37
False False True False → SMR 5 ← new (rule 2)
False True False False → SMR 122
False True True False → SMR 1
True False False False → COLLIDER 3 ← new orphan-mat
True False False True → COLLIDER 300
True True False False → TRANSFORM_ONLY 2 ← new (rule 1)
True True False True → TRANSFORM_ONLY 1 ← new (rule 1)
```

Decision-tree mispredictions: **0 / 1001**.

## Why the prior single-fix attempts overfired

- **"Just emit MF+MR when `material_index is None`"** (assetbundle.md §5b
 alone) over-emits for the 3 skinned-collider parents — they have
 `material_index = None` but prod ships Transform-only on the parent. Adding
 MF+MR there would have produced 3 fresh GO mismatches (an "anti-fix" for
 GO pattern 4).
- **"Drop component emission on `_collider` nodes"** (gameobject.md naive
 read of pattern 4) — over-shoots: most colliders (303/306 in the corpus)
 DO get MF+MC. Suppressing all of them would create ~300 fresh GO
 mismatches.

The discriminator that splits cleanly is the four-bit combination above,
**specifically the interaction `is_collider AND (is_skinned OR has_morph)`**,
which fires on exactly 3 corpus nodes (the three Pattern 4 cases) and on
zero others.

### Why `has_morph` (not the animation channel path)

The naive fix for pattern 3 ("animation-target nodes become SMR") tried to
classify by the animation channel `path`:

```
13 ('rotation',) → MR (NOT SMR)
13 ('rotation','scale','translation') → MR
 3 ('translation',) → MR
 4 ('rotation',) → SMR
 1 ('weights',) → SMR
```

`rotation` lands on both sides — channel path is NOT the discriminator.
Adding the morph-target signal makes it cleanly partition:

```
has_morph=False, animated → MR (29 cases — Cube.010, OGCoin, BronzeCoin, …)
has_morph=True, animated → SMR (5 cases — craftNitro, craftrock2/3, craftPetrol, Nitro)
```

SkinnedMeshRenderer is the Unity component required to evaluate blend
shapes; the animation channel is incidental — it could be on rotation,
weights, or even nothing at all. The mesh-level `has_morph` is the
real driver.

## Implementation

Three files, ~120 net lines:

### `src/builder.rs`

1. **`pending_smr`**: change `skin_idx: usize` → `skin_idx: Option<usize>`
 (the morph-only SMR has no glTF skin).
2. **SMR resolution loop**: when `skin_idx is None`, emit
 `m_Bones = []`, `m_RootBone = (0,0)` (verified vs prod on craftNitro).
3. **`material_orphan(scene, mat_idx)`**: new entry point — like `material`
 but skips the `glb_referenced_mats` push (the orphan material is in the
 bundle's `material_K.mat` container entry but NOT in the.glb run's
 preload, because no GO under the.glb closure references it).
4. **`attach_primitive`**: gains an `is_parent_prim: bool` flag; the four
 rules above are encoded in order. The old early-return on
 `material_index.is_none` (already removed at HEAD) stays out.
5. **`build_node`**: passes `true` for prim[0], `false` for prim[1..N].

### `src/scene.rs` and `src/gltf.rs`

No structural changes needed — `Primitive.morph_targets`,
`Primitive.skin_index`, `Primitive.weights`, and `Node.is_collider` already
carry the discriminator signals. Parsing in `gltf.rs` already populates
them.

## Bundles closed by class

### GameObject (8 → 0)

| cid                      | node                | outcome change |
|--------------------------|---------------------|----------------|
| bafkreidr2qcyve3         | `Button_collider`   | SMR → TRANSFORM_ONLY (rule 1) |
| bafybeig5eieoabq         | `DogeGod_collider`  | SMR → TRANSFORM_ONLY (rule 1) |
| bafybeihx4i5ecjg         | `MarsDoge_collider` | SMR → TRANSFORM_ONLY (rule 1) |
| bafkreiallram3yq         | `craftNitro`        | MR  → SMR (rule 2, has_morph) |
| bafkreibzx26zpgz         | `craftrock3`        | MR  → SMR |
| bafkreidakad6vkr         | `craftrock2`        | MR  → SMR |
| bafkreif62igfe2b         | `craftPetrol`       | MR  → SMR |
| bafkreifz3xq7pj2         | `Nitro`             | MR  → SMR (weights anim) |

### AssetBundle (74 → 68)

Closes the 7 of 8 "longer" cases (preload over-emit from incorrectly-rule-2'd
collider parents and incorrectly-rule-4'd morph nodes), plus 2 of the 5
"shorter" cases (preload under-emit for collider mat=Some entries:
Vase_05_collider, muscleDogePose_collider, marsDodge_collider).

Remaining 68 AB residuals split into:
- **63 same-len content diffs** — shader-slot ordering within a material's
 preload run; not derivable from GLB data alone with the static rules we've
 swept. Three concrete paths to close are documented in `abgen/sbp_order.py`'s
 docstring (harvest Unity importer InstanceIDs, per-CID override table, or
 emit-and-verify dispatch).
- **4 shorter** — cross-bundle external textures (§5a, needs a resolver
 callback) + 2 JSON-not-glb routing cases.
- **1 longer** — `bafybeiczim5cqrv` (MarsDoge LAND), where ours has 11
 extra Mesh objects from an unrelated mesh-dedup pre-existing issue
 (separately tracked).

### Other classes

No regressions in MeshFilter (11), MeshRenderer (0), SkinnedMeshRenderer
(0), Material (4), Mesh (100), Texture2D (113), TextAsset (3).

## How to verify

```bash
cd ab-generator/abgen-rs
cargo build --release --bin ab-build-local
python3 dev/primitive_classifier.py" # 0 mispredictions
python3 dev/measure_full_vs_prod.py" # 19927 ppm differ
cargo test --release --test parity_bytes # passes
```

## Cross-references

- `dev/primitive_classifier.py` — oracle script
- `dev/primitive_class.csv` — classification of 1001 parent prims
- `dev/fix_proposals/gameobject.md` — original forensic, 4 patterns identified
- `dev/fix_proposals/assetbundle.md` §5b — default-material/MF/MR
- `src/builder.rs::attach_primitive` — implementation
- `src/builder.rs::material_orphan` — orphan-material allocator
