# AnimationClip PathID parity — verified at validation scale

**Why it matters:** Animated glb bundles embed AnimationClip sub-assets whose PathIDs must match Unity exactly for byte-identical output. abgen-rs derives these via the recycle-namespace path (the clip's PathID is computed from the glb guid and a local id for the clip's recycle name). This needed verification across the full validation corpus, not just a couple of hand-checked bundles.

**How it works:** Scanning every animated glb bundle against the reference output, the AnimationClip PathID sets match in all cases, including heavy multi-clip scenes. The recycle-namespace derivation is confirmed correct end-to-end; no code fix is needed.

The lasting caveat is a coverage gap, not a code gap. There is a second, distinct PathID path for emote AnimatorControllers (the md5-seeded sub-clip derivation). It is never exercised at validation scale because the reference corpus contains no emote bundles — the Unity batch converter skipped all emote entities, leaving their reference directories empty. Until the converter produces emote reference bundles, that AnimatorController path remains structurally verified (unit test plus direct reading of the converter source) but unconfirmed against real reference output. The actionable item is to regenerate the reference corpus with emotes included, then re-run the scan.
