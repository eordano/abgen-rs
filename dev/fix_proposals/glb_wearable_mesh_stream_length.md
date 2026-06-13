# glb-wearable mesh vertex-stream LENGTH — RESEARCH_AREAS #4 (size class)

**Verdict: the premise is false on this corpus. The mesh vertex-stream LENGTH
is already byte-exact for every glb-wearable mesh. The ~205 size-divergent
glb-wearable bundles are NOT caused by a wrong stream length — they are LZ4
compression noise (200 bundles) plus a separate GameObject-name de-duplication
bug (4 bundles). Nothing to fix in `mesh_layout.rs` / `gltf.rs` channel logic.**

Corpus: `ad0564d-val300-windows` (ref) vs `/tmp/abgen-val300-integrated` (ours),
report `/tmp/abgen-val300-integrated-report.json`.

## 1. SOURCE

The report's `ours_bytes`/`ref_bytes` are the **LZ4-compressed UnityFS bundle
sizes**, not uncompressed object sizes. A small compressed delta does not imply
any object changed length. Decompressing the bundle and comparing the
uncompressed CAB + `.resS` payloads is the only valid size test, and it shows:

- The Mesh vertex/index streams (the `.resS` blob) and the Mesh object header
  (in the CAB) are **always the same length** as the reference.
- The remaining compressed-bundle delta is the LZ4 encoder reacting to
  bit-value differences in the CAB (Mesh AABB ULP / normal / tangent bytes —
  Areas 8/10 — and path-id ordering). Same input length, different compressed
  output length.

So the size question for Area #4 is already solved. The byte deltas live in
the bit-value tier, not the size tier.

## 2. Evidence (recoverable vs irreducible)

### A. No Mesh ever differs in length — corpus-wide

`examples/objalign` over **all 660 glb-wearable bundles**, every Mesh object:

```
glb-wearable checked: 660
total Mesh objects:   2014
Mesh SIZE diffs:      0
Mesh DIFF-flagged (bit-values, same size): 191
```

Zero meshes have a size difference. 191 differ in bit-values only (same length)
— that is Area 10's territory, not this one. The channel-inclusion rule in
`mesh_layout::build_channels` (positions always; normals always — recalculated
if absent; tangents iff source TANGENT or material has a normal map; colors iff
COLOR_0; all TEXCOORD_n; bones iff WEIGHTS_0+JOINTS_0) and the interleaved
stride/stream packing are already producing Unity's exact stream length on
every mesh in the corpus.

### B. Of the 205 size-divergent bundles (4 < |Δ| ≤ 1024): none is a mesh-length miss

`objalign` object-level size triage over all 205:

```
compress_only:    200   (no object differs in length at all)
mesh_size_diff:     0
other_size_diff:    5   (GameObject/Transform/SMR present one side only)
```

### C. The "compress_only" 200 — uncompressed payload is byte-size-identical

Worked example, `bafkreibrb4faieou5eb…/bafybeig4rb54rlj6rzbtl…_windows`
(report Δ = −19):

```
ours compressed: 525439     ref compressed: 525458   (Δ −19)
ours decompressed CAB:  98991      ref:  98991         (identical)
ours decompressed .resS: 5592432   ref: 5592432        (identical)
.resS byte diffs: 0          <-- vertex/index streams BYTE-IDENTICAL
CAB  byte diffs: 1089        <-- Mesh header bit-values + path-id ordering
```

The vertex stream is not merely the right *length* here — it is bit-perfect.
The entire −19 comes from LZ4 compressing the 1089 differing CAB bytes to 19
fewer bytes.

Random 12-bundle sample across the Δ range (−189 … +175): **every** bundle's
uncompressed payload size set is identical ours-vs-ref. Examples:
`Δ=−189` → both `[1192256, 25795936]`; `Δ=+175` → both `[9627936, 13981040]`;
`Δ=+115` → both `[164003, 13631568]`. The compressed delta is uncorrelated with
any length change. **Irreducible under the no-decompile rule** unless abgen-rs
reproduces Unity's exact LZ4 match-finder *and* closes every CAB bit-value diff
first — i.e. it collapses to zero automatically once Areas 8/10 land; there is
no independent size fix.

### D. The 5 "other_size_diff" — not mesh, not stream length

These are object-set differences (objalign matches by path-id, so a renamed or
re-id'd object shows as "only in ours / only in ref"):

- **1 bundle** is a pure path-id *permutation* — same `(class, size)` multiset
  on both sides, just different path-ids (`SkinnedMeshRenderer 344`, etc.).
  Compression noise again; nothing changed length.
- **4 bundles** are a genuine but unrelated structural diff: a **GameObject
  name-length** mismatch. Concrete case in
  `bafkreihp4g3b2…/bafybeiffughg65…_windows` (Δ = +7):

  ```
  ours GameObject "Ennemy_2_Finalized_Mesh"     (size 63)
  ref  GameObject "Ennemy_2_Finalized_Mesh_0"   (size 67)
  ```

  Unity appends a `_0` de-duplication suffix to a node/primitive GameObject
  name that abgen-rs leaves bare. That is a glTF node-naming rule
  (`src/gltf.rs` GameObject construction / mesh-primitive naming), **not** a
  vertex-stream issue, and it touches 4 bundles. Out of scope for Area #4;
  filed here only so it is not re-attributed to mesh length.

## 3. Fix proposal

**For Area #4 (mesh stream length): none required — already byte-exact (0/2014
meshes wrong).** Recommend marking RESEARCH_AREAS #4 *closed — size already
correct; residual is Area 10 (mesh normal/tangent quant) bleeding into LZ4
output size*. The actionable lever for these 205 bundles is Area 10/8, not a
channel-set change.

**Adjacent, separable finding (do not file under #4):** a GameObject-name
`_N` de-duplication suffix is missing on duplicate node/mesh names, costing 4
bundles their byte match. Worth a small targeted drill in `gltf.rs` node-naming
if those 4 are wanted, but it is independent of the mesh-layout code this area
points at.

**Methodological note for the report harness:** `ours_bytes`/`ref_bytes` are
post-LZ4 sizes. Any "size class" triage must decompress and compare
CAB/`.resS` lengths before attributing a delta to a structural cause —
otherwise pure bit-value diffs masquerade as size diffs (as they did for all
200 here).

## 4. Numbers

| Metric | Value |
|---|---|
| glb-wearable bundles total | 660 |
| Mesh objects total | 2014 |
| Mesh objects with a **length** difference | **0** |
| Mesh objects differing in bit-values (same length) | 191 |
| Size-divergent glb-wearable (4<\|Δ\|≤1024) | 205 |
| └ compression-only (uncompressed payload size identical) | 200 |
| └ path-id permutation (same class/size multiset) | 1 |
| └ GameObject name-length diff (the `_0` suffix bug) | 4 |
| Size-divergent caused by mesh stream length | **0** |
| `.resS` byte diffs on the −19 worked example | 0 |
| Uncompressed-payload-size match on 12-bundle Δ∈[−189,+175] sample | 12/12 |

Tools used (all absolute, prebuilt):
`target/release/examples/{objalign,dump_decomp}`. Scan scripts left at
`/tmp/scan_all_mesh.py`, `/tmp/scan_objdiffs.py`, `/tmp/final_census.py`,
`/tmp/ress_check.py`.
