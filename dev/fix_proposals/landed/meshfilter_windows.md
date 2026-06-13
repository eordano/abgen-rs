# MeshFilter — windows test set (pathid_rt_v10_windows, 280 bundles)

> **Status: landed.** The two upstream rules this proposal
> identified — derive `scene_name` from `gltf.scenes[idx].name` and force
> wrap for `.gltf` text sources — landed in commit `13a3f4d`, closing
> MeshFilter/MeshRenderer/SkinnedMeshRenderer/MeshCollider to 0 ppm on
> windows + mac. AnimationClip emission for `.gltf` sources landed in
> commit `43b378b`, closing a further 7.48 Mbit per platform.

## TL;DR

**Windows is bit-for-bit symmetric to mac (and to linux) at the MeshFilter
layer.** A fresh measurement on `workdir/pathid_rt_v10_windows` (280 bundles,
1392 MeshFilters paired by PathID on both sides) yields:

- **paired-PID MeshFilter byte-identical rate: 1392 / 1392 = 100 %** (every
 MeshFilter that exists with the same PathID on both sides is byte-exact —
 zero diff in `m_GameObject.m_FileID`, `m_GameObject.m_PathID`,
 `m_Mesh.m_FileID`, or `m_Mesh.m_PathID`).
- **6 unpaired-ours / 6 unpaired-prod MeshFilters on 2 bundles in 1 entity**
 (`bafkreidkmjaata…/basescene.glb`, `bafkreihxu6pmg5…/autopad.gltf`).
- residual: **144 bytes** ≈ **4292 ppm-bytes** of MF byte volume; or
 **2304 bits** ≈ **8547 ppm-bits** (numerator counts the unpaired-extra
 symmetric difference on both sides). These are 100 % accounted for by the
 unpaired-pid MFs above — no in-bundle paired-PID drift.

No change lands in the MeshFilter pipeline itself. The class is closed for
windows; root cause for the residual lives **upstream in the scene/GameObject
recycle-name layer**, and is the same root cause already flagged on mac
(see `meshfilter_mac.md`, commit `d4ad36d`). Concrete recycle-path mismatch
characterised below — implementation lives in a separate proposal so its
PathID-rippling effects on every object in those two bundles can be reviewed
independently.

## Baseline measurement (windows, 280 bundles)

`abgen-rs/dev/meshfilter_windows_forensic.py` + `abgen-rs/dev/mf_windows_bytes.py` +
`abgen-rs/dev/mf_windows_ppmbits.py` (added):

```
MESHFILTER WINDOWS FORENSIC — 280 bundles paired, 0 build err
MeshFilters total (prod side) : 1398
paired MF objects (same PathID both sides) : 1392
 of which byte-identical : 1392 ← 100% paired parity
MeshFilters unpaired (ours-only / prod-only): 6 / 6
Bundles affected : 2
 entity = bafkreihh2mgaqi42kz247dfyme6pg3vgk7aqcdqsijsjnbue4c4yqmp7me
    cid  = bafkreidkmjaata…  (basescene.glb,  babylon.js export, 3 ours-only / 3 prod-only)
    cid  = bafkreihxu6pmg5…  (autopad.gltf, Blender export,    3 ours-only / 3 prod-only)

Per-field breakdown (count of MF objects):
 0x m_GameObject.m_FileID
 0x m_GameObject.m_PathID
 0x m_Mesh.m_FileID
 0x m_Mesh.m_PathID
 6x <unpaired_ours_pid>
 6x <unpaired_prod_pid>

MF bytes diff : 144
MF ppm-bytes : 4292
MF bits-diff : 2,304
MF ppm-bits : 8547.0
```

## End-to-end trace of one offender (autopad.gltf)

Source file `models/AutoPad.gltf` (text gltf, Blender exporter 4.3.47):

```json
"scenes": [{ "name": "Scene", "nodes": [1] }]
"nodes": [
 { "name": "Pad_collider", "mesh": 0 }, // node[0]
 { "name": "Pad.001", "mesh": 1, "children": [0] } // node[1] = scene root
]
```

`scene.root_nodes = [1]`, no animations → in `Builder::build_scene`
(`src/builder.rs:1041`) `wrap = (len != 1) || has_anim = false`. We hit the
non-wrap branch and call `build_node(scene, 1, parent_tr=0,
parent_path="scenes/Scene")`. Inside `build_node`
(`src/builder.rs:1234-1304`):

