# `AssetDatabase.AddObjectToAsset` PathID — non-deterministic across sessions

## Tl;dr

Drove a controlled 370-vector probe of `AssetDatabase.AddObjectToAsset(child,
parent)` (no name override) across `{AC, Material, Texture, PrefabGO}` ×
`{AnimationClip, Material, Texture, Mesh, GameObject}` sub-asset combinations,
covering 37 distinct name patterns (empty, ASCII, UTF-8, long, repeated, the
exact "Super Sayajin" / "Walk" / "Jump" emote names blocking the emote-clip-PID work). Ran the
same probe **twice** against the same project to test cross-session
determinism.

**Result: 717 / 717 non-main sub-asset LFIDs DIFFER between runs.** Same
parent guid, same child name, same child type → different `localIdentifierInFile`.
Only stable per-type main asset IDs (`AnimatorController=9100000`,
`Material=2100000`, `Texture2D=2800000`) are reproducible across runs; even
`Prefab/GameObject` main assets DIFFER.

This means **no pure function of `(parent_guid, child_name, child_type)` can
predict the LFID** — Unity's internal generator carries hidden state across
the call (instance-id-derived or seeded by an editor-session RNG, persisted
into the `.asset` YAML only as a post-hoc record). The 140 M brute force in
the prior emote-clip-pid investigation was correct in its negative result;
the answer truly is not in the search space.

This rules out **Option A** ("probe Unity and derive the formula") from
`emote_animclip_pathid.md`. Only Options B (upstream override of
`localIdentfierInFile` via importer / `ScriptedImporter`) and C (structural-
diff acceptance — bookkeep the 2 emote bundles in the windows-corpus as
"known PathID nondeterminism") remain.

## Reproducer

```bash
PROJ=<workspace>
UNITY=<unity-editor>
FHS=<unityhub-fhs-env>
OUT=/abs/out.jsonl

ABGEN_AOA_OUT=$OUT $FHS $UNITY -batchmode -nographics -quit \
 -projectPath $PROJ \
 -executeMethod AbgenRs.BundleProbe.ProbeAddObjectToAssetPidFromCli \
 -logFile /tmp/unity-aoa.log
```

Pre-flight one-time setup on this host: `Assets/Editor/ClaudeIPC.cs` in the
abgen-verify project references `UnityEngine.UI.Button` but the project
manifest lacks UGUI on the default Editor assembly classpath. To compile
batchmode, move `ClaudeIPC.cs(.meta)` aside (`.disabled` suffix) before the
run — `-executeMethod` doesn't need ClaudeIPC's IPC machinery. Restore after.

Probe added (additively, no changes to existing methods) at
`workspaces/abgen-verify/Assets/Editor/AbgenProbe/AbgenBundleProbe.cs::ProbeAddObjectToAssetPid{,FromCli}`.

## Vector matrix

10 (parent_kind, child_kind) pairs × 37 name patterns × 1 rep = **370
vectors, 1457 JSONL rows total** (370 parent + 1050 sub + 37 trailing
metadata).

Parent kinds: `AC` (AnimatorController via
`AnimatorController.CreateAnimatorControllerAtPath`), `Material`, `Texture`
(Texture2D-as-asset), `PrefabGO` (PrefabUtility.SaveAsPrefabAsset of an empty
GameObject).

Child kinds: `AnimationClip`, `Material`, `Texture` (Texture2D), `Mesh`,
`GameObject`. Combinations are gated to plausible parent×child pairs
(`AC×{AnimationClip,Material,Texture,Mesh}`, `Material×{Material,Texture}`,
`Texture×Texture`, `PrefabGO×{GameObject,Mesh,Material}`).

Name patterns: empty, single char, exact-failing-case clip names (`Super
Sayajin`, `Walk`, `Jump`, `Run`, `Idle`), `Clip0..Clip4` collisions, repeated
characters, ASCII variations, 64/128/256-char strings, UTF-8 multibyte
(`サイヤ人`, `Über`, `你好`), and same-name-thrice (`["dup","dup","dup"]`).

## Captured data

- `dev/probe_data/addobjecttoasset_pid.jsonl` — first run (375 KB, 1457
 rows).
- `dev/probe_data/addobjecttoasset_pid_run2.jsonl` — re-run, same project,
 same matrix, same parent_guids (paths preserved → guids cached in `.meta`).

