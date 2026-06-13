# Parity findings from a parallel investigation wave

**Why it matters:** Several distinct categories were diverging from the reference's bundle output, and it was unclear which were real abgen-rs gaps versus measurement artifacts. Mis-attributing a divergence wastes effort fixing the wrong layer, and some categories cascade — a single small field error shifts every downstream object's stream offset, inflating the apparent diff far beyond its true cause.

**How it works:** This note records durable diagnostic conclusions so the same probes need not be re-run.

- **Mesh AABB is one ULP off when an axis is degenerate.** When a mesh axis has `min == max`, the reference's bounds center and extent land one ULP away from the naive f32 `(min+max)/2` / `(max-min)/2`. The converter does not compute bounds from the source f32 positions; it reads back the encoded (f16/snorm) vertex stream, decodes to f32, and derives bounds from the decoded values, which introduces a rounding bias. Reproducing the decode path bit-exactly is what closes this tail.

- **Mecanim splits constant curves out of the streamed clip.** Curves whose samples are all equal (within tolerance) are partitioned into a constant sub-clip, leaving only non-constant curves streamed. The serialized blob size must then be reported in the clip's muscle-clip-size field. abgen-rs must classify each curve, emit the streamed and constant sub-streams in the same value-array order, and compute the size after serialization.

- **Negative finding — normal-map channel reshuffle is already implemented.** The BC7 normal-map repack runs on the GLB-embedded path; the standalone path correctly skips it because standalone residuals are all sRGB and carry no normal maps. Remaining residuals there are encoder mode-selection drift, a different root cause.

- **Negative finding — animator name-table content is correct; ordering is the blocker.** The name hash is CRC32 over the UTF-8 name bytes and matches the reference exactly, and the set of pairs is identical. What diverges is the iteration order of the serialized pairs, which is governed by an internal Unity hashtable variant that cannot be reproduced without Unity source.

- **Negative finding — standalone-texture inline-vs-streamed was a harness artifact.** The builder emits the streamed-residual form correctly when told the texture is model-referenced; the diff came from the corpus-build harness's model-referenced detection missing some entities, not from abgen-rs.

A process lesson also stands: parallel agents working on shared git objects must operate strictly through their own worktree, never via absolute paths into the main checkout, or their writes collide.
