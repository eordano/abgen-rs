# Mac vs Windows class audit: no mac-only divergence remains

**Why it matters:** abgen-rs targets several platforms, and a per-platform
defect would mean a fix that closes Windows parity could silently leave Mac
diverging (or vice versa). This audit asks whether the Mac target carries any
parity gap that Windows does not, so future effort is scoped correctly.

**How it works (negative finding):** Comparing Mac and Windows output per Unity
object class, across every paired object, shows they are byte-for-byte identical
on every class except one statistical case. All previously mac-specific behaviors
(shader-slot ordering, metadata version, Basic-preset Texture2D) now emit
identical typetrees on both platforms. The audit also re-verified every
target-conditioned site in the code (build-target id, metadata version, CAB
names, externals position, name suffix stripping) and confirmed none falls back
to a Windows default for Mac.

The one platform-sensitive spot is the AssetBundle preload's shader-slot
position, which is a statistical majority rather than a closed-form rule; the
per-target majority is already chosen and there is no exact fix. Every other
residual is cross-platform and identical on both targets: a small set of prod
AnimationClips missing entirely from our output, Texture2D BC7 payload dust, and
signed-zero floats (Unity normalizes `-0.0` to `0.0` on import in transform TRS
and material fields; abgen-rs can do the same with a one-line normalization in the
transform builder or the float writer).

The conclusion is that there is no mac-only work to do — the bulk of the
remaining raw-byte residual lives in SerializedFile bookkeeping that our output
does not yet produce, which is a bundle-structure investigation, not a per-class
platform issue.