- `node_name = "Pad.001"` (from gltf `node.name`)
- `node_path = "scenes/Scene/Pad.001"`
- `is_root = (parent_tr == 0) = true`
- `go_name = root_hash = "bafkreihxu6…"` ← line 1292
- Role stored as `Role::Glb("GameObject", "scenes/Scene/Pad.001")`

So our root GO writes:
- `m_Name = "bafkreihxu6…"` (the CID, from `root_hash`)
- PathID derived from recycle `"scenes/Scene/Pad.001"` →
 `−1993837768036086567` (verified via direct
 `local_id_for_recycle_name("GameObject", …) + prefab_packed_path_id` —
 see `examples/probe_pid.rs` in the trace, removed post-confirmation).

The reference (prod, Unity v10) for the same node writes:
- `m_Name = "Pad.001"` (the gltf node name — keeps the node name as the GO
 name even at the scene root)
- PathID derived from recycle `"scenes/Scene/Scene/Pad.001"` →
 `4685315551748918489`. The extra `/Scene` is the wrap layer.

**The reference always emits an extra "wrap" GameObject layer for `.gltf`
source files**, even when the gltf scene has exactly one root node and no
animations. Our code only emits the wrap when `wrap = (len > 1 ||
has_anim)`. For autopad.gltf (the only `.gltf` source file in the
windows pathid_rt_v10 corpus that also has root-node mesh primitives), this
manifests as 3 mismatched MeshFilter PathIDs.

## End-to-end trace of the second offender (basescene.glb)

Source file `basescene.glb` (binary glTF, babylon.js exporter):

```json
"scenes": [{ "nodes": [0], "extensions": {} }] // ← NO "name" field
"nodes": [{ "name": "baseScene", "mesh": 0 }] // 1 node, 3 primitives in mesh
```

Our code hard-codes `let scene_name = "Scene"` (`src/builder.rs:1039`), so
the recycle prefix becomes `"scenes/Scene/baseScene"`. Mesh PathIDs (which
recycle through `"meshes/baseScene"`, independent of `scene_name`) match
exactly between ours and prod. But GO / Transform / MeshFilter PathIDs use
`scene_name` and therefore drift:

| object        | ours recycle                          | prod recycle (matched via brute-force probe) |
|---            |---                                    |---                                          |
| root GO       | `scenes/Scene/baseScene`              | `scenes//baseScene`                          |
| baseScene_1   | `scenes/Scene/baseScene/baseScene_1`  | `scenes//baseScene/baseScene_1`              |
| baseScene_2   | `scenes/Scene/baseScene/baseScene_2`  | `scenes//baseScene/baseScene_2`              |
| MF root       | `scenes/Scene/baseScene/MeshFilter`   | `scenes//baseScene/MeshFilter`               |
| MF baseScene_1| `scenes/Scene/baseScene/baseScene_1/MeshFilter` | `scenes//baseScene/baseScene_1/MeshFilter` |
| MF baseScene_2| `scenes/Scene/baseScene/baseScene_2/MeshFilter` | `scenes//baseScene/baseScene_2/MeshFilter` |

All six prod PathIDs were reproduced bit-exact via the recycle-name → PathID
hash chain (`local_id_for_recycle_name("GameObject"|"MeshFilter",
recycle) → prefab_packed_path_id(guid, lid, FILE_TYPE_META_ASSET)`). The
only difference between ours and prod's recycle path is the literal text:
**"Scene" → empty string when the gltf scene has no `name` field**.

The single basescene.glb is the *only* bundle in the 280-bundle corpus whose
gltf scene is unnamed; all 218 other gltf-backed bundles have
`scene.name = "Scene"` (matching our hard-coded literal).

## Where the actual fix should land

Two upstream rules in `src/builder.rs` are responsible. Both ripple through
every GO/Transform/MF/MR PathID in the affected bundle (because the change
is to the recycle-path prefix), so they should be implemented and validated
as a unified scene-rebuild change, not as MeshFilter-class edits.

### Rule 1 — derive `scene_name` from the gltf `scene.name` field

Currently `src/builder.rs:1039`:

```rust
let scene_name = "Scene";
```

Should be:

```rust
let scene_name = scene.name.as_deref.unwrap_or("");
```

(needs `Scene::name: Option<String>` plumbed through `src/gltf.rs`,
populating from `gltf.scenes[scene_idx].name`.)

Effect:
- 218 bundles with `scene.name = "Scene"`: no change (matches the hard-coded
 literal).
