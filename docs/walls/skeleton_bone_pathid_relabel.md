# Skeleton bone PathIDs on skin+animation bundles — not clean-room derivable

**Why it matters:** Bundles that combine a glTF `skin` and a glTF `animation`
emit GameObject/Transform/SkinnedMeshRenderer objects whose
`localIdentifierInFile` (PathID) values differ from the reference's. The object trees,
names, and hierarchy are byte-identical; only the PathIDs differ, and that
cascades through bone arrays, parent/child links, the avatar table-of-skeleton,
the AssetBundle container, and the preload table — so these bundles cannot go
byte-identical.

**How it works:** This is a negative finding. The recycle-name PathID model that
is byte-exact for static meshes — and for skinned meshes *without* animation,
and for bone hierarchies *without* a skin — does not reproduce these bundles for
any recycle string, type, occurrence index, or hash family. The single
discriminator, verified across the corpus, is that a bundle fails exactly when
it has both a skin and an animation. Everything else was ruled out: the
deterministic GUID matches (the asset-hash bytes packed into the PathID are
identical on both sides), the node hierarchy and names match including
duplicate-sibling uniquification, and brute force over recycle strings, raw
sequential integers, type codes, and hash families finds zero matches. The
remaining generator is the `AddObjectToAsset` fallback in the converter's
toolchain, which is proven
nondeterministic across runs of the same project — it is instance-id /
editor-session-state driven, not a pure function of (guid, name, type). The
skin+animation path routes the prefab subtree through that fallback, so the
PathIDs are simply not a pure function of any content the converter can see.
This is the same class of wall as the preload-table ordering residual: a value
the converter emits that is content-indistinguishable from the clean-room side.

A separate skeleton-only-extraction behavior (the converter stripping the armature
wrapper and mesh from an animation-only, skin-less glb) is a different bug,
unrelated to the PathID question.
