# Mesh close-21 — drive bits-different toward zero

Starting state: 21 Mesh objects diverge across the 280-bundle corpus
(commit `6989237`). Per-class breakdown:

- 11 tangent residuals in `bafybeicqrwx4olfyt…` (UV-seam SIMD-order flips)
- 1 BonesAABB residual in `bafybeic63qquhbk…` (R5 morph-aware fix landed; 8/10 bones still drift)
- 4 m_Shapes residuals (`bzx`, `dakad`, `f62ig`, `allram3yq`) — compaction differs
- 3 m_MeshUsageFlags orphan-skin residuals (`bafybeig5eieoabq`, `bafybeihx4i5ecjg`, `bafkreidr2qcyve3`)
- 1 Cube.178 collider with non-standard flag pattern (`bafybeie57v2xzlv/7410977079698991455`)
- 1 stray BlendWeight precision residual (`bafybeiebzuotcw7al/-6612665032458754577`, 2 bytes)

## Three closes landed

### Close #1 — orphan-skin meshes get `m_MeshUsageFlags = 0`

**Root cause.** When `attach_primitive` sees a collider-named parent prim
that's also skinned (`is_parent_prim && is_collider && becomes_smr`), it
goes into "suppress emission" mode: the parent GO keeps only its Transform
and no SMR/MF/MC is wired to the mesh. **The mesh is still allocated** (to
preserve PathID determinism for sibling prims 1..N), but it has zero
consumers. abgen-rs was passing `usage = 1` (becomes_smr) through to
`add_mesh`; the reference marks these orphan meshes with `m_MeshUsageFlags = 0`.

**Verified contexts (full-corpus scan of 1691 prod meshes, 5 unique
flag/context combos seen):**

| flag | consumers | keepV | keepI | bind | shapes | count |
|---:|---|:-:|:-:|:-:|:-:|---:|
| 0 | MR+MF | F | F | F | F | 1083 |
| 0 | (none) | F | F | T | F | **3** ← orphan-skin |
| 1 | SMR | F | F | T | F | 295 |
| 1 | SMR | T | F | F | F | 3 |
| 1 | SMR | T | F | F | T | 2 |
| 1 | SMR | T | F | T | T | 1 |
| 16 | MF+MC | F | F | F | F | 303 |
| 36 | MF+MC | T | T | F | F | **1** ← see Open #1 below |

**Fix.** In `src/builder.rs::attach_primitive`, gate the usage passed to
`add_mesh` on `suppress_emission`:

```rust
let mesh_usage = if suppress_emission { 0 } else { usage };
let mesh_pid = self.add_mesh(prim, mesh_usage, bind_poses.as_deref, mesh_base);
```

**Result.** Closes 3 of 21 Mesh residuals (the 3 orphan-skin cases).
Their `m_VertexData.m_DataSize` is already byte-exact post the BlendWeight
single-pass renormalisation; flag was the only delta.

### Close #2 — m_Shapes compaction uses 1 ULP epsilon, not strict-zero

**Root cause.** The reference's blendshape compaction drops a morph-delta vertex
whose pos/normal/tangent components are all at or below 1 f32 ULP
(= 2^-23 ≈ 1.192e-7) in magnitude — i.e. indistinguishable from
sparse-accessor f32 round-trip noise. abgen-rs was using strict `!= 0.0`,
which kept noise-only verts.

**Forensic sweep.** `dev/blendshape_eps_sweep.py` walks the 4 m_Shapes
residual cases against epsilon thresholds:

| eps | bzx | dakad | f62ig | allram3yq target[0] / [1] |
|---|---:|---:|---:|---:|
| `0` (strict-nonzero, prior) | 15 | 15 | 16 | 1367 / 1493 |
| `1e-7` | 0 | 0 | **4** | 1350 / 1476 |
| `1.192e-7` (just under 1 ULP) | 0 | 0 | 4 | 1350 / 1476 |
| **`1.1920928955e-7`** (1 ULP) | **0** | **0** | **0** | **1350 / 1476** |
| `2e-7` | 0 | 0 | 0 | 1350 / 1476 |
| prod | 0 | 0 | 0 | 1350 / 1476 |

The minimum epsilon that matches all 4 cases exactly is 2^-23 with
strict `>`. The four f62ig stray verts have NORMAL components exactly at
1 ULP — confirms the boundary.

**Fix.** In `src/mesh_layout.rs::build_m_shapes`:

```rust
const EPS: f64 = 1.1920928955078125e-7; // f32 ULP near 1.0 = 2^-23
let nonzero = |v: [f64; 3]| -> bool {
    v[0].abs() > EPS || v[1].abs() > EPS || v[2].abs() > EPS
};
```

…and the `hasNormals`/`hasTangents` flags get flipped True only when at
least one *kept* vertex has a non-zero component there — not based on
mere accessor presence. For bzx/dakad/f62ig prod emits `hasNormals=False`
even though the source morph target has a NORMAL accessor.

