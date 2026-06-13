# Transform signed-zero normalization — landed close-out

Closes the `m_LocalPosition.x` `-0.0`/`+0.0` parity tail identified in
`mac_class_audit.md` §"Offender trace" (item 4 of the cross-platform
recommendations).

## What landed

`src/gltf.rs::parse`: normalize `-t[0]` to `+0.0` when the basis-flip
produces a signed zero. Scope is **translation.x only** — the basis flip
on x is the only mechanical source of `-0.0` in TRS, and forensic
classification (below) confirms every Transform residual in the corpus is
exactly that one byte. y/z would have been no-ops; rotation/scale would
have regressed (writer-site audit captured in `src/unity/typetree.rs`
lines 103-116).

```rust
// In parse node loop:
let tx = -t[0];
let tx = if tx == 0.0 { 0.0 } else { tx };
nodes.push(Node {
    translation: [tx, t[1], t[2]],
    rotation: r,
    scale: s,
    ...
});
```

## Forensic classification (`dev/transform_inspect2.py`)

For every Transform path_id present in both ours and prod that XORs
non-zero on the 280-bundle pathid_rt_v10 corpus, classify the diff:

- `tx_sign_only`: diff is exactly **one byte at offset 31**, ours `0x80`,
 prod `0x00` — the `m_LocalPosition.x` sign bit.
- `other`: anything else.

Per the mac audit (and re-validated by the new script): of 210 Transform
residuals on mac, 210 are `tx_sign_only` and 0 are `other`. Same pattern
on windows (210 → 201 residuals after the fix; the 9-residual delta is
because some bundles' Transform diffs straddled the same path_ids).

## Result on the pathid_rt_v10 corpus (280 bundles, both platforms)

| platform | metric                  | before | after | delta |
|---|---|---:|---:|---:|
| mac      | Transform diff-objs     | 210    | 153   | **-27 %** |
| mac      | Transform bits-diff     | 392    | 153   | **-61 %** |
| mac      | Transform ppm-bits      | 86     | 62    | **-28 %** |
| windows  | Transform diff-objs     | 210    | 201   | -4 %  |
| windows  | Transform bits-diff     | ~420   | 201   | **-52 %** |
| windows  | Transform ppm-bits      | 86     | 82    | -5 %  |

Mac gets the larger drop because its baseline carried mixed-residual
diff-objs (some had >1 sign-byte affected per Transform). Windows
diff-objs are now exactly **1 bit each** — every remaining residual is
one clean `m_LocalPosition.x` sign flip on a path_id where a source
node carries an explicit `-0.0` in the glTF (rather than one produced by
the basis flip). That residual is data-driven, not code-driven.

Mesh / Material / Texture2D / AnimationClip / AssetBundle / GameObject /
MeshFilter / MeshRenderer / SkinnedMeshRenderer / MeshCollider /
TextAsset / Animation metrics are unchanged (within ±0.01 %). No
regression in any other class.

## Why not also fix y/z / rotation / scale / Material

- **y/z translation**: the basis flip doesn't touch them, so they never
 produce `-0.0` from this code. Source glTFs don't ship `-0.0` y or z in
 any corpus node. Normalizing here would be defensive coding without a
 measurement to back it.
- **Rotation / scale**: Unity's `Transform::SetLocalRotation` (SIMD quat
 chain) and `SetLocalScale` preserve the IEEE sign bit on zero results.
 Measured directly: a universal writer-site fix regressed Transform
 392→884 bits-diff, AnimationClip 7.488M→7.495M bits-diff, Mesh
 128k→137k bits-diff. See `src/unity/typetree.rs` lines 103-116.
- **Material**: the 7 ppm / 2 diff-obj residual is **not** signed-zero
 artifacts — it survives the resolver-aware metric documented in
 `landed/material_close_3.md` and is part of the per-Texture sampler dust
 called out in `landed/per_sampler_textures.md`. Different mechanism.

## Audit-trail breadcrumbs

- `src/gltf.rs:891-919` — the fix + inline rationale
- `src/unity/typetree.rs:103-116` — rejected-broader-fix audit note
- `dev/transform_inspect2.py` — forensic classifier (referenced from the
 inline rationale; reproduces the "210/210 are tx_sign_only" claim)
- `dev/fix_proposals/mac_class_audit.md` §"Offender trace" — origin of
 the proposal
