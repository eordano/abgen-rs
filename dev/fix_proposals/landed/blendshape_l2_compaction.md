# Blendshape compaction uses Euclidean (L2) magnitude >= 1e-5, not per-component

> **Status: landed.** Closes the dominant glb-wearable size-divergence cluster.
> windows test-set, kind=glb-wearable: diff-bits 864,366,483 -> 840,466,217
> (-23,900,266 bits, -2.76%); ppm 321,732.7 -> 312,836.6. TOTAL diff-bits
> 4,013,834,953 -> 3,989,934,687. No other kind regressed. Parity gate green.

## What the brief expected vs what was actually wrong

The area brief hypothesised the glb-wearable smaller/larger split came from a
**vertex-stream length / channel-set** mismatch (which attributes the importer keeps,
packed stride). That is NOT the cause for this corpus.

`examples/dump_chan` on the worst-divergent bundles shows the channel set,
per-channel format/dimension, stream layout, and `m_VertexData.m_DataSize` are
**byte-identical** between ours and ref. The vertex stream length is already
correct. `examples/dump_objsize` then pinpoints the divergence to the **Mesh
object body**, and `dump_mesh_diff` (throwaway) showed the only structural
field that differs is `m_Shapes.vertices[len]` — ours emits a few MORE morph
(blendshape) delta vertices than the reference. The extra vertices shift every
subsequent vertex byte, cascading into a multi-Mbit per-mesh diff and a
slightly larger post-LZ4 bundle.

## The dominant cluster

One entity, `bafkreigj6enc6maoevdz2tzoohzqlz5itnqcaknzrrggojkhg2btlq7p2e`
(15 bundles, all size-divergent), accounted for **548M of the 864M** baseline
glb-wearable diff-bits (63%). All its meshes are Ready-Player-Me-style avatar
heads/bodies with dense per-vertex blendshape (morph) targets.

## Root cause — the keep/drop predicate is on the L2 magnitude

Unity's mesh importer drops a morph-delta vertex when the delta is
negligible. abgen-rs (post mesh_close_21 Close #2) used a **per-component**
threshold: keep iff any single component of pos OR normal exceeds 2^-23.

That cannot reproduce the reference. Proper set-based comparison (keyed on
`(shape_index, vertex_index)` to avoid tandem-walk desync —
`examples/probe_setdiff`) over all 213,778 candidate morph verts in the worst
entity shows the per-component metric **overlaps**: ref keeps verts whose
max-component normal delta is as small as 6.47e-6 while dropping verts whose
max-component normal delta is as large as 9.93e-6.

The discriminant that separates cleanly is the **Euclidean (L2) magnitude** of
the delta vector (`examples/probe_overlap`, throwaway):

- max DROPPED (pos==0) normal L2 = 9.995e-6
- min KEPT   (pos==0) normal L2 = 1.0002e-5

A sweep (`examples/probe_l2`) confirms the boundary is exactly **1e-5**:

```
T=9.9e-6 : extra=18 missing=0
T=1e-5   : extra=0  missing=0   <- perfect
T=1.05e-5: extra=0  missing=69
```

extra = kept-by-us/dropped-by-ref; missing = dropped-by-us/kept-by-ref. The
window is one-sided and tight: 1e-5 is the unique clean cut across 213k verts.

Unity's rule, recovered black-box:

> Keep a morph-target vertex iff `||delta_position|| >= 1e-5` **or**
> `||delta_normal|| >= 1e-5`, where `||.||` is the Euclidean vector magnitude.

This is the well-known Unity blendshape import epsilon (1e-5), applied to the
delta vector magnitude — not to individual components.

## The fix

`src/mesh_layout.rs::build_m_shapes`:

```rust
const KEEP_EPS: f64 = 1e-5;
let l2 = |v: [f64; 3]| (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
...
let keep = l2(p) >= KEEP_EPS || l2(nrm_vec) >= KEEP_EPS;
if !keep { continue; }
```

The per-component `nonzero` (2^-23) helper is retained only for the
`hasNormals` flag bookkeeping (unchanged behaviour for that flag).

## Verification

```
baseline glb-wearable : 125 bundles, 43 byte-id, 57 smaller + 15 larger,
                        diff-bits 864,366,483, ppm 321,732.7
after    glb-wearable : 125 bundles, 43 byte-id, 57 smaller + 15 larger,
                        diff-bits 840,466,217, ppm 312,836.6
```

- byte-id count unchanged (these bundles carry independent BC7/texture
  residuals, so closing m_Shapes alone does not make them byte-exact — it cuts
  the cascading mesh-body diff).
- `cargo test --release --test parity_bytes` -> ok (2 passed).
- `cargo test --release --lib` -> 129 passed; the only 2 failures
  (`bc7_pure::bit_exact_all_vectors`, `bit_exact_mip_chains`) are **pre-existing
  on the clean baseline** and unrelated to this change.

## Reproduction tools (committed under examples/)

- `dump_chan` — dump m_VertexData channels/streams/dataSize for both sides
  (proves the stream length already matches).
- `dump_objsize` — per-object class + length + bytes-diff (pinpoints Mesh body).
- `probe_setdiff` — set-based morph keep/drop bracket (reliable, no desync).
- `probe_l2` — L2-threshold sweep against the ref keep set.

## Residual / next step

The glb-wearable diff-bits floor (840M) is now dominated by BC7 texture
residuals and the remaining per-vertex value drift, not by m_Shapes length.
The few non-cluster entities with morph targets were not separately
cross-validated bundle-by-bundle (arg-list length limited the multi-bundle
probe), but the rule is content-agnostic (a single global epsilon on the
delta magnitude) and the full-corpus verify shows a net win with zero
regression in any kind, so the rule generalises.
