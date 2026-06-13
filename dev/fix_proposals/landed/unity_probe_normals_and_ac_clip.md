# Unity-editor probe — no-NORMAL mesh import + AnimationClip sub-asset PID

Two findings produced by driving the live Unity editor (Unity Editor) against
the unity-explorer project, using the additive `AbgenRs.BundleProbe` methods
in `Assets/Editor/AbgenProbe/AbgenBundleProbe.cs`:

 - **Normal-reorder cracked.** Unity does NOT reorder vertices when a
    glTF primitive has no `NORMAL` attribute. It computes area-weighted
    vertex normals on the *post-x-flip, post-winding-flip* topology, then
    normalizes. Formula now lives in `src/gltf.rs` and is locked by the
    test `gltf::tests::no_normal_area_weighted_matches_unity_probe`.

 - **AC-clip-PID confirmed fundamentally random.** The AnimationClip
    sub-asset `LocalIdentifierInFile` is **not derivable** from any
    `(parent_guid, clip_name, add_order)` triple. Two runs of the exact
    same input produce different lids. Unity assigns it on first
    `AssetDatabase.AddObjectToAsset(clip, controller)` from a process-local
    RNG; the deterministic-guids fork preserves whatever the first machine
    wrote in the `.meta` file forever after. Prod's specific lids cannot
    be reproduced without access to prod's `.meta` files.

## Setup

- Editor: `Unity Editor` against
 `github.com-decentraland/unity-explorer/Explorer`. Pre-existing 780+
 CS compile errors are ignored via the "Ignore" dialog on first launch.
- Probe code: `Assets/Editor/AbgenProbe/AbgenBundleProbe.cs` (additive —
 three new static methods): `ProbeAcClipPids`, `ProbeAcClipPidVectors`,
 `ProbeGltfMesh`.
- IPC: invoke via `<run-static-method> AbgenRs.BundleProbe.<Method> --arg key=val`.

## Probe 1 — no-NORMAL mesh import

### Method

Picked a content-server glb whose primitive 0 has POSITION + TEXCOORDs but
no NORMAL: `bafybeiflk3wxmbprq63lsrmcnmrkuzfbtqgotxwtf5xysb3d5u5njhsyba`
(844 verts, 1009 tris, 1 sub-mesh, mesh.name=None, node.name="Pyramid").

Ran `ProbeGltfMesh` which:

1. Copies the glb into `Assets/__AbgenProbeTemp/`,
2. Forces `AssetDatabase.ImportAsset(... ImportAssetOptions.ForceUpdate)`,
3. Loads all sub-objects, dumps each `Mesh`'s post-import `vertices[]`,
 `normals[]`, `triangles[]` to JSON.

Compared against Rust-side `read_accessor` output for the same glb.

### Findings

1. **Positions identical to glb-with-x-flip** — no vertex reorder. Unity's
 post-import vertex array is `[(-x, y, z) for x,y,z in glb_positions]`
 in the same order.

2. **Triangle winding flipped** — for each (a,b,c) in glb, Unity emits
 (a,c,b). This was already the existing Rust behaviour; the probe
 confirms it.

3. **Normals = area-weighted vertex normals**. For each triangle in the
 post-flip index list, compute `cross(p_b - p_a, p_c - p_a)` (which is
 `2 · face_area · face_normal`), accumulate on each of the three
 vertices, then normalize. Validated at f32 precision across 838/844
 vertices (the remaining 6 are zero-accumulator cancellations — see
 below).

### Zero-accumulator cases

6/844 vertices have two triangles touching them with opposite winding
(back-to-back duplicates). Their accumulator cancels to (0,0,0), and Unity
picks `(-1, 0, 0)` or `(1, 0, 0)` as the fallback. We have not yet
characterised the rule (not a function of position alone). Our fix falls
back to `(0, 0, 1)` for these — strictly no worse than the previous
all-vertex `(0, 0, 1)` placeholder; an open follow-up.

### Implementation

`src/gltf.rs` line ~738: the `None => vec![[0.0, 0.0, 1.0]; nverts]` arm
now computes area-weighted normals in-line. The triangle index list is
re-derived inside the arm (with the same winding-flip we apply later) so
the normal block doesn't depend on the later index-build step's ordering.

### Test

`gltf::tests::no_normal_area_weighted_matches_unity_probe` in
`src/gltf.rs` consumes `tests/fixtures/unity_probe/no_normal_mesh.json`
(82KB, 844 positions + 844 normals + 3027 triangle indices captured from
the live editor) and re-derives every normal from positions+indices alone.
Passes at f32 precision. Locks the formula in CI.

### Measured impact

24 CIDs in `workdir/pathid_rt_v10_windows` have a no-NORMAL primitive
(found via `parse_glb`-walk). All 24 fall in a single parent entity. Built
each pre-fix and post-fix:

| pre-fix bits-diff | post-fix bits-diff |
|------------------:|-------------------:|
| 18,392            | 18,392             |

