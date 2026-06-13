# The mislabeled "muscle clip" lead was a missing specular texture

**Why it matters:** A bundle showed a large diff that the bit-diff atlas
attributed to AnimationClip data (a `shift_cascade` tag), which suggested a
muscle-clip / animation problem. Chasing that attribution would have been wasted
effort: the real cause was unrelated to animation, and the misleading tag would
have kept recurring on any bundle with the same underlying defect.

**How it works:** This is a diagnostic note. The atlas tags a diff by looking up
which prod object owns the byte range where the divergence falls. When an object
is missing from our output, every later object in the SerializedFile shifts, so
the diff lands inside whatever object now occupies the prod byte range — here, an
AnimationClip — and gets tagged as that object's fault. The true cause was an
image referenced only through `KHR_materials_specular.specularColorTexture`,
which the material parser ignored, so its Texture2D pair was never emitted and
everything after it shifted.

The durable lesson: a `shift_cascade`-style tag means "something earlier is
missing," not "this object diverged." Trace shift cascades back to the first
missing object rather than trusting the byte-range attribution. The actual fix
landed under the specular-texture work; see `docs/materials/khr_materials_specular.md`.
