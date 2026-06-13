# The "other" bundle kind is legacy standalone textures

**Why it matters:** the parity scoreboard classifies bundles by kind so each
cohort's divergence can be attributed and chased. A bucket of bundles was
landing in the catch-all `other` kind, obscuring what was actually diverging and
making it look like an unsolved category when it was not.

**How it works:** every `other` bundle has the identical class signature — one
Texture2D and one AssetBundle, with no TextAsset, GameObject, or Transform. They
are legacy CIDv0 (`Qm…`-prefix) standalone textures. The metadata TextAsset is
suppressed for CIDv0 hashes by `emits_metadata_textasset`, and the verifier's
`standalone-texture` classifier requires a TextAsset, so these fall through to
`other`. The durable fix is purely a classifier relabel (a `standalone-texture-legacy`
kind in `abgen-verify.rs`), which recovers no bytes — it only makes attribution
correct.

This is a negative finding on byte recovery: there is no new builder mechanism to
add for this kind. Its divergence decomposes into three already-owned causes.
Oversize sources beyond the max texture size should emit a mean-color BC7
placeholder; that path was generalized and now fires correctly, collapsing the
worst offenders to near parity. The remaining drift is BC7-compression-envelope
noise on `.resS`-streamed bundles (the streamed raw mip chain is byte-identical;
only the LZ4HC chunking of the bundle envelope differs, and abgen-rs is generally
smaller, not larger) and per-block BC7 partition/mode mismatches on small inline
sources. Both are tracked under existing LZ4HC and BC7 work; the serialization
order of the two objects (Texture2D-first vs AssetBundle-first) varies with
Unity's legacy CIDv0 insertion order and is not a builder choice.
