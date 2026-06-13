# Mesh normal/tangent quantization bit-residual (RESEARCH_AREAS #10) — negative finding

Baseline commit: 12e0ff4. Reference: `ad0564d-windows` test-set (4243 bundles).
Tool: `examples/mesh_nt_census.rs` (added here) decodes Mesh `m_VertexData`
per-channel and compares ours vs ref on **size-matched** Mesh objects (paired by
`m_Name`), isolating bit-value residual from any size/structure miss.

## Headline census (baseline, /tmp/ours_baseline vs ad0564d-windows)

```
positions:  0 diff bits   (all size-matched meshes byte-identical)
uv0:        0 diff bits
normals:    5367 size-matched, 5366 byte-identical, 53565 diff bits  <- ALL from ONE mesh
tangents:   462  size-matched, 349  byte-identical, 10146 diff bits  <- the real area-#10 class
```

### Normals are effectively solved
- 5366 / 5367 size-matched meshes are byte-identical.
- The single diffing mesh (`TyglooShape`) diffs by d_ulp ~6.9M on component 0
  (ref `bf5af969` vs ours `bef0e2a2`) — a wholly different value, i.e. a
  name-pairing / topology artifact (different mesh matched by name, or a
  no-NORMAL fallback), NOT a quantization/rounding residual. Out of scope for #10.
- Storage format confirmed f32, not packed SNorm: glTFast
  `VertexBufferConfig.cs:343` declares `VertexAttribute.Normal,
  VertexAttributeFormat.Float32, 3`. Float normals are imported by
  `ConvertVector3FloatToFloatInterleavedJob` (`Jobs.cs:1260`) which does only
  `tmp.x *= -1` — bit-identical to our `[-n[0], n[1], n[2]]` (`gltf.rs:731`).
  Normalized SHORT/BYTE normals would be renormalized (`GetVector3Job`
  `ensureUnitLength`), but the corpus exhibits no normal residual, so that path
  is not exercised here.

### Tangents are the residual class — and it is irreducible native arithmetic
Computed tangents (glTF lacks TANGENT) come from Unity's **native
`Mesh.RecalculateTangents()`** — confirmed in glTFast
`PrimitiveCreateContext.cs:114`. Read tangents (TANGENT present) use
`ConvertTangentsFloatToFloatInterleavedJob` (`Jobs.cs:1346`, only `tmp.z *= -1`)
and are bit-exact with our `gltf.rs:758`. So the residual is entirely in our
`src/tangents.rs` clean-room reimplementation of `RecalculateTangents`.

Diff structure on the 113 diffing tangent meshes:
```
component ULP hist [=1, =2, 3-8, 9-32, 33+] = [2930, 114, 143, 78, 484]
per-mesh max-ULP class [max=1, max<=4, max<=32, max>32] = [32, 30, 15, 36]
lane diffs [x,y,z,w] = [1276, 1255, 1103, 115]   (w = handedness sign, 115 flips)
```
- ~2930 of 3749 differing components are exactly 1 ULP — a narrowing/ordering
  residual, spread evenly over x/y/z.
- 36 meshes diff by >32 ULP (e.g. `DiscoBall` d_ulp -611, `Object_0.002` +29) —
  topology / UV-seam / degenerate-triangle cases, not arithmetic order.

## Experiments (all corpus-verified on the same 462 size-matched tangent meshes)

| change | byte-id meshes | tangent diff bits | verdict |
|---|---|---|---|
| baseline (f64 accumulate, single f32 narrow at output) | **349** | **10146** | — |
| f32-round `tan1/tan2 +=` accumulation per step | 317 | 268548 (26x worse) | REJECTED |
| f32-round entire final Gram-Schmidt + normalize | 317 | 522479 (51x worse) | REJECTED |
| FMA (`mul_add`) on final `d = n·t` dot | 349 | 10145 (−1 bit) | neutral, not principled |

The two natural "Unity works in float" hypotheses (f32 accumulation; f32 final
orthonormalize) make parity **dramatically worse**, proving Unity's
`RecalculateTangents` keeps double-precision temporaries during both the
angle-weighted accumulation and the final orthonormalization — which our f64
model already replicates. The f64 implementation sits at a sharp local optimum:
every arithmetic-order perturbation tested is either neutral or a large
regression.

## Why it's irreducible
The current `tangents.rs` already encodes the non-obvious Unity specifics
(per-triangle sdir/tdir normalize, `acos(angle) * |den|` weighting) that match
349/462 meshes bit-for-bit. The remaining 1-ULP residual is the difference
between Unity's native (Burst/SIMD-compiled) instruction sequence for the
double-precision dot/sqrt/divide chain and our scalar f64 chain at shared
vertices with many contributing triangles. Reproducing it bit-exactly would
require reading Unity's native `RecalculateTangents` codegen — which is
off-limits under the clean-room rule. The triangle-accumulation order already
matches (we iterate the same winding-flipped index buffer Unity recalculates on,
`gltf.rs:808` → `prim.indices`).

## Magnitude / priority
Whole tangent residual is 10146 bits across the entire 4243-bundle test set
(total corpus diff is 4.01e9 bits). Tangents are ~0.00025% of the residual.
Even a perfect tangent fix is invisible at the bundle-kind verify level. Normals
(the larger nominal number, 53565 bits) are a single mispaired mesh, not a
rounding class. **Area #10 is closed as fundamentally low-yield + irreducible:**
the size is already right (f32, matches the reference), normals are bit-solved, and the
tangent bit-residual is native-arithmetic noise that resists every clean-room
ordering model.

## Artifacts
- `examples/mesh_nt_census.rs` — the per-channel size-matched mesh diff census
  (kept; reusable for future mesh-bit work).
