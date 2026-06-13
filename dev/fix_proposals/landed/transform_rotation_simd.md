# Transform.m_LocalRotation — Root Cause D, the SIMD lane-split normalize

## TL;DR

The 80 Transform residuals on `m_LocalRotation.{y,w}` that the earlier
forensic (`transform.md`) had labelled "not closable by any scalar formula
we'd enumerated" turned out to be **fully closable** once hybrid
normalizations were added to the search: the `x` and `z` lanes
divide by `sqrt((x²+y²)+(z²+w²))`, while the `y` and `w` lanes divide by
`sqrt((x²+w²)+(y²+z²))`. Same f32 math throughout; only the inner pair-sum
order changes between the two lane-pairs.

Closed 80/80 Transform residuals + ~150 dependent objects (mostly
MeshRenderer, which references Transform PathIDs) in the 280-bundle corpus.

Patch lives in `src/gltf.rs::normalize_quat_f32` (commit 8c05e14, bundled
with Root Cause J).

## Why the earlier forensic missed it

`transform.md` §5 ran an exhaustive scalar search over:

| partition | scheme | result |
|---|---|---:|
| `(xy)(zw)` | V0 — current Rust | 0/78 |
| `(xz)(yw)` | V1 | 0/78 |
| `(xw)(yz)` | V7 | 0/78 |
| 24 left-assoc perms × 3 pair partitions × 2 add orders | | 0/78 |
| f64-throughout, f64sum+f32div, f64inv+f32mul, NoNormalize | | 0/78 |
| `VHybrid_yw_only_f64div` | **all four lanes** divide by an f64-quotient on y/w only | 7/78 |

The exhaustive search treated normalize as a single global scale: pick a
pair-partition, compute one `s = sum(sq)`, one `n = sqrt(s)`, and divide
every lane by `n`. That family of formulas can never produce prod's
output, because prod has `.x` and `.z` matching the **V0** rounding while
simultaneously `.y` and `.w` match the **V7** rounding — two different
norms applied to two different lane subsets in one pass.

Forensic doc §3 actually shows the smoking gun ("`.x` and `.z` are NEVER
perturbed") but the search didn't enumerate **hybrid normalizations**, so
nothing matched.

## The empirical fingerprint, re-read

For `Avatar_RightForeArm` (case 1 in `transform.md` §5):

```
raw f32 (post axis-flip): x=3e8307c6 y=3d1e4631 z=bcf87f68 w=3f7727af
prod (Unity bundle): x=3e8307c6 y=3d1e4632 z=bcf87f68 w=3f7727b0
V0 scalar normalize: x=3e8307c6 y=3d1e4631 z=bcf87f68 w=3f7727af (matches raw)
V7 scalar normalize: x=3e8307c7 y=3d1e4632 z=bcf87f68* w=3f7727b0
 (* V7 also bumps x/z in some other cases; here it happens to round same)
```

prod = (V0's x, V7's y, V0's z, V7's w) — a **per-lane mix** of the two
pair-partitions. Run the same comparison on all 78 cases:

```
HYBRID (V0 x/z, V7 y/w) match: 78/78 (100%)
```

## Why Unity does this

Almost certainly an SSE `_mm_shuffle_ps`-style horizontal-add inside the
engine's `Transform::SetLocalRotation` (closed-source). Most likely
candidates:

1. `_mm_hadd_ps(q*q, q*q)` followed by a `_mm_movehl_ps`/`_mm_movelh_ps`
 blend, where the high pair comes from one add chain and the low pair
 from another.
2. Independent `_mm_dp_ps(xz_mask)` and `_mm_dp_ps(yw_mask)` for the two
 lane sub-sets, broadcast through `_mm_shuffle_ps` for the divide.
3. A `_mm_rsqrt_ps` + Newton-Raphson refine where the refine step's add
 order differs between the high and low pair due to register shuffling.

All three would expose the same observable: even-index lanes carry one
round-off path, odd-index lanes carry the other.

## The patch

`src/gltf.rs::normalize_quat_f32`, all f32:

```rust
let qq = [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32];
let sq = [qq[0]*qq[0], qq[1]*qq[1], qq[2]*qq[2], qq[3]*qq[3]];
let s_xz = (sq[0] + sq[1]) + (sq[2] + sq[3]); // (xy)(zw) for x,z lanes
let s_yw = (sq[0] + sq[3]) + (sq[1] + sq[2]); // (xw)(yz) for y,w lanes
let n_xz = s_xz.sqrt;
let n_yw = s_yw.sqrt;
[ qq[0]/n_xz, qq[1]/n_yw, qq[2]/n_xz, qq[3]/n_yw ] // back to f64 for Value
```

Trivial cost: 2 sqrts instead of 1, 4 divides total instead of 4 — runtime
parity-test still completes in 38 s.

## Impact

| Class      | Before | After | Δ   |
|------------|-------:|------:|-----|
| Transform  | 80     | 0     | −80 |
| MeshRenderer | 102  | 0     | −102 (knock-on: PathIDs of children stabilise) |
| Material   | 46     | 4     | −42 (compounded with Root Cause F + J) |
| AssetBundle | 79    | 74    | −5  (preload-table shuffles less) |
| **Overall paired-object byte-exact** | **36300 ppm differ** | **20900 ppm differ** | **−15400 ppm** (+228 objects) |

The MeshRenderer drop is dominantly **the same fix**: Unity's
`MeshRenderer.m_GameObject.m_PathID` is salted by the parent Transform's
PathID, and the parent Transform's PathID is a hash of its serialised
TypeTree (including `m_LocalRotation`). Fix the rotation bits, fix the
Transform CAB hash, fix every dependent PathID downstream.

## What remains

8 paired-object classes still residual (see `bitwise_residuals.md`):

- `Texture2D` (113) — streaming-resource sharing (`m_StreamData.path` /
 `.offset` / `.size`); Root Cause C partial fix already in, the
 cross-bundle.resS sharing logic isn't fully wired up yet.
- `Mesh` (100) — `m_VertexData.m_DataSize` byte-stream divergence (Agent
 E sweep) + the 10 BonesAABB residuals (recompute-after-skinning includes
 morph deltas, still partial).
- `AssetBundle` (74) — `m_PreloadTable` PathID sort order + cross-bundle
 dependency PathID drift.
- `MeshFilter` (11) — `m_Mesh.m_PathID` drift (downstream of the Mesh fix).
- `GameObject` (8) — `m_Component` ordering for the few primitives with
 3+ components.
- `Material` (4) — TexEnv `m_Offset` / `m_Scale` overrides (`bafybeif66iyagwk`
 edge case).
- `TextAsset` (3) — `assets.json` dependency list (cross-bundle resolver).

None of these are quaternion-precision; each has its own focused
investigation queued.
