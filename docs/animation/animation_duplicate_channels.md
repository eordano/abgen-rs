# Legacy AnimationClip — drop duplicate (node, path) channels

**Why it matters:** A source glTF can contain duplicate animation channels — the same node's translation/rotation/scale targeted more than once. The legacy animation emitter looped over every channel and pushed one curve per channel, so it produced extra redundant curves where the reference produces a single one. That extra-curve count diverged from the reference and broke byte-identical output for any clip with duplicated channels.

**How it works:** the converter's glTF importer keeps exactly one curve per `(node, target path)` pair, taking the first occurrence and dropping later duplicates. abgen-rs now matches this in the legacy emitter (`src/animation.rs::build_animation_clips_from_gltf`): before processing a channel it checks a seen-set of `(node, target path)` and skips channels already recorded, keeping the first in channel order. The Mecanim path already did this dedup; this brings the legacy path in line. The fix only changes output when duplicate channels are present, so it cannot regress the common no-duplicate case.

Note: this was a small, real fix found while investigating a larger animated-glb residual. That broader residual turned out not to be animation-curve work at all — it is dominated by Mesh vertex/normal encoding and BC7 textures, with AnimationClip a rounding-error contributor.
