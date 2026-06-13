# Emote AnimationClip PathID derivation (blocked)

**Why it matters:** For emote bundles built through the mecanim path, our emitted AnimationClip has the right name, type, and parent Animator/AnimatorController PathIDs, but a different PathID than prod. Because the PathID drives object identity, this single mismatch makes the whole clip object diff against prod. This is a negative finding: the correct derivation is not recoverable without information abgen-rs cannot currently access.

**How it works (and why it is blocked):** The converter, on the emote path, instantiates each clip and calls `AssetDatabase.AddObjectToAsset(clip, controller)` to attach it to the generated `.controller` asset. That call asks Unity itself to assign the clip a local file ID inside the controller. Unity's algorithm for assigning local file IDs to named sub-assets is internal and unspecified — an undocumented hash of the object's name, class, and parent — and the resulting local file ID is what then feeds the standard `prefab_packed_path_id(parent_guid, lfid, file_type)` construction.

The shape of the final hash is understood (the PathID is unambiguously produced by `prefab_packed_path_id`), but the input triple is the unknown. Exhaustive brute-force probes — across plausible main-asset local IDs, file types, hundreds of guid-input string variants, sub-asset id ranges of both the controller and the GLB, recycle-name derivations, direct CRC/XXH/MD4/MD5 hashes of the clip name — produced zero matches. This is the same class of blocker as the AnimatorController `m_TOS` ordering: a non-public Unity algorithm that the no-disassembly rule prevents reverse-engineering from outside.

Note that the non-emote (wearable) glTF path is *not* affected: there the importer calls `AddObjectToAsset` with an explicit name override (`animations/<name>`), which abgen-rs already reproduces byte-exact. The block is specific to the emote mecanim path's nameless attach.

**What would close it:** an IPC probe that dumps each clip's assigned `.controller` local file ID against its name, so the (name → local file id) mapping can be reverse-engineered and mirrored in `pathids.rs`; or an upstream change to the converter that assigns clips a deterministic local file ID we can mirror. Until then this is treated as a known structural-emission gap, not a per-byte content drift, and tagged accordingly so the bookkeeping reflects "blocked" rather than "fixable."

**2026-06-11 probe update — wall confirmed, mechanism fully identified.** The
fork's `SetDeterministicSubAssetIds` rewrites every large YAML document anchor
in the saved `.controller` to `i64_le(md5("{glbHash}/animatorController/{idx}")[..8])`
where `idx` is the document's **rank in the saved YAML file**. Every reference
emote clip PathID is in this family (`examples/clipidx_scan.rs` finds the rank
for all 24 val300 reference clips; single-clip controllers draw ranks 1–6 out
of the 7 large-id docs). The rank is the only unknown — and a controlled
converter re-run probe (identical queue, two batch sessions; 17 emote
conversions each) shows the rank **changes across sessions for 16 of 17
emotes** while everything else in the bundle (including m_TOS order) is
deterministic. Unity writes the YAML docs in ascending original-localID order,
and those original IDs are session-random. So the wall is precisely: a
uniform-random draw of one rank out of ~`#docs` per controller per session.
The only true fix is upstream: make the converter normalize document order (or
index sub-assets by class+name) **before** the md5 rewrite; abgen-rs already
computes the right PathID for any given rank.