JSONL row schema:
```
{vid, rep, role, parent_kind, child_kind, parent_path, parent_guid,
 parent_lfid, parent_name} # role=parent
{vid, rep, role, order, parent_kind, child_kind, is_main, type, name,
 guid, lfid, iid} # role=sub
```

## Hash families tested (all zero matches)

For each of 200 first-occurrence `(parent_guid, name, type, lfid)` samples
under run1, tried every combination of:

- **Payload mix** (12 variants): `g_raw + name`, `g_raw + class_id + name`,
 `g_raw + name + class_id`, `name + g_raw`, `g_raw + name\0`,
 `g_raw + parent_lfid + name`, `g_asc + name`, `g_asc + class + name`,
 `class + name`, `class_q + name`, `name only`, `g_raw + name + class_q`.
- **Hash function** (9 variants): MD4-low64, MD5-low64, SHA1-low64,
 xxh64-seed0, xxh64-seed1, xxh64-xxh-seed-constant, spooky_short first half,
 spooky_short second half, spooky_short XOR.

= **108 (payload × hash) combinations × 200 samples = 21,600 hash-vector
checks, 0 matches.**

That's a strong negative on the trivial "hash some byte string" hypothesis,
and it matches the cross-run nondeterminism finding: there's no formula to
discover.

## Cross-session determinism check (definitive)

Ran the probe twice against the same `abgen-verify` project (same temp asset
paths → same parent guids cached in `.meta`):

| asset class                | same lfid | diff lfid |
|----------------------------|----------:|----------:|
| Main asset, stable type    |       259 |         0 |
| Main asset, PrefabGO       |         0 |        74 |
| Sub-asset (any non-main)   |         0 |       717 |

The 259 "main same" are the well-known per-Unity-class fixed IDs:
`AnimatorController=9100000`, `Material=2100000`, `Texture2D=2800000`. The
74 "main diff" are prefab main objects, whose LFIDs are also derived from
the same nondeterministic generator that controls all sub-assets.

YAML spot-check of the same `(parent_path, child_name)` across runs
confirms Unity rewrites the `.controller`/`.prefab`/`.asset` YAML with a
fresh `&<fileID>` each session — the LFID is **persisted as
post-hoc state, not derived from `(parent_guid, child_name)`**.

## What this changes

- `dev/fix_proposals/emote_animclip_pathid.md` — Option A is **dead** (no
 derivable formula exists). The blocker tag is therefore not "we haven't
 brute-forced the right hash variant yet"; it's "Unity uses an editor-
 session-stateful PRNG for `AddObjectToAsset` sub-asset IDs". Update that
 doc when this lands.
- `dev/fix_proposals/animator_controller_tos.md` (m_TOS ordering) —
 this probe doesn't directly inform m_TOS ordering, but the same
 conclusion applies if m_TOS ordering depends on a hash-map iteration
 whose insertion order is in turn driven by instance-id state.
- Implementation: Option B requires asset-bundle-converter to set
 `AssetImporter.AddRemap` / `AssetImporter.SetExternalObjects` /
 ScriptedImporter `localIdentfierInFile` overrides at conversion time,
 producing deterministic LFIDs we can mirror in `src/animation_mecanim.rs`.
 That's an upstream-converter change, outside this probe's scope.
- Bookkeeping: the bit_diff_atlas should classify the 2 emote-clip-PID
 divergence bundles as a permanent "blocked-on-upstream" tag (Option C
 from the original doc) rather than as a tractable per-byte residual.

## Files

- ADDED: `dev/fix_proposals/addobjecttoasset_pathid_probe.md` (this file).
- ADDED: `dev/probe_data/addobjecttoasset_pid.jsonl` (run1, 1457 rows).
- ADDED: `dev/probe_data/addobjecttoasset_pid_run2.jsonl` (run2, 1457 rows).
- ADDED (additive, outside-this-worktree project): `workspaces/abgen-verify
 /Assets/Editor/AbgenProbe/AbgenBundleProbe.cs::ProbeAddObjectToAssetPid{,FromCli}`.
- UNTOUCHED: `src/animation_mecanim.rs`, `src/pathids.rs`, `src/builder.rs` —
 the cap is gated on a non-derivable Unity-internal generator.
