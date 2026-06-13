# Parallel agent findings

Ten agents were dispatched in parallel from `ec0a338` on isolated worktrees,
plus one comment-cleanup agent. The worktree isolation didn't bind their
absolute-path Edit calls — every agent wrote to the main checkout instead
of its worktree, so the dispatch was halted early to prevent cross-agent
corruption. Only one commit landed (`c44ee4a` — cleanup, −7 lines from
`src/builder.rs`); all other work was reverted.

The agents did produce real diagnostic signal before being stopped. The
actionable findings are captured below; the negative-result findings are
captured to avoid re-running the same probes.

## Actionable findings

### A. Mesh AABB ULP-level mismatch (mesh_shift_cascade root cause)

Source: Mesh shift-cascade drill on `QmRy1fKFKuvBK4FQDo…`
glb-animated, 3,558+3,169 @Mesh/shift_cascade runs in atlas row 6.

Observed:
- `ours center.y = 9.01957893371582` (== the f32 min stream value, since
 the corresponding axis has `min == max`)
- `prod center.y = 9.019579887390137` (one ULP higher)
- `ours extent.x = 0.75` (clean)
- `prod extent.x = 0.7499999403953552` (= 0.75 − 1 ULP)

Our `mesh_layout::compute_aabb` uses `center = (min + max) * 0.5` and
`extent = (max − min) * 0.5` in f32. The reference's AABB derivation gives
results that are one ULP off in both center and extent when min == max.

Hypothesis: the reference's `RecalculateBounds` reads back the f16/snorm-encoded
vertex stream and decodes it to f32 via a path that introduces a 1-ULP
rounding bias, then computes bounds from the decoded values. Not from the
source f32 positions.

Action: probe the reference (additive method on `AbgenBundleProbe.cs`) capturing
`mesh.bounds.center/extent` for a known-AABB mesh, then identify the
decode path that reproduces the 1-ULP bias bit-exactly. NO disassembly.

Sample size:
- Tagged 11 Mbits sampled (`mesh_shift_cascade` in `bit_diff_atlas.md`).
- One small Mesh field fix cascades — closes the shift_cascade tail too.

### B. AnimationClip constant-curve split (glb-emote 4737 ppm)

Source: glb-emote AnimationClip drill on
`bafybeickh2wiibpue…`.

Observed:
- Prod: `1185 streamed + 0 dense + 395 constant = 1580 total curves`
- Ours: `1580 streamed + 0 dense + 0 constant = 1580 total curves`
- Prod: `m_DenseClip.m_FrameCount = 268` with `m_CurveCount = 0` and
 empty `m_SampleArray` (frame count set but no curves — diagnostic info)
- Prod: `m_MuscleClipSize = 1319176`
- Ours: `m_MuscleClipSize = 0`
- Both share `m_ValueArrayDelta_len = 1580` and
 `m_ClipBindingConstant.genericBindings_len = 474`
- The `m_ValueArrayDelta[N]` values match for the first 1185 entries
 (the streamed portion); divergence begins at the boundary where prod
 switches to constant-encoded entries

Mecanim partitioning rule (inferred from the reference): curves whose every sample equals every
other sample (within a float tolerance) get partitioned out of the
streamed clip into the `m_ConstantClip`. The streamed clip becomes only
the non-constant curves. `m_MuscleClipSize` is the serialized byte size
of the resulting blob.

Action: implement curve-partitioning in `src/animation_mecanim.rs`:
1. Classify each curve as constant (all samples equal under tolerance)
 or streamed.
2. Emit two sub-clip streams in the same `m_ValueArrayDelta` order
 (streamed first, constant second).
3. Compute and emit `m_MuscleClipSize` post-serialization.

The 395 constant curves at ~857 Kbits each (1.319 MB / 1580) ≈ ~857 Kbits.
Could close ~1 Mbits/emote × 51 emotes = ~50 Mbits if extrapolated, but
the bit_diff_atlas only tags 20 Mbits across all sampled bundles —
attribute the rest to mesh_shift_cascade in finding A above.

### C. Standalone-tex `.resS` is already correct — bug is in verify harness

Source: `Standalone-tex.resS externalization` drill on
`bafkreic5kfzbd34ay…`.

Observed:
- `src/builder.rs` correctly emits `.resS` when `--model-referenced` is
 passed.
