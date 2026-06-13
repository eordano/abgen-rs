# AssetBundle parity — mac + URP v10 (280-bundle test set)

> **Status: landed.** Target-aware `ExternalsPosition`
> (mac = First) landed in commit `bc5c9b0`. Residual ~71 minority
> bundles want shader-LAST under the new FIRST default — closed-form
> rule proven non-existent in `assetbundle_shader_slot_rule_v2.md`.

Per-class slice of `dev/fix_proposals/bitwise_residuals.md`, scoped to
AssetBundle typetree objects on the mac corpus at
`workdir/pathid_rt_v10_mac` (StandaloneOSX). Built from
`dev/bitwise_residuals_mac.py` (set `ABGEN_FILTER_CLASS=AssetBundle`) and
`dev/measure_bits_assetbundle_mac.py`.

## Why look at mac specifically

After the windows AssetBundle fixes landed (target-aware shader-slot position
+ source-extension-aware container key — see
`dev/fix_proposals/assetbundle_windows.md`), the mac corpus still uses the
LAST default inherited from linux. This file repeats the windows analysis on
mac and applies the same target-aware switch.

## Pre-fix signatures (148 differing AssetBundles across 280 bundles, 4 sigs)

Equivalent to the pre-fix windows row. Dominated by shader-slot.

| n | sig | dominant fields |
|---|-----|-----------------|
| 145 (98%) | shader-slot | `m_PreloadTable[*].m_FileID` + `.m_PathID` |
| 2 | cross-bundle externals (preload+container+deps off-by-1) | `.m_Container[*][*]` family + `.m_PreloadTable` |
| 1 | external `.m_Container[*][*].asset.m_PathID` rewrite | + preload-slot |

Per-bundle externals-position classifier (`classify_shader_slot.py` ported to
mac, see /tmp/mac_classify.json):

```
final: {'FRONT': 146, 'BACK': 71, 'MIXED': 2, 'NONE': 61}
```

`NONE` = no material run / no shader external present (e.g.
standalone-texture bundles, model-only-renderer bundles). Among the 219
bundles that carry a shader external, **146/219 = 67% emit FIRST**, mirroring
the windows 149/218 = 68% almost exactly. mac is dominantly shader-FIRST.

## Hypothesis and the fix

Identical to the windows hypothesis: the URP shader external's per-material
slot is set by Unity's editor-side InstanceID assignment for the shader
bundle, which we cannot reproduce without invoking Unity. The per-target
majority slot is the best closed-form we have:

* windows: shader-FIRST dominates 149/218 = 68% (already landed)
* mac: shader-FIRST dominates 146/219 = 67% (this fix)
* linux: shader-LAST dominates 155/217 = 71% (unchanged default)

### Implementation

Extended `ExternalsPosition::for_target` in
`abgen-rs/src/sbp_order.rs` to return `First` for `"mac"` as well as
`"windows"`; everything else stays `Last`. Mirrored in
`abgen/sbp_order.py::externals_position_for_target`. The
source-extension-aware container-key fix from
`dev/fix_proposals/assetbundle_windows.md` (windows landed) already covers
the 2 gltf-source mac bundles — no extra work needed there.

The unit test in `sbp_order.rs::externals_position_for_target_matches_first_rule`
was extended to assert `"mac" -> First` and renamed from
`..._matches_windows_first_rule`.

## Post-fix signatures (74 differing AssetBundles, 3 sigs)

| n | sig | dominant fields |
|---|-----|-----------------|
| 71 (96%) | shader-slot (now wrong-direction LAST-class minority) | `.m_FileID` + `.m_PathID` |
| 2 | cross-bundle preload+container+deps off-by-1 | as before |
| 1 | cross-bundle asset.m_PathID + shader-slot | as before |

Shader-slot signature shrinks from 145 → 71 (−74, −51%), matching the
windows reduction. The remaining 3 are out-of-class cross-bundle externals
(Texture2D/TextAsset plumbing).

## Closure measurement

Per-class AssetBundle bits-diff on the 280-bundle `pathid_rt_v10_mac` test
set, measured via `dev/measure_bits_assetbundle_mac.py` (canonical-JSON of
the AssetBundle typetree, XOR + popcount across bytes):

