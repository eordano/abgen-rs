# Rare-residual forensic: `AnimationClip` (1) + `SkinnedMeshRenderer` (1)

Full-corpus survey (280 paired production bundles via `dev/forensic_rare.py`)
turned up exactly the two singleton residuals predicted by the type-count
table — and both reduce to the **same** source-level feature: **morph targets
(glTF `weights`) / Unity BlendShapes**. Neither is a numeric drift; both are
unconditionally-empty fields that should pull data from the glb.

| Type | Hits | Bundle | PathID |
|---|---:|---|---:|
| `AnimationClip` | 1 | `bafkreifz3xq7pj2a35towrvsq5lnkmiwnr7knmu3erkockjriqfjsiqmt4_linux` | `3248878175312858203` |
| `SkinnedMeshRenderer` | 1 | `bafybeic63qquhbkotau6ecw5gzp22uus2nlqm5nligozbsipq7ideihbrq_linux` | `-6903010554775285873` |

The two bundles are independent (different scenes/wearables). They surface the
two halves of glTF morph-target support:

1. **Animating the weights** → must populate `AnimationClip.m_FloatCurves`.
2. **The initial / static weights** → must populate `SkinnedMeshRenderer.m_BlendShapeWeights`.

Both fixes are surgical and isolated to known emit sites.

---

## Case 1 — `AnimationClip` (cid `bafkreifz...iqmt4`, path `3248878175312858203`, `m_Name = 'Take 001'`)

### Field-level diff

Only one field differs across the entire typetree:

```
.m_FloatCurves [len 0 vs 2]
    ours: []
    prod: [
        {attribute: 'blendShape.target_0', path: 'Nitro', classID: 137,
         script: {m_FileID:0, m_PathID:0}, flags: 0,
         curve: {m_Curve: [12 keyframes], m_PreInfinity:2, m_PostInfinity:2, m_RotationOrder:4}},
        {attribute: 'blendShape.target_1', path: 'Nitro', classID: 137,
         script: {m_FileID:0, m_PathID:0}, flags: 0,
         curve: {m_Curve: [12 keyframes], m_PreInfinity:2, m_PostInfinity:2, m_RotationOrder:4}},
    ]
```

All other clip fields are byte-identical (m_Name, m_Legacy=true,
m_Compressed=false, m_UseHighQualityCurve=true, m_RotationCurves=[],
m_PositionCurves=[], m_ScaleCurves=[], m_EulerCurves=[], m_PPtrCurves=[],
m_SampleRate=60, m_WrapMode=2, m_Bounds zeroed, m_MuscleClipSize=0,
m_ClipBindingConstant empty, m_Events=[]).

The keyframe schema in the prod blob is the **scalar** twin of the
`vec_keyframe` already used for rotation / translation / scale curves — i.e.
`{time:f, value:f, inSlope:f, outSlope:f, weightedMode:0, inWeight:0, outWeight:0}`
where all four scalar fields are plain `f32`, not the `{x,y,z[,w]}` map. Slopes
are finite-difference-derived in exactly the same `LINEAR`-bake style our
existing curve baker uses for rotation/position/scale (verified spot-check:
e.g. between `t=0.0` and `t=0.04166...`, prod `outSlope = 1.3322` is just
`(0.05551 − 0.0) / 0.04167 = 1.3322`, identical to the algorithm in
`animation.rs::bake_vec_curve` with `width=1`).

### Source GLB

```
animations[0]: name='Take 001' 1 channel 1 sampler
 channel[0] sampler=0 target.node=0 target.path='weights'
 sampler[0] interp='LINEAR' input=8 (12 SCALAR floats) output=9 (24 SCALAR floats)

nodes[0]: name='Nitro' mesh=0
meshes[0]: name='0' prims=1 weights=[0.0381..., 0]
 primitives[0]: targets=2 extras={targetNames: ['target_0','target_1']}
```

i.e. one glTF morph-target channel of 12 timestamps × 2 morph targets = 24
output values, interleaved per the glTF spec
(`[w0_t0, w1_t0, w0_t1, w1_t1, …]`). The converter's importer fans this out into
one `FloatCurve` per `(path, targetIndex)` with attribute
`blendShape.<targetName>`, where `<targetName>` is taken from
`mesh.extras.targetNames` (here `target_0`, `target_1` — the literal glTF
extras names, NOT the `blendShape1.*` convention the reference uses).

### Root cause

`abgen-rs/src/animation.rs::build_animation_clips` (line 337+) iterates glTF
animation channels and only handles three `target.path` values:

```rust
if tpath == PATH_ROTATION {... }
else if tpath == PATH_TRANSLATION {... }
else if tpath == PATH_SCALE {... }
// `weights` channel silently dropped
```

Then at line 412 it emits unconditionally:

```rust
"m_FloatCurves" => arr![],
```

`animation_mecanim.rs` likewise lists `m_FloatCurves` in its field table at
line 327 but never populates it for non-humanoid clips.

There is no morph-target support in the animation pipeline at all.

