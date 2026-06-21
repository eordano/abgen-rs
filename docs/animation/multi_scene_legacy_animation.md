# Per-scene legacy Animation component on multi-scene glbs

Some glbs declare more than one glTF scene, e.g.

```
"scenes": [
  { "name": "Scene",     "nodes": [0] },
  { "name": "Scene.001", "nodes": [1] }
]
```

The converter instantiates **every** scene in the file, not just the
default one. For each instantiated scene it calls the instantiator's
`AddAnimation` once with the import-global clip array. When the clips are
legacy (the scene `AnimationMethod`), `AddAnimation` adds an `Animation`
component to that scene instance's root GameObject and registers all the
import's clips on it.

The clip array is import-wide, so the result is one legacy `Animation`
component per glTF scene, each carrying the same clip list — even for a
scene whose own nodes are not animated. In the example above the single
animation targets node 0 (under `Scene`), yet `Scene.001` still receives
its own `Animation` component.

abgen builds the extra scenes in `build_extra_scene`. Each extra scene
that needs a wrapper GameObject (the SceneTransform) now receives an
`Animation` component on that wrapper whenever the glb has legacy
animation clips, mirroring the per-scene `AddAnimation` call. The
component's PathID derives from the wrapper's recycle path
`scenes/<scene>/<scene>/Animation`, the same scheme used for the main
scene's Animation.

The `roots.is_empty()` branch (empty animated extra scene) already
emitted an inner Animation host; this rule covers the normal case where
the extra scene has nodes.
