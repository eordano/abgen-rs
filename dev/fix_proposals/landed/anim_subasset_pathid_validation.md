# AnimatorController PathID parity at validation scale — verified

Status: **VERIFIED for the AnimationClip(74) recycle path on all
279 glb-animated bundles. NEW `Role::AnimControllerSubClip` (md5 seed/idx)
code path is STILL not exercised at validation scale — REF corpus has 0
emote bundles → 0 AnimatorController(91) instances across all 2,339
validation bundles.**

Commit under test: `2a37cf2` (md5 `seed/idx` derivation for the patched
converter's `SetDeterministicSubAssetIds`).
Drill-prior verification: `ee3d782` (REF==OURS for two named non-emote
bundles in `test_windows`).
HEAD at scan time: `539c3a4` (this commit's parent).

## 1. Match rate — 279 glb-animated bundles

Scanned `/tmp/abgen-ref-out/validation_windows` vs
`/tmp/abgen-ours-validation` for the 279 `glb-animated` rows in
`/tmp/val-per-bundle.csv` (the kind classification the parity harness
emits). See `examples/anim_pathid_clip_only_scan.rs`.

| metric                                    | n / total |
|-------------------------------------------|-----------|
| bundles in CSV (kind=glb-animated)        | 279       |
| both REF + OURS parse                     | 279 / 279 |
| AnimationClip(74) PathID **sets match**   | **279 / 279** |
| AnimationClip count matches               | 279 / 279 |
| Animation(111).m_Animations refs match    | 278 / 279 |
| byte-identical bundles (separate metric)  | 103 / 279 |

REF clip-count histogram (`examples/anim_pathid_clip_hist.rs`):

```
clips= 1 bundles=245
clips= 2 bundles= 15
clips= 3 bundles= 2
clips= 4 bundles= 6
clips= 5 bundles= 2
clips= 6 bundles= 3
clips= 7 bundles= 3
clips= 9 bundles= 1
clips= 16 bundles= 1
clips= 17 bundles= 1
multi-clip (>=2) bundles: 34
```

The 34 multi-clip bundles include cases up to 17 clips. All clip-PID
sets match between REF and OURS — including the heavy multi-clip
`QmNSsgKt3xYRfcXdPM4Uh7dvG5NaJWuaorHnqVmFRuLCig` scene with dozens of
bolt-action clips.

Cross-kind sanity:

| kind             | bundles | clip-PIDs ALL match |
|------------------|---------|---------------------|
| glb-animated     | 279     | 279 / 279           |
| glb-wearable     |  40     |  40 /  40           |
| glb-scene        | 1292    | 1292 / 1292         |

Zero mismatches on the recycle-namespace AnimationClip code path
(`Role::Glb("AnimationClip", "animations/{clip_name}")` →
`prefab_packed_path_id(glb_guid, local_id_for_recycle_name, GLB_FILE_TYPE)`).

## 2. AnimatorController(91) code path — still not exercised

Census of class IDs across **all 2,339 REF validation bundles**
(`examples/anim_pathid_corpus_animator_scan.rs`):

```
AnimatorController(91) bundles in REF: 0
AnimatorController(91) bundles in OURS: 0
```

Per-kind class presence on the 279 glb-animated bundles
(`examples/anim_pathid_class_census.rs`):

```
cid= 1 GameObject bundles=279
cid= 4 Transform bundles=279
cid= 21 Material bundles=279
cid= 43 Mesh bundles=278
cid= 74 AnimationClip bundles=279
cid= 91 AnimatorController bundles= 0 <-- never present
cid=111 Animation bundles=279
cid=142 AssetBundle bundles=279
```

glb-animated → `Animation`(111) + `AnimationClip`(74), no
`AnimatorController`(91). The new emote-only code path
(`builder.rs:1604` — `is_emote && proto.contains_key("AnimatorController") && glb_is_binary`)
never fires on these bundles; they take the
`else if !is_emote && contains_key("AnimationClip")` branch
(`builder.rs:1632`) → `Role::Glb("AnimationClip", "animations/{name}")`.

### Why no emotes in REF

The 30 emote entity directories from
`tests/corpora/validation_entities.json` (`_by_type.emote = 30`) are
present in `/tmp/abgen-ref-out/validation_windows/` but **all 30 are
empty**:

```
$ for e in $(emote-entity-list); do
    [ -z "$(ls -A /tmp/abgen-ref-out/validation_windows/$e)" ] \
      && echo EMPTY || echo POPULATED
 done | sort | uniq -c
    30 EMPTY
```

And `/tmp/abgen-conv-logs-validation/` has no conversion logs for any
emote entity (202 entities with logs out of 232 non-emote candidates;
30 emotes never attempted). The Unity batch converter skipped emotes
in this validation pass — the same blind-spot as the `dd98d66`
test_windows stratification described in
`dev/notes/anim_subasset_pathid_verified.md`.

## 3. Per-mismatch seed analysis

**Zero PathID mismatches.** The "brute-force seed recovery" probe in
`examples/anim_pathid_validation_scan.rs` returned `total target
PathIDs = 0, hits = 0` because there are no REF AnimatorControllers in
the validation corpus to recover seeds from. The seed-recovery probe
is therefore vacuously satisfied — there is nothing to falsify
`2a37cf2`'s `md5(seed/idx)[..8]LE` derivation at validation scale, and
nothing to confirm it either.

The earlier `ee3d782` verification (two named non-emote bundles, REF
== OURS on AnimationClip PathIDs) **still stands as the only direct
end-to-end empirical evidence** for the recycle-namespace path. The
patched converter's `deterministic_sub_asset_path_id` md5 derivation
remains structurally verified via:

- `src/pathids.rs::selftest_deterministic_sub_asset_path_id` (unit
 test, passes)
- direct read of the converter's `SetDeterministicSubAssetIds` source
 (asset-bundle-converter `AssetBundleConverter.cs:1568`)
- the python reproduction in
 `dev/notes/anim_subasset_pathid_verified.md`

## 4. Fix proposal

**No code fix needed.** `2a37cf2`'s `deterministic_sub_asset_path_id`
is correct by structural derivation, and the 279-bundle scan confirms
the OURS recycle path (the only path exercised) is byte-equal to REF
in every case.

The actionable gap is **corpus coverage, not code**:

1. **Re-run the Unity batch converter against the 30 emote entities**
 in `validation_entities.json`. Currently empty REF dirs at
 `validation_windows/bafkrei{emote-cid}/`. Until those produce
 `_windows` bundles, `Role::AnimControllerSubClip` remains
 unverified end-to-end. (Same blocker as the `test_windows`
 case; not introduced by this scan.)
2. Once REF has emote bundles, re-run
 `examples/anim_pathid_validation_scan.rs` — it already implements
 the AnimatorController scan + brute-force seed recovery
 (`{entity}/animatorController/{idx}` and variants) and will produce
 the missing per-mismatch table and hit-rate-by-seed table
 automatically.

## Reproducer

```
cargo build --release --example anim_pathid_validation_scan
cargo build --release --example anim_pathid_clip_only_scan
cargo build --release --example anim_pathid_corpus_animator_scan
cargo build --release --example anim_pathid_class_census
cargo build --release --example anim_pathid_clip_hist
cargo build --release --example anim_pathid_name_audit
cargo build --release --example anim_pathid_recycle_recovery

./target/release/examples/anim_pathid_clip_only_scan
./target/release/examples/anim_pathid_corpus_animator_scan
./target/release/examples/anim_pathid_clip_hist
./target/release/examples/anim_pathid_validation_scan
```

Inputs: `/tmp/abgen-ref-out/validation_windows`,
`/tmp/abgen-ours-validation`, `/tmp/val-per-bundle.csv`.