- 1 bundle (basescene.glb) with unnamed scene: switches to empty string,
 closing the 3 unpaired-MF residuals from this bundle and shifting every
 GO/TR/MF/MR PathID in this bundle to match prod.

### Rule 2 — always emit the wrap GameObject for `.gltf` source files

Currently `src/builder.rs:1041`:

```rust
let wrap = scene.root_nodes.len != 1 || has_anim;
```

Should be:

```rust
let is_text_gltf = self.glb_file_ext == "gltf";
let wrap = scene.root_nodes.len != 1 || has_anim || is_text_gltf;
```

Effect:
- All `.glb` bundles (≥277 / 280): no change.
- 1 bundle (autopad.gltf) with single root node + no animations: gets the
 extra `/Scene/` wrap layer in every recycle, closing the 3 unpaired-MF
 residuals from this bundle.

(Caveat: only one `.gltf`-source bundle in the corpus exercises both
`single-root` AND `has-mesh-on-root` AND `no-animations`. The other gltf
in this entity, `Lildoge_NLA.gltf`, has root="Armature" with no mesh — so
the rule's effect on Lildoge's GO/Transform recycle still needs to be
verified once the rule is implemented and full-bundle parity is
re-measured.)

### Why not land it as part of the MeshFilter work

- Both rules change the **root recycle prefix** of every scene object in the
 affected bundle — GO, Transform, MeshFilter, MeshRenderer all shift. The
 diff signature is much broader than "MeshFilter": MF, MR, GO, and TR for
 each affected bundle all change their PathIDs and therefore the prod-vs-
 ours raw bytes of the bundle file.
- The MeshFilter class itself is already at 100% paired-PID parity. There is
 no MF-internal bug to fix.
- The natural home is `gameobject_root_recycle.md` (or `scene_naming.md`),
 with a measurement that validates the *full* bundle ppm-bits closes on
 these 2 bundles, not just the MF slice.

## Reproducibility — added scripts

- `abgen-rs/dev/meshfilter_windows_forensic.py` — paired/unpaired
 enumeration (already existed; ran to completion in this investigation).
- `abgen-rs/dev/mf_windows_bytes.py` — ppm-bytes measurement (mirrors
 the now-removed `mf_mac_bytes.py` from commit `d4ad36d`).
- `abgen-rs/dev/mf_windows_ppmbits.py` — exact ppm-bits + per-field
 breakdown (new for windows).
- `abgen-rs/dev/full_windows_bits.py` — full-bundle ppm-bits + top
 divergent bundles. Used for sizing impact: corpus-wide windows residual
 is ≈ 477 058 ppm-bits and is **dominated by image/texture bundles, not
 the MF-affected pair**.

All three new scripts run under `nix-shell --run "python3 …"` from the
repo root.

## Test bars (unchanged post-investigation, since no Rust edits landed)

```
$ cargo test --release --lib → 107 passed, 0 failed
$ cargo test --release --test parity_bytes → 1 passed
```

## Status )

- **MeshFilter ppm-bits (windows v10, 280 bundles):** 8 547 ppm; 144
 bytes / 2304 bits in 2 bundles (figures from §"Baseline measurement"
 above). MeshFilter class itself is at 100% paired-PID parity
 (1392/1392 byte-exact). Residual is 6 unpaired-pid MFs across 2
 bundles, driven by recycle-name drift upstream.
- **Why still in root:** the proposed Rule 1 (derive `scene_name` from
 glTF `scene.name` field) and Rule 2 (always emit wrap GameObject for
 `.gltf` source files) have NOT landed. Both touch the root recycle
 prefix and would ripple through GO/Transform/MR PathIDs in the 2
 affected bundles.
- **Next concrete step:** plumb `Scene::name: Option<String>` from
 `gltf.rs` through to `src/builder.rs:1039`, swap the hard-coded
 `"Scene"` literal for `scene.name.as_deref.unwrap_or("")`, and
 extend the wrap predicate to `scene.root_nodes.len != 1 || has_anim
  || self.glb_file_ext == "gltf"`. Re-measure with
 `dev/mf_windows_ppmbits.py` and `dev/full_windows_bits.py`; closure
 should take the 2-bundle pair to bit-exact at the MeshFilter layer
 and shift their GO/TR/MR/MF PathIDs en masse to match prod. Test bar:
 `cargo test --release --lib` (107) + `--test parity_bytes` (1) stays
 green (the change only affects the path that derives `scene_name`
 from gltf data; the parity fixtures all have `scene.name = "Scene"`).