- The bit_diff_atlas was generated against bundles built by
 `dev/build_corpus_for_verify.py`, whose model-referenced detection
 heuristic misses `bafkreic5kfzbd34ay…`.
- Atlas row 1's 22 Mbits `standalone_tex_inline_vs_streamed` tag is a
 HARNESS-build artifact, not a real abgen-rs gap.

Action: re-audit `dev/build_corpus_for_verify.py`'s
`collect_model_referenced_hashes` logic — the all-or-nothing rule
(landed in `d2eaf9e`) may be too restrictive for the standalone-tex
bundle's entity. After fix, re-run the bit_diff_atlas to refresh the
taxonomy.

Sample size:
- 22 Mbits `standalone_tex_inline_vs_streamed` + 16 Mbits cascading
 `tex_other_field` = 38 Mbits sampled (and proportionally more
 corpus-wide). NONE of this is a real abgen-rs gap.

## Negative-result findings (don't re-run)

### D. BC7 Rule B (NormalMap channel reshuffle) — already implemented

Source: completed cleanly.

`_pack_normal_map` (R=255, G=Y, B=127, A=X) IS invoked on the
GLB-embedded path via `encode_texture_bc7` line 199 (gated by
`!srgb && looks_like_normal_map`). The standalone path skips it because
`csp=1` (sRGB) for all 56 corpus standalone residuals — no normal-maps
exist on that path.

Synthetic probe confirmed packing reduces:
- `normalmap_flat_64`: 8,232 → 343 bits (−96 %)
- `normalmap_packed_64`: 17,680 → 6,597 bits (−63 %)

Residuals are Rule-4 mode-selection drift (gradient mode bias —
NOT the same root cause as B).

### E. AnimatorController `m_TOS` content is correct — ordering is the blocker

Source: research artifact at
`dev/fix_proposals/m_tos_hash_research.md`.

- Hash function = CRC32-over-UTF8-name-bytes, verified bit-exact against
 prod via UnityPy probe (28/28 captured pairs across 2 AC bundles).
- Already implemented correctly in `src/animation_mecanim.rs::crc32`.
- Set equality holds — both ours and prod have the same 14 `(hash, name)`
 pairs across the test corpus.
- The residual is **iteration order** of the serialized pairs. 17 ordering
 hypotheses tested in `animator_controller_tos.md`, 0/17 match.
- Likely cause: a non-public hashtable variant in the reference's serializer
 (EASTL `swissmap`, bucket_hash_map, or hand-rolled). Confirming requires
 upstream source or disassembly — blocked under no-disassembly rule.

New tool added (additive, project-local):
`AbgenBundleProbe.cs::ProbeAcTOS(bundlePath, outPath)` — Editor IPC
method for capturing larger TOS sets (multi-clip emotes, wearables with
bone-anim overrides) than the 2 in our test corpus.

## What did NOT progress past research

- BC7 Rule A (gradient 2-subset mode bias) — an agent was actively
 debugging mode-selection when stopped. Mode-6 was being selected
 most of the time per debug output, but the bias function was insufficient
 on blocks where mode-5 happened to have err << mode-6.
- BC7 Rules C+E (sub-2 RGBA32 + post-resize mip count) — an agent had
 a partial fix for `default_mip_count` in `src/texprofile.rs` (sub-4
 RGBA32 fallback uses `default_mip_count(w,h)` instead of hardcoded 1)
 and was working on adding a `encode_rgba32_mip_chain` helper. Did not
 complete.
- BC7 Rule D (NPOT downscale via the reference's bilinear resize) — an agent was
 blocked by Unity Editor compile-error modal dialog; never got the
 probe running.
- AnimationClip `m_MuscleClip` — an agent stopped very early (was
 still reading existing Material struct fields).
- Comment cleanup — an agent landed `c44ee4a` (−7 lines from
 `src/builder.rs`); was about to commit more when stopped.

## Re-dispatch notes

Future parallel dispatches must NOT pass absolute paths into the agent's
target codebase as the "repo root" — every agent's Edit / Write call
will then bypass worktree isolation and stomp the main checkout. Either:
1. Tell the agent its working directory IS the worktree root (relative
 paths only), or
2. Pre-resolve `$AGENT_WORKTREE_ROOT` and instruct the agent to
 substitute it for all writes, or
3. Run agents serially.

The shared `.git` makes parallel branches feasible but only if every
agent operates strictly on its own working tree files.