### Proposed fix

1. **De-interleave the output accessor.** When `tpath == "weights"`, the glTF
 spec says the output array has length `count(input) * numMorphTargets`,
 stored target-major within each timestamp. Walk the target node's
 `mesh.primitives[0].targets` to get `numMorphTargets`, and the mesh's
 `extras.targetNames` (with a deterministic fallback to `Key 0`,
 `Key 1`, … if absent — the reference convention for unnamed morphs).
2. **Bake one scalar curve per target.** For each `t in 0..numMorphTargets`,
 extract the slice `values[i*numMorphTargets + t] for i in 0..count(input)`
 and feed it through a `width=1` variant of `bake_vec_curve` — the existing
 `LINEAR` finite-difference slope formula is exactly what the reference produces. The
 output shape per keyframe is the **scalar** variant of the existing keyframe
 shape:

   ```
 { time, value, inSlope, outSlope,
     weightedMode: 0, inWeight: 0.0, outWeight: 0.0 }
   ```

3. **Wrap each curve.** Build one `FloatCurve` entry per target:

   ```
 {
     curve: { m_Curve: [...], m_PreInfinity: 2, m_PostInfinity: 2,
              m_RotationOrder: 4 },
     attribute: format!("blendShape.{}", target_name),
     path: <node_path_from_glb::animation_path>,
     classID: 137,                            // SkinnedMeshRenderer
     script: { m_FileID: 0, m_PathID: 0 },
     flags: 0,
 }
   ```

4. **Wire it in.** Add a `mut float_curves: Vec<Value>` alongside the existing
 `rot/pos/scl` vectors in `build_animation_clips`, push to it from the
 `weights` branch, and swap `"m_FloatCurves" => arr![]` for
 `"m_FloatCurves" => Value::Array(float_curves)` at line 412.

5. **Helper.** Factor a `fn bake_scalar_curve(times, values, interp) -> Vec<Value>`
 from `bake_vec_curve` (or generalise `bake_vec_curve` with `width=1` and a
 bool that emits the scalar keyframe shape). The existing `LINEAR` finite-
 differences and `STEP` infinity-slope logic transfers verbatim — only
 `vec_keyframe` needs a scalar twin that writes raw `f64` instead of an
 `{x,y,z}` map.

6. **No glb::animation_path change needed** — the reference's `path` for the morph
 curve is the same node-path encoding we already produce for the
 T/R/S curves (verified: prod uses `'Nitro'`, which is exactly what
 `glb::animation_path(0, names, parent)` returns for the root mesh node here).

### Side-band sanity for the fix

- `m_ClipBindingConstant.genericBindings` stays empty in prod (this is a
 **legacy** clip, `m_Legacy=true`; generic bindings are a Mecanim concept).
 So no need to touch that field.
- `m_HasMotionFloatCurves` stays `false` (motion curves are a different
 concept: humanoid root motion, not morph weights).
- Mesh blendshape **channels** on the underlying Mesh asset are a separate
 concern from the AnimationClip — and in this AnimationClip-only case the
 mesh ships from the glb correctly enough that Unity links the animation by
 attribute name. So this fix is self-contained to `animation.rs` and does
 not require touching mesh builders.

---

## Case 2 — `SkinnedMeshRenderer` (cid `bafybeic63q...hbrq`, path `-6903010554775285873`)

### Field-level diff

Only one field differs across the entire `SkinnedMeshRenderer` typetree:

```
.m_BlendShapeWeights [len 0 vs 1]
    ours: []
    prod: [1.0]
```

Everything else matches: `m_GameObject`, `m_Mesh PathID = 3836835455412195215`,
`m_Materials` (1 entry, same PathID), `m_Bones` (62 entries, all PathIDs
match), `m_RootBone`, `m_AABB` (zeroed), `m_DirtyAABB=True`,
`m_UpdateWhenOffscreen=True`, all 30+ rendering flags. The only divergence
is the missing weight scalar.

### Source GLB

```
meshes[1]: name='M_uBody_BaseMesh_Mesh.004' prims=1 weights=[1]
    primitives[0]: targets=1
    mesh.extras={targetNames: ['Key 1']}
```

i.e. mesh has one morph target with name `Key 1`, and a glTF
`mesh.weights = [1]` array that specifies the initial / static weight Unity
should apply at instantiation. The skinned-renderer node (`M`, node 63) uses
this mesh through skin 0.

### Root cause

Two cooperating omissions:

1. **`builder.rs::attach_primitive` does not propagate `mesh.weights`.**
 `Primitive::weights` (`scene.rs:20`) is the *skinning* `WEIGHTS_0`
 vertex-attribute array (`Vec<[f64; 4]>`), not glTF `mesh.weights`. The
 morph-target initial weights are never carried through `Scene` →
 `Primitive` at all — `gltf.rs` doesn't extract them.

