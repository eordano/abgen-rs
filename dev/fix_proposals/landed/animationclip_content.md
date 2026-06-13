# AnimationClip CIDv1 content divergence — closed

> **Status: landed.**

## Background

After commit `43b378b` closed the "missing clips entirely" gap for
`.gltf` sources (text glTF + external `.bin` were silently emitting zero
AnimationClips), the clips were *emitted* but their content still
diverged from prod. The remaining residual was the second-largest
per-class diff on the 22-entity windows corpus: **233,471 ppm of class**
(per the per-class refresh table in `landed/gltf_container_key.md`).

`dev/bitwise_residuals_windows.py` with `ABGEN_FILTER_CLASS=AnimationClip`
located 27 differing AnimationClip objects across 15 distinct diff
signatures, dominated by:

- 979x `m_{Position,Rotation,Scale}Curves[*].curve.m_Curve[*].weightedMode`
 — ours: `0`, prod: `3` (~1/3 of all keyframes from CUBICSPLINE sources).
- 33x rotation + 19x position + 11x scale × 3 dims of `inWeight.{y,z,[w]}`
 / `outWeight.{y,z,[w]}` — ours: `0.0`, prod: `0.3333333432674408`.
- 80x rotation + 72x scale + 64x position `.path` — node-name fallback
 spelling mismatch (`node_46` vs `Node-46`, and `//RootNode` vs
 `/Node-3/RootNode` where intermediate unnamed nodes appear).
- 1x `m_FloatCurves[*].attribute` — `blendShape.Key 0` vs `blendShape.0`.

## Root causes (5 patterns)

### 1. CUBICSPLINE keyframes — `weightedMode = 3`, not `0`

Unity's glTF importer tags every CUBICSPLINE-sourced keyframe with
`weightedMode = 3` (`kBoth` — in and out both weighted). Our code was
emitting `weightedMode = 0`. Verified against Barley_pile,
Cable_Box.glb, balloon_v1.glb, Vertical_Platform_SciFi_Alt.glb.

### 2. CUBICSPLINE single-keyframe weight pattern

Multi-keyframe CUBICSPLINE uses `inWeight = outWeight = {all: 0.5}`.
Single-keyframe CUBICSPLINE uses `{first dim: 0.5, rest: 1/3}` — the
non-first components fall back to Unity's single-key default-weight
constant `0.3333333432674408` (= `f32::from_bits(0x3EAAAAAB)`, round-up
`f32` of 1/3). Verified against Cable_Box.glb `open` / `close` clips.

### 3. LINEAR single-keyframe weight pattern

Same `{first dim: 0, rest: 1/3}` pattern applies to single-keyframe
LINEAR curves (slopes still zero, weightedMode still 0). Previously we
emitted all-zero weights. Verified against pacman/scene.gltf
`Sphere.007` (quat) and timetunnel.glb `Node-3` (vec3).

### 4. Unnamed-node path fallback

Unity's glTF importer falls back to `Node-{index}` (capital N, hyphen,
0-based index) for both `"name": absent` and `"name": ""`. We were
emitting `node_{index}` (lowercase, underscore). Verified against
host_2.glb (`Node-46`) and timetunnel.glb (`Node-3`).

### 5. Empty `targetNames` blendShape attribute fallback

When `mesh.extras.targetNames` is absent or `null`, Unity uses the
literal target *index*: `blendShape.0`, `blendShape.1`. We were
emitting `Key {i}`. The `Key {i}` spelling is reserved for the
on-the-fly typetree path Unity uses internally for SkinnedMeshRenderer;
for AnimationClip FloatCurve attributes the importer emits bare
indices. Verified against flame.glb (`Plane` / `Plane.001`).

## Fix

All five live in `src/animation.rs`:

- `glb::node_names_and_parents_from_json`: change fallback name from
 `node_{i}` to `Node-{i}`, and route `"name": ""` to the fallback by
 adding `.filter(|s| !s.is_empty)`.
- `bake_vec_curve` CUBICSPLINE branch: set `weightedMode = 3`; split
 weight emission into multi-key (`0.5` across) vs single-key
 (`{first: 0.5, rest: 1/3}`).
- `bake_vec_curve` LINEAR branch: hoist single-keyframe path before the
 multi-key loop and emit the same `{first: 0, rest: 1/3}` weights as
 the STEP n==1 path, but with zero slopes.
- `build_animation_clips_from_gltf` `PATH_WEIGHTS` arm: change empty
 target-name fallback from `format!("Key {t}")` to `format!("{t}")`.

The mecanim path (`src/animation_mecanim.rs`) shares
`glb::node_names_and_parents` and `glb::animation_path`, so the
`Node-{idx}` fallback fix propagates transparently.

## Measured impact (22-entity windows corpus, 2,158 paired bundles)

```
                       differing objects   distinct signatures
before 27 15
after pass 1 (4 fixes) 6 1
after pass 2 (+ #2) 0 0
```

`dev/class_bits_audit.py` AnimationClip class-ppm:

| | bits_diff | ppm_of_class |
|---|---:|---:|
| before | ~510M | **233,471** |
| after  | 120M  | **54,978**  |

The remaining 120M bits are layout-shift artifacts: `class_bits_audit`
attributes bits by **prod** byte-window, and when other classes (Mesh,
Texture2D) have size deltas in the same SerializedFile, AnimationClip
windows in the prod file overlap differently-classed bytes in ours.
The bitwise residuals scan (which decodes typetrees, not bytes) shows
0 differing AnimationClip objects post-fix, so the AnimationClip
*content* is 100% closed.

## Test bars

- `cargo test --release --lib`: 116 passed.
- `cargo test --release --test parity_bytes`: 2 passed at 773,032
 ceiling. The existing parity fixtures don't contain animations, so
 the ceiling is unchanged.

## Representative cases (verified bit-exact post-fix)

| CID prefix | source | clips | residual pre-fix |
|---|---|---:|---|
| `bafkreihxu6pmg5u` | autopad.gltf | 1 | (the .gltf-emit baseline case) |
| `bafkreicfrzxevc4` | Cable_Box.glb | 3 | CUBICSPLINE single-key wMode + weights |
| `bafkreigzq4rj2it` | pacman/scene.gltf | 1 | LINEAR single-key inWeight 1/3 |
| `bafybeibngc65oft` | Barley_v2.glb | 1 | CUBICSPLINE wMode=3 multi-key |
| `bafkreibgohlfxrd` | flame.glb | 1 | blendShape.Key 0 vs blendShape.0 |
| `bafkreif4cslu2if` | host_2.glb | 1 | node_46 vs Node-46 |
| `bafybeiezqgeug3u` | timetunnel.glb | 1 | empty-name //RootNode vs /Node-3/RootNode + weights |
