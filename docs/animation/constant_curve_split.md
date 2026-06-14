# AnimationClip constant-curve split (blocked)

> **SUPERSEDED BY [emote_constant_classification.md](emote_constant_classification.md).** The
> constant-curve classification shipped via `classify_constant`; this "blocked" note predates
> that resolution. Kept for the paper trail.

**Why it matters:** The converter bakes a humanoid AnimationClip into a streamed-curve group plus a constant-curve group. Reproducing that exact partition is needed for byte-identical emote bundles, since the layout of both the streamed binary and the constant data depends on which curves land where.

**How it works:** This is a negative finding. The attempted rule classified a curve as constant when every baked sample was bit-exactly equal to the first sample. It built cleanly and left non-emote fixtures intact, but on emotes it over-extracted: a handful of curves the reference kept streamed were marked constant. Because the two groups are positional, a single misclassified curve shifts every entry after it in both the streamed stream and the constant tail, so the bundle diverged further from the reference rather than closer — the split makes things worse unless it is exact.

The extras are curves whose raw glTF keyframes are all bit-equal (often quaternion w-components at unity) yet the reference kept streamed, which means the converter is not classifying on raw post-axis-conversion samples. Plausible mechanisms (resampling at the bake rate, inspecting tangents, muscle-space projection) cannot be confirmed without the converter's internal source. A second blocker compounds this: even a correct split would also need `m_MuscleClipSize`, a Unity-runtime in-memory struct size that is not derivable from the serialized layout. See `constant_curve_classifier_probe.md` for the probe that traced the rule to humanoid muscle-space. The split stays out until both are resolved.