2. **`mesh_layout.rs::skinned_mesh_renderer_tree` (line 337) does not write
 `m_BlendShapeWeights`.** It writes `m_GameObject`, `m_Mesh`, `m_Materials`,
 `m_Bones`, `m_RootBone`, `m_BoneNameHashes=[]`, `m_RootBoneNameHash=0`,
 `m_AABB={zero}`, `m_DirtyAABB=true`, `m_UpdateWhenOffscreen=true`. The
 base-prototype clone leaves `m_BlendShapeWeights` as whatever default the
 proto carries (here: empty array).

### Proposed fix

1. **Plumb the weights through `Scene`.**
 - In `scene.rs`, add `pub blend_shape_weights: Vec<f32>` to `Primitive`
     (separate name from the existing `weights` to avoid the name clash).
     Alternatively, store on the mesh itself if a richer mesh struct is
     introduced — but the primitive-level field is the minimal-change path.
 - In `gltf.rs::build_scene` (the loop around line 535 that constructs
     `Primitive`), populate it from `glb.json["meshes"][mi]["weights"]`
     (cast each entry to `f32`; default to empty when absent).
 - This is read-only on the glb path; no behaviour change for meshes
     without morph weights.

2. **Pass it into the SMR builder.**
 - In `builder.rs::attach_primitive`, when `becomes_smr`, capture the
     primitive's `blend_shape_weights` and stash it in the `pending_smr`
     tuple (extend the tuple, or refactor to a small struct).
 - At the SMR-finalisation site (line 720 `for (smr_pid, go_pid, mesh_pid,
     mat_pid, skin_idx) in pending { … }`), pass the weights to
     `skinned_mesh_renderer_tree`.

3. **Write the field in `mesh_layout.rs::skinned_mesh_renderer_tree`.**
 Add one line after the `m_AABB` insert:

   ```rust
 t.insert(
       "m_BlendShapeWeights",
       Value::Array(blend_shape_weights.into_iter().map(Value::from).collect()),
 );
   ```

 plus a new `blend_shape_weights: Vec<f32>` parameter. When the slice is
 empty the field becomes `[]` (which is identical to current behaviour for
 meshes without morph targets, so this is a no-op for the other 279
 bundles).

### Why this is a minimal-blast-radius change

- The `m_BlendShapeWeights` field on `SkinnedMeshRenderer` is independent of
 the `m_Shapes` field on `Mesh`. The renderer can carry the initial weight
 even if the mesh's `m_Shapes.channels` is left empty — the
 `m_BlendShapeWeights` field is just a `vector<float>` keyed by index. (Our
 current `m_Shapes` is empty regardless, but UnityPy still loads cleanly
 and bones / vertex skinning are unaffected. Fixing `m_Shapes` is a separate
 semantic gap and is **not** required to close this `m_BlendShapeWeights`
 residual.)
- Meshes without morph targets get `blend_shape_weights = vec![]`, so
 `m_BlendShapeWeights = []`, which is exactly what the base proto already
 produces. Zero risk of regressing the other 279 paired bundles.

---

## Estimated impact

| Type | Before | After fix |
|---|---:|---:|
| `AnimationClip` residuals | 1 | **0** |
| `SkinnedMeshRenderer` residuals | 1 | **0** |
| Combined paired-object-exact contribution | 2 / corpus | **0 / corpus** |

Both residuals are deterministic edge cases triggered by glTF morph targets
(only two bundles in the 280-bundle corpus use them at all — one for the
animation, one for the static weight). The fixes touch four functions:

- `abgen-rs/src/animation.rs::build_animation_clips` (add `weights` branch +
 scalar curve baker)
- `abgen-rs/src/gltf.rs::build_scene` (extract `mesh.weights` into the
 primitive struct)
- `abgen-rs/src/scene.rs::Primitive` (new field)
- `abgen-rs/src/builder.rs::attach_primitive` + `mesh_layout.rs::skinned_mesh_renderer_tree`
 (plumb + emit `m_BlendShapeWeights`)

No other Unity object type touches `m_BlendShapeWeights` or `m_FloatCurves`,
so neither fix can regress any other residual category.

### Deferred to a separate ticket

- Mesh `m_Shapes.{vertices,shapes,channels,fullWeights}` reconstruction
 from `mesh.primitives[].targets` is a separate larger feature (would
 let blendshapes actually deform at runtime). It is **not** required to
 close these two residuals — UnityPy's typetree round-trip is what the
 parity check measures, and the missing `m_Shapes` data does not appear
 in the per-object-byte-exact diff because both prod and ours emit empty
 channels for these two specific bundles (verified in `forensic_rare.py`
 output: `Mesh.m_Shapes.channels len=0` for the SkinnedMesh case).
 File a follow-up ticket when runtime blendshape animation is required;
 the parity-bit count is unchanged either way.

## Reproducing

```bash
cd.
python3 abgen-rs/dev/forensic_rare.py" shell.nix
```

Expected output: `type totals: AnimationClip 1, SkinnedMeshRenderer 1` plus
the two detailed bundle reports printed above. After applying the fix the
type totals line should read `type totals:` (empty).