|                              | baseline | + fix (mac→FIRST) |
|------------------------------|---------:|------------------:|
| AssetBundle byte-id          | 132/280  | 206/280           |
| AssetBundle bits diff        | 111 394  | 60 842            |
| AssetBundle ppm-bits         | 12 269   | 6 701             |
| delta vs baseline (ppm)      | —        | −5 568            |
| delta vs baseline (% diff)   | —        | −45%              |

Baseline (mac, LAST) measurement: 132/280 AB byte-id, 12 268.8 ppm-bits diff.
After flipping mac to FIRST: 206/280 AB byte-id, 6 701.1 ppm-bits diff —
74 additional byte-id bundles. The 5 568 ppm-bits drop and the 74-bundle
byte-id swing match the 74 shader-slot-signature bundles that flipped from
wrong (LAST) to right (FIRST) under the new default.

The container-key gltf vs glb fix (already in main for windows) contributes
0 ppm-bits movement on mac — the 2 gltf bundles in this corpus were already
covered by that earlier landed change.

## Open residuals

* **71 mac bundles want shader-LAST** (the minority population under the new
 mac-FIRST default). No closed-form rule has been found that splits them
 from the 146 majority — prior research is documented in
 `abgen/sbp_order.py:1-132` (single-static-key contradiction, feature
 sweep, decision-tree threshold tests; tested on both linux and windows
 populations). The mac population behaves the same.
* **3 cross-bundle external residuals** show up as AssetBundle field-diffs
 but are caused by the Texture2D / TextAsset cross-bundle plumbing —
 flagged for the cross-bundle externals work, out of class for this slice.

Path forward to close the minority population:

 1. **Path A — harvest Unity importer InstanceIDs at build time.** Same
     conclusion as windows; needs Editor IPC, out of scope for abgen-rs.
 2. **Path B — per-CID override table.** Recordable from the harvested
     prod corpus (~217 mac bundles + 218 windows bundles = ~435 entries
     across the two corpora). Trivially implementable but ugly and fragile.
 3. **Path C — emit-and-verify** for ambiguous runs when an expected hash
     is known (already covered by the parity test in principle).

## Files touched

* `abgen-rs/src/sbp_order.rs` — `ExternalsPosition::for_target` now also
 returns `First` for `"mac"`. Enum docstring + impl docstring updated with
 the mac measurement (146/219 = 67% shader-FIRST). Unit test renamed and
 expanded to cover `"mac"`.
* `abgen/sbp_order.py` — module docstring + `externals_position_for_target`
 updated to mirror the rust side.
* `abgen-rs/dev/measure_bits_assetbundle_mac.py` — measurement harness
 (copied from previous mac-fix attempt).
* `abgen-rs/dev/bitwise_residuals_mac.py` — signature classifier (same
 source).

No changes to `tests/parity_bytes.rs` or its fixtures. The sbp_vectors.json
golden already covers `cases_first` since the windows fix landed.

## Repro

```bash
/home/dcl/linux-rigging/dcl-shell -c "cargo build --release --bin ab-build-local"
ABGEN_REPO_ROOT=/home/dcl/umbrella/ab-generator \
ABGEN_AB_BIN=$PWD/abgen-rs/target/release/ab-build-local \
nix-shell --run \
 "python3 abgen-rs/dev/measure_bits_assetbundle_mac.py" shell.nix

ABGEN_REPO_ROOT=/home/dcl/umbrella/ab-generator \
ABGEN_AB_BIN=$PWD/abgen-rs/target/release/ab-build-local \
nix-shell --run \
 "python3 abgen-rs/dev/bitwise_residuals_mac.py" shell.nix
```

## Status )

- **mac → ExternalsPosition::First** — LANDED in commit `bc5c9b0` (same
 patch as the windows fix; `for_target("mac") == First`). Verified in
 `src/sbp_order.rs`.
- **AssetBundle ppm-bits (mac v10, 280 bundles):** 6 701 ppm
 (post-fix figure from §"Closure measurement"; not re-run on this
 branch).
- **Why still in root:** 71 shader-LAST-minority bundles remain — same
 shape as the windows residual (cross-platform Unity ImporterContext
 artifact). All three "Open residuals" path-A/B/C options below remain
 applicable.
- **Next concrete step:** identical to `assetbundle_windows.md` — pursue
 an emit-and-verify path A on the AB sub-tree when a prod hash is
 supplied. Or measure the residual using `dev/measure_bits_assetbundle_mac.py`
 after any reordering experiment.