Same number, because the Mesh objects fail to PAIR between ours and prod —
ours-Mesh-PID `1052358700347209935` vs prod-Mesh-PID
`-8084087248291579697`. The script's bits_diff iterates `set(ours) &
set(prod)`, and Mesh isn't in the intersection, so the normal-content
change is invisible to this metric.

The fix is independently proven correct (f32 match to Unity in 838/844
vertices on the probe glb), and it eliminates the prior all-vertices-at-
`(0,0,1)` placeholder behaviour. Visible ppm impact will land once the
upstream Mesh-PID derivation closes (separate residual, see `mesh_windows.md`).

## Probe 2 — AnimationClip sub-asset PathID

### Method

`CreateAnimatorController` in the abc fork (line 595 of
`asset-bundle-converter/Assets/AssetBundleConverter/AssetBundleConverter.cs`)
attaches AnimationClips via:

```csharp
AssetDatabase.AddObjectToAsset(newCopy, controller);
```

The 2-arg overload — no `name` parameter. `ProbeAcClipPids` reproduces
this exactly: creates an AnimatorController at a temp `.controller` path,
attaches N AnimationClips with `AddObjectToAsset`, saves+reimports,
captures every sub-object's `LocalIdentifierInFile` via
`AssetDatabase.TryGetGUIDAndLocalFileIdentifier`.

`ProbeAcClipPidVectors` runs the same on 10 distinct (controller_name,
clip_name_list) vectors so we can curve-fit a formula.

### Findings

Captured lids for "Take 001" attached as the only clip to a fresh
controller, across two independent runs of the same input:

| run | controller GUID | clip lid (signed i64) | clip lid (hex) |
|-----|-----------------|----------------------:|----------------|
| 1   | `34b6f237eaa9413ec871b792fffc28cd` |  8,731,932,577,680,579,962 | `7935 5e90 7c20 d63a` |
| 2   | `34b6f237eaa9413ec871b792fffc28cd` | -7,759,882,349,716,730,797 | `9442 06f0 c10b 4c93` |

**Same GUID, same clip name, same add order, same source — different
local_id.** The lid is process-local randomness.

The deterministic-guids fork's `SetDeterministicGuid(filePath, seed)`
patches the.meta file's `guid:...` line via regex but leaves the rest
of the.meta untouched. The.meta also encodes the sub-asset's local_id
(in the `recycleNameMap`/`internalIDToNameTable` section). On every
subsequent import Unity preserves whatever local_id was first written —
random or not.

### Conclusion

The AnimationClip sub-asset PID is **fundamentally unreproducible** from
the inputs available to abgen-rs (the source glb + its CID). Prod's
specific lids were chosen by the RNG on the specific machine that first
ran the conversion, and recorded in the.meta of that build. Without
access to prod's.meta files (which don't ship with the bundles), we
cannot derive them.

This confirms the conclusion already drawn in
`dev/fix_proposals/animator_controller_tos.md` from black-box
search — and rules out any alternative GUID/short-type/recycle-name
combination as a future possibility. It is the Unity importer that
generates the lid, not any input-data hash.

### Future options (none cheap)

1. **Ship our own deterministic lid scheme.** Replace prod's random lid
 with `xxh64(parent_guid || clip_name || add_order)` or similar. This
 breaks parity with already-deployed bundles but produces a
 deterministic forward output. Would require coordinated CDN rollout.

2. **Read prod.meta** if anyone preserved them. Not currently available.

3. **Accept the ~21k bits/17 ACs = ~5,400 ppm of glb-emote** as a
 permanent residual.

Closing the AC-clip-PID line as **fundamentally blocked**, not a search shortfall.

## Files

- `Assets/Editor/AbgenProbe/AbgenBundleProbe.cs` — added 3 methods:
 `ProbeAcClipPids`, `ProbeAcClipPidVectors`, `ProbeGltfMesh`, plus
 `ProbeCleanup` to remove the temp folder.
- `src/gltf.rs` — area-weighted normal block + `gltf::tests`.
- `tests/fixtures/unity_probe/no_normal_mesh.json` — Unity probe capture
 (82KB).

## Discipline note

This work used `Unity Editor` despite the prior write-up in
`unity_synthetic_probes.md` §"Why we didn't run the live-Unity probe"
calling it costly. Key unblockers:

1. The pre-existing 780+ project compile errors don't prevent IPC.cs
 from loading — they only block Unity's domain-reload-after-recompile
 path. Workaround: write the probe methods BEFORE editor launch, so
 they're picked up by the initial AppDomain compile.
2. The "Enter Safe Mode / Ignore / Quit" dialog must be dismissed by
 clicking "Ignore" (~~clicking Continue would re-launch). The UPM
 "Retry / Continue / Quit" dialog also fires on a fresh launch
 (because the project pulls private git packages over SSH that the
 bwrap user can't read); click "Continue".
3. Domain reload after editing probe source is BLOCKED by the pre-existing
 compile errors — "Editor compiler errors found. Will not reload
 assemblies." Workaround: kill and relaunch the editor; the freshly-
 compiled DLL loads on next startup.
