# TextAsset metadata: the `version` literal depends on the converter checkout

**Why it matters:** The `version` field in every `metadata.json` TextAsset was hard-coded to one value. The converter bumped that constant in a newer checkout, so a single ASCII byte in the version string differed for bundles built with the newer converter — and that one byte made every paired TextAsset object mismatch under typetree compare, blocking the entire class from being byte-identical.

**How it works:** The version literal is simply the converter's `VERSION` constant written verbatim into each metadata TextAsset; it changed between converter checkouts. The corpora correlate one-to-one with those checkouts by build target: the legacy Linux corpus was built with the older converter, while the Windows and macOS corpora come from the newer one. Decentraland ships only Windows and macOS clients, so the build target is a reliable proxy for which converter produced a bundle.

abgen-rs reproduces this with a single helper that maps the build target to the correct version string — newer value for Windows/macOS, older value otherwise — derived from the target field already present on both the glb and standalone-texture builders. No new options or wiring are needed, and the Linux corpus stays byte-identical because its target maps back to the original literal.
