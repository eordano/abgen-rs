# `.gltf` vs `.glb` AssetBundle container key

> **Status: landed (commit `9e68d46`).**

## Background

Commit `bc5c9b0` ("windows + mac parity work") claimed in its message
that it threaded `is_gltf` through `Builder::new` and emitted
`<root_hash>.gltf` for `.gltf` source inputs. The actual `src/builder.rs`
diff in that commit only contained `Bc7Profile` plumbing — the container
key stayed hardcoded as `format!("{}.glb", self.root_hash)` regardless
of source extension.

The miss only became visible after the 20-entity edge-case move from
`validation_2` into `workdir/pathid_rt_v10_windows`, which brought in
17 entities carrying `.gltf` text sources. On the original 2-entity test
set, every source was `.glb`, so the fix wasn't measurably needed.

## Fix

3 hunks in `src/builder.rs`:

1. Add `is_gltf: bool` to `Builder`.
2. Thread `is_gltf` through `Builder::new` (parameter after `is_emote`).
3. In `fill_assetbundle`:
   ```rust
 let glb_ext = if self.is_gltf { "gltf" } else { "glb" };
 entries.push(sbp_order::Entry {
       guid: self.glb_guid.clone(),
       key: format!("{}.{}", self.root_hash, glb_ext),
       ...
 });
   ```

Detection at `build_bundle`: `let is_gltf = ext == ".gltf"`. The caller
(`bin/ab-generate`, `bin/ab-build-local`) already computes `ext` from
the filename, so no public API change.

The `.gltf` key also shifts the entry's `m_Container` sort position
(`.glb` and `.gltf` collate differently), which closes downstream
`preloadIndex` / `preloadSize` slot mismatches in bundles where both
forms coexist.

## Per-class refresh data (motivating measurement)

From the per-class refresh measurement on three corpora:

| class | orig 2-ent win (280 bdl) | **new 22-ent win (2,158)** | mac 2-ent (280) |
|---|---:|---:|---:|
| Texture2D           | 15,637 | **267,890** | 15,637 |
| AnimationClip       | 0      | **233,471** | 0      |
| Mesh                | 337    | **240,029** | 337    |
| MeshRenderer        | 0      | 150,634     | 0      |
| Transform           | 62     | 139,387     | 62     |
| GameObject          | 0      | 151,205     | 0      |
| AssetBundle         | 17,653 | 85,665      | 19,092 |
| TextAsset           | 0      | 303,042     | 0      |
| MeshFilter          | 0      | 134,477     | 0      |
| Material            | 7      | 2,238       | 7      |
| MeshCollider        | 0      | 60,831      | 0      |
| SkinnedMeshRenderer | 0      | 11,310      | 0      |
| **TOTAL (paired)**  | **11,651** | **247,121** | **11,654** |

The expansion from 2 → 22 entities exposes classes that were 0 ppm on
the original test set (TextAsset, MeshRenderer, GameObject, MeshFilter,
AnimationClip) — they had no instances to diverge on. The TextAsset
column blow-up (303k ppm) is largely the `metadata.json` dependency-array
shape on scenes/wearables, which has different signatures across the new
corpus's content variety. Closed for CIDv0 entities by commit `9d33fdc`;
remaining CIDv1 TextAsset divergence is a separate follow-up.

## Measured impact of this fix specifically

The `.gltf` container key change is small in ppm (AB delta ≈ 76 ppm on
the new corpus, 10 ppm on the original + mac) because it's one string
per AB object differing by one ASCII byte. The substantive value is
**correctness**: `<cid>.gltf` keys now match the reference byte-for-byte on
`.gltf`-sourced bundles, which closes downstream container-sort
mismatches in bundles where both `.glb` and `.gltf` entries coexist.

## Test bars

- `cargo test --release --lib`: 115 passed.
- `cargo test --release --test parity_bytes`: 2 passed (after the
 ab-expect-hash test was added) at 773,032 ceiling.
- Existing parity fixtures all use `.glb` source, so no fixture
 regeneration needed.

## What's still left (not this fix)

- 16 of 2,174 bundles on the new test set fail to build (`.gltf`
 external-buffer resolution gap; orthogonal to this fix).
- The TextAsset, AnimationClip, Mesh, Texture2D scaling on the new
 corpus is the next investigation surface — these classes barely
 appeared on the original 2-entity test set and dominate the
 expanded ppm.
