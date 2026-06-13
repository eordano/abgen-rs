# Shared-mesh deduplication

**Why it matters:** When two scene nodes reference the same glTF primitive, the converter creates a single shared Mesh object and points every referencing MeshFilter or SkinnedMeshRenderer at that one PathID. abgen-rs was emitting a separate Mesh per node, which produced extra ours-only Mesh objects and MeshFilters whose mesh pointers diverged from the reference.

**How it works:** abgen-rs deduplicates meshes on a key combining the glTF mesh index, primitive index, usage (renderer / skinned / collider), and skin index. Usage and skin are part of the key because nodes that share a primitive but render through different component types, or bind different skins, produce different mesh payloads and must not be merged. On a dedup hit the builder returns the existing PathID immediately and does no further work.

The critical subtlety is the recycle-name counter. A mesh's PathID is a pure function of its recycle name, which comes from a per-prefix counter that advances on each call. The dedup short-circuit must return before touching that counter — otherwise it would advance spuriously and shift the PathIDs of every later distinct mesh, reintroducing the very drift the dedup removes. Emitting the same shared PathID multiple times is safe downstream: the preload table dedups its entries through a set before assembly.