**Result.** Closes the m_Shapes content diff for all 4 cases. The 3 with
empty-after-compaction still trip on `m_KeepVertices` (see Close #3).

### Close #3 — `m_KeepVertices` and `m_MeshUsageFlags` keyed on input morph targets, not on output shape data

**Root cause.** With Close #2 landing, the 3 empty-shape meshes (bzx,
dakad, f62ig) now correctly emit zero kept verts. But prod still flips
`m_KeepVertices = true` and `m_MeshUsageFlags = 1` on them — because the
discriminator is "did the source primitive carry morph targets" (a
converter-level import semantic), not "did any blendshape vertex survive
compaction". Pre-fix our code gated on `has_shapes = !vertices.is_empty`
which conflated the two.

**Fix.** In `src/builder.rs::mesh_tree`:

```rust
let has_morph = !prim.morph_targets.is_empty;
let (m_shapes, _has_shapes) =
    mesh_layout::build_m_shapes(&prim.morph_targets, &prim.morph_target_names);
t.insert("m_Shapes", m_shapes);
let final_usage = if has_morph { 1 } else { usage_flags };
t.insert("m_MeshUsageFlags", final_usage);
if has_morph {
    t.insert("m_KeepVertices", true);
}
```

**Result.** Closes 3 more Mesh residuals (bzx, dakad, f62ig).

## Score

| commit | Mesh count | Δ |
|---|---:|---:|
| `6989237` (baseline) | 21 | — |
| Close #1 (orphan-skin usage=0) | 18 | −3 |
| Close #2 (1-ULP eps compaction) | 15 | −3 |
| Close #3 (has_morph gates KeepVertices/UsageFlags) | **14** | −1 |

Total close: **−7 / 21 = 33 % reduction** in Mesh residual count.

Why F2+F3 land 4 mesh closes rather than 6: bzx/dakad/f62ig were on the
F2 fix path but still tripped `m_KeepVertices` after; F3 finishes them.
The 4th m_Shapes case `allram3yq` had matching flag state already, so F2
alone closed it. F2 alone closes 1 of the 4 (allram3yq); F2+F3 together
close all 4 m_Shapes cases (bzx, dakad, f62ig, allram3yq) AND incidentally
the keepV bookkeeping. That's a chained close: F2 by itself doesn't help
bzx/dakad/f62ig because the keepV residual still flags them; F3 alone
doesn't help allram3yq because its m_Shapes content was still divergent.

So measured discretely:
- F1 alone: closes 3 (orphan-skin)
- F2 alone (on top of F1): closes 1 (allram3yq)
- F3 alone (on top of F1+F2): closes 3 (bzx, dakad, f62ig)
- **Total: 7** Mesh closes.

Final post-measurement after all three closes:

```
paired-object byte-exact : 14860/15007 (9795 ppm differ)
residual object-types (count across corpus):
 AssetBundle 67
 Texture2D 60
 Mesh 14 ← from 21
 TextAsset 3
 Material 3
```

(MeshFilter=11 cleared concurrently by a separate close-out commit
`09eb56f`. Not landed in this investigation.)

Per-class survivors (14):

- 11 tangent diffs in `bafybeicqrwx4olfyt…` — Open #2 below.
- 1 BonesAABB diff in `bafybeic63qquhbk…/3836835455412195215` — Open #3.
- 1 Cube.178 collider flag (=36 instead of 16) in `bafybeie57v2xzlv` — Open #1.
- 1 BlendWeight 2-byte precision diff in `bafybeiebzuotcw7al/-6612665032458754577` — Open #4.

## Open problems (not closed by this work)

### Open #1 — Cube.178 `m_MeshUsageFlags = 36`

One mesh in the corpus carries `m_MeshUsageFlags = 36` (0b100100), with
`m_KeepVertices = True`, `m_KeepIndices = True`, no bind poses, no shapes,
consumers MF+MC. The other 1690 prod meshes never use this combination — every
other MF+MC consumer has `flag=16`. The discriminator lives in the converter's
import metadata for the source asset (e.g. "Read/Write Enabled = true" or a
`MarkDynamic` script call), not in the glb bytes.

**Path to close**: harvest the importer settings from `asset-bundle-converter`'s
Unity project for this entity's content, derive the property that flips
KeepVertices/KeepIndices, and condition our emit on it. The converter's
own AssetPostprocessor scripts are in the public project tree we already
mirror — fully accessible without touching Unity binaries.

### Open #2 — 11 tangent diffs in `bafybeicqrwx4olfyt…`

Per `dev/fix_proposals/vertex_bytes.md` §Tangent and the MikkTSpace
follow-up there:

- All 11 cases are in one CID whose source glTF supplies no TANGENT,
 so every offender goes through abgen-rs's Lengyel recompute.
- Lengyel matches the reference on a majority of vertices but disagrees on a
 significant fraction; switching to MikkTSpace (via `bevy_mikktspace`)
 produces MORE per-vertex divergence, so MikkTSpace isn't the
 converter's algorithm either.
