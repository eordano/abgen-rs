# Emitting every glTF scene, not just the default one

**Why it matters:** a glTF file can declare multiple named scenes, but abgen-rs
was only emitting the roots of the default scene and silently dropping all the
others. Source files where most geometry lives outside the default scene produced
bundles far smaller than the reference, a large structural deficit that no amount of
per-byte tuning could close.

**How it works:** Unity's editor glTF importer iterates over every scene in the
file and adds each scene's GameObject tree to the bundle. The default scene
becomes the bundle's main asset; each additional scene is added as a named root
GameObject. abgen-rs mirrors this: the parsed `Scene` carries the roots of every
non-default scene, and the builder, after the default scene is fully wired,
emits each extra scene through a dedicated helper. That helper reproduces Unity's
wrapping decision — if an extra scene has a single root and the bundle has no
animation component, the root node is emitted directly with no wrapper GameObject;
otherwise a wrapper GameObject named after the scene parents the roots. The
bundle's root-hash rename fires only for the default scene, so additional scenes'
root nodes keep their source node names.

Single-scene bundles are completely untouched by this path. A small residual
remains on the extra-scene trees from Unity's collider post-process, which
abgen-rs does not yet model on those trees, but it is second-order, not
structural. The unrelated BC7 long-tail deficit on single-scene bundles is a
separate axis tracked elsewhere.
