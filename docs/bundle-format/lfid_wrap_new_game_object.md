# The glb importer's PathIDs are deterministic; the animated-wrap recycle segment is "New Game Object"

**Why it matters:** 20 val300 windows pairs (glb-animated + animated glb-wearable
bundles) matched the reference in every byte except the PathIDs of their
hierarchy objects — GameObject, Transform, MeshFilter, MeshRenderer,
MeshCollider, Animation. The standing claim was that these LFIDs come from a
session PRNG ("AddObjectToAsset relabel wall") and could never be derived.
That claim had never been tested for run-to-run determinism.

**The determinism test:** one affected scene entity
(`bafkreicecqnrws3...`, whose `models/ball.glb` bundle is a CAT7 pair) was run
through the actual reference converter (Unity 6000.2.6f2 batchmode,
`abc-deterministic-guids` @ ad0564d) twice — two separate batchmode
invocations, fresh output dirs, content served from a flat local HTTP store,
with a sacrificial warm-up glb entity first in the queue (DCL_Scene gotcha).
All 37 bundles were byte-identical between the runs, **and** byte-identical to
the committed `ad0564d-val300-windows` reference produced months earlier in a
300-entity queue. The glb-import PathIDs are a pure function of content. (The
separate `AssetDatabase.AddObjectToAsset` path used for emote
AnimatorController sub-assets — `docs/walls/addobjecttoasset_pathid_probe.md` — is a
different mechanism and is *not* exonerated by this test.)

**The rule:** with PathID = spooky(guid, LFID, fileType) and LFID =
xxh64(`Type:{type}->{recyclePath}{index}`) already established, brute-forcing
the reference PathIDs over candidate recycle paths inverts every relabeled
object of both probed pairs at index 0 — the only delta versus abgen's model
was the wrap segment: reference paths read
`scenes//New Game Object/...` where abgen wrote `scenes//Scene/...`.

The mechanism is visible in the converter (clean-room-allowed) sources.
`GltfImporter.OnImportAsset` creates the per-scene parent with
`new GameObject(sceneName)`; `GetSceneName` returns the raw glTF `scene.name`,
so an unnamed scene yields a GameObject literally named "New Game Object".
glTFast's `BeginScene` only creates its own `new GameObject(name ?? "Scene")`
child when the scene has != 1 root nodes (`SceneObjectCreation.
WhenMultipleRootNodes`). When animation clips exist, `useFirstChild` is false,
so for a scene with <= 1 root nodes the object serialized as the asset root is
the *importer parent itself* — recycle-keyed while still carrying its
constructor name, before the converter renames the main object to the entity
hash. Hence:

- roots > 1 (glTFast scene GO is the root): wrap segment `"Scene"` /
  scene name — the `empty_scene_name_wrap.md` rule, unchanged.
- roots <= 1 **and** animation present (importer parent is the root): wrap
  segment `"New Game Object"` when the scene is unnamed, scene name otherwise.
- roots == 0 + animation: the outer wrap is the importer parent
  ("New Game Object"); glTFast's inner scene GO stays a `"Scene"` child.

`src/builder.rs` (`build_scene` wrap branch) now picks the wrap segment by
`scene.root_nodes.len() <= 1 && has_anim`; the empty-anim-scene inner GO keeps
its constructor name via a separate `inner_name`. Serialized `m_Name`s are
untouched (the root still serializes as the entity hash — the rename happens
after recycle-ID generation).

Probe tooling: `examples/lfid_brute.rs` (enumerate recycle paths from segments
under a prefix, invert PathIDs). Verified on `models/ball.glb`
(11/11 relabeled PathIDs) and `redTap.glb` (25/25, including a
multi-primitive `redTap_1` child and a `_collider` MeshCollider chain).