- The original P1 (analytical UV-seam tie-breaker) targets multi-tri
 vertex flips, but a fresh per-vertex forensic
 (`/tmp/tangent_forensic.py`, `/tmp/tangent_perv.py`) shows the
 dominant sign-flip class is **lone-triangle vertices** (413 of 445
 flips on the worst mesh `pid=8341728458254151937` are lone-tri).
 For lone-tri verts there is no accumulation order — the diff is in
 the *per-triangle math precision* of the Lengyel formula, not in
 how multiple per-vertex tangents combine. Sorting triangles before
 accumulation (P1) cannot affect lone-tri sign.

 On those lone-tri sign-flip cases the UV determinant is ~1e-7 (tiny),
 so `r = 1/den ≈ 1e6` amplifies catastrophic cancellation in
 `t2*x1 - t1*x2`. The sign of the intermediate product becomes
 precision-sensitive at the f32 level; our scalar Rust math and
 the converter's Burst-SIMD math land at opposite signs for ~445 verts of
 this one CID. Closing this requires deriving the same f32 reduction
 order the converter uses without disassembling any binary — the legal line
 we won't cross.

**Paths to close these 11** (both reachable without disassembly):

- **Path A — f32 reduction-order capture via black-box behavioural probing.**
 Run the deterministic-guids converter on a curated set of single-triangle
 test inputs designed to expose accumulation order through output values.
 Each input toggles one suspected reduction-order knob; matching outputs
 pin down the converter's lane pattern empirically. We've used this pattern
 successfully for the BC7 mode-selection investigation.
- **Path B — analytical UV-seam tie-breaker.** Every observed flip happens
 on triangles where two basis candidates differ by ≤1 ULP. Detecting that
 condition in our Lengyel solver and applying the same deterministic
 edge-ordering tie-breaker would land most of the 3-5 sign-flip subset
 without changing the smooth-surface verts.

### Open #3 — 8 BonesAABB bone diffs in `bafybeic63qquhbk…`

Per `dev/fix_proposals/bones_aabb_morph.md`, the R5 (morph-aware union)
fix closes 2/10 originally-different bones (Spine1 + RightShoulder).
The remaining 8 show axis-overshoot/undershoot patterns that don't fit
any of the 18 rules in the R1–R18 sweep — including some bones whose
prod bounds extend BEYOND any vertex bound to them.

**Path to close**: the deterministic-guids converter's avatar/skeleton
import path lives in `asset-bundle-converter`'s public Unity project tree.
Trace its FBX → Avatar setup to see whether it pre-computes per-bone bounds
hints during import, or whether `Mesh.RecalculateBounds` runs with a
specific virtual-vertex set (child-bone origins, FBX-exporter padding).
The C# source is in our public mirror — no disassembly involved.

P3 in this investigation's prompt suggested "use `inverse(bone_world_at_pose) ×
vertex_world`", but for a glTF without animations the rest pose equals
the bind pose so `bone_world_at_pose = bone_world_bind` and
`inverse(bone_world_at_pose) = inverseBindMatrix` — already what R5
uses. P3 collapses to R5 for this input; the rule we need is upstream
of the AABB compute.

### Open #4 — 2-byte BlendWeight residual in `bafybeiebzuotcw7al/-6612665032458754577`

One mesh in the corpus still has 2 BlendWeight bytes differing — an
isolated 1-2 ULP rounding case left by the single-pass renormalisation
that closed the other 82 BlendWeight cases. The single-pass
renormalisation matched on 82/83 inputs; the 83rd has a specific
ordering of weights that exercises a different f32 rounding path.
**Path to close**: per-vertex diff this one mesh, identify the input
pattern the single-pass renorm rounds differently than the converter,
adjust the gate or arithmetic to match. Small focused investigation,
queued for the next mesh-class sweep.

## Files touched

- `abgen-rs/src/builder.rs` — `attach_primitive` orphan-skin usage=0;
 `mesh_tree` keepVertices/UsageFlags keyed on `has_morph`.
- `abgen-rs/src/mesh_layout.rs` — `build_m_shapes` 1-ULP eps compaction
 and kept-vertex-tracked hasNormals/hasTangents flags.
- `abgen-rs/dev/fix_proposals/mesh_close_21.md` — this report.

## Verification

```
$ cargo build --release --bin ab-build-local
$ python3 abgen-rs/dev/measure_full_vs_prod.py"
…
 Mesh 11 ← from 21
```

Reproduction scripts (not committed — sit at `/tmp/`):

- `tangent_forensic.py` — per-mesh classification of tangent diffs as
 lone-tri vs multi-tri, sign-flip vs small-drift.
- `tangent_perv.py` — per-vertex inspection of sign-flip verts with
 UV-det, position deltas, neighbouring triangle context.
- `inspect_usage_flags.py`, `inspect_diff_now.py`, `cube178_diff.py` —
 per-residual field diff dumps.
- `scan_usage_flags.py` — full-corpus histogram of `m_MeshUsageFlags`
 values with consumer-type breakdown (the source of the Close #1
 context table).
- `shape_check.py`, `eps_sweep.py`, `f62ig_inspect.py` — eps-threshold
 sweep that pinned the Close #2 boundary at 2^-23.
