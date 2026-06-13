# glb-with-morph final closure

After landing the m_Shapes tangent rule (commit TBD), the 8 glb-with-morph
bundles drop from **17,366 bits / 122.27 ppm** to **2,104 bits / 14.81 ppm**
(combined paired_bits=142,024,736; -87.9%).

| cid | before | after | residual |
|-----|-------:|------:|----------|
| `bafkreibgohlfx…` | 15,269 | 7 | Transform pid `1627470370868671357` (quat w sign) |
| `bafkreia644b…`   |  1,660 | 1,660 | AssetBundle dep order (1,612), Transform (34), Material `04↔02` (14) |
| `bafybeic63q…`    |    253 | 253 | Mesh `m_BonesAABB` X-precision (247), Transform (6) |
| `bafkreif62ig…`   |    184 | 184 | AssetBundle dep order (184) |
| 4× zero-diff      |      0 | 0 | — |

The fix landed two empirical rules in `build_m_shapes`
(`src/mesh_layout.rs`):

1. **Keep-vertex predicate** uses POSITION ∨ NORMAL only — TANGENT is not a
 keep-signal (was `POSITION ∨ NORMAL ∨ TANGENT`).
2. **`hasTangents` is always `false`**; the per-vertex `tangent` slot is
 either `(0,0,0)` (when the source target lacks `TANGENT`) or a verbatim
 copy of the `normal` triple (when the target ships a `TANGENT`
 accessor — the converter's importer evidently reuses the normal buffer
 instead of reading the tangent).

Verified across all 4 prod bundles with non-empty `m_Shapes.vertices`:
every `hasTangents = false`, every vertex `tangent` is either zero or
byte-for-byte equal to `normal`. Bundles #1 (`bgohlfx`, `Plane_0`) ship
`TANGENT` in target and prod tangent equals normal; bundles #2/#3
(`a644b…` mesh `0`/`1`, `c63q…` mesh `M_uBody`) ship POSITION+NORMAL
only and prod tangent is exactly zero. The `tan_present` switch in the
fix matches this on a per-target basis.

## Residuals — non-trivial, not landed

### `bafybeic63q…` Mesh `m_BonesAABB` X-axis precision (247 bits)

Only the `x` axis of `m_BonesAABB[*].m_Min`/`.m_Max` differs; y/z match
exactly. Sample: bone[0] ours x=`-18.31805` vs prod x=`-17.97576` (Δ
≈ 0.342). Our `compute_bones_aabb` widens each bone box by every morph
delta (`base + tgt.positions[vi]`). The delta scale matches the
discrepancy (one of the morph POSITION deltas is ~0.342), suggesting
the reference weights the morph contribution rather than taking the raw delta:
maybe `base + mesh.weights[ti] * delta` (which is zero when
`mesh.weights = [0,0,…]` as in this corpus), or maybe `base + delta`
clamped/folded against the bind matrix differently.

Dropping the morph contribution entirely *increases* diff to 405 bits
on this mesh and adds 119 bits on `bafkreia644b…` mesh `0`, so the
contribution IS needed — just not via a raw sum. Next step: write a
synthetic probe that constructs a single-bone single-morph mesh, runs it
through the upstream Unity AssetBundleBrowser, and reads back
`m_BonesAABB` to determine the actual aggregation rule. Until that's
done the 247-bit residual is parked.

### `bafkreia644b…` AssetBundle dependency ordering (1,612 bits)

`AssetBundle.m_Dependencies` byte runs at 0x9a, 0xa6, 0xb2, … show
8-byte hash-string fragments being permuted. Same signature as the 184-bit
`bafkreif62ig…` residual: prod is using a different sort key for
`m_Dependencies` than us. This is a generic AssetBundle issue (not
morph-specific) — kept for the broader sf_other/AssetBundle work.

### Transform quaternion `w` bit-flips (47 bits across 4 bundles)

`@0x1f: 00↔80`, `@0x13: 00↔80`, etc — all single-byte sign flips on
quaternion components. Same pattern as the existing transform residuals
documented in the wearable / scene buckets; not a morph-specific issue.

### Material `04↔02` (14 bits)

`bafkreia644b…` materials carry a `04` byte where prod has `02` at
fixed sub-field offsets — likely a default-value field (m_Filter? m_Wrap?)
where we emit a different default. Generic Material issue, not morph
specific.

## What changed in code

`src/mesh_layout.rs::build_m_shapes` only — six-line vertex push +
`hasTangents = false`. No changes outside that function. 118 lib tests
and `parity_bytes` (488,000 cap) still green.
