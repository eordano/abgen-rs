# AssetBundle parity — windows + URP v10 (280-bundle test set)

> **Status: landed.** Target-aware `ExternalsPosition`
> (windows = First) landed in commit `bc5c9b0`. The ~67-bundle minority
> that wants shader-LAST cannot be statically predicted —
> see `assetbundle_shader_slot_rule_v2.md` for the audit + the
> emit-and-verify recommendation tracked there.

This is the per-class slice of `dev/fix_proposals/bitwise_residuals.md`,
scoped to AssetBundle typetree objects on the windows corpus at
`workdir/pathid_rt_v10_windows`. Built from
`dev/bitwise_residuals_windows.py` (set `ABGEN_FILTER_CLASS=AssetBundle`).

## Why look at windows specifically

Background: validation_2 reported ~477 000 ppm-bits diff on the windows test
set vs ~3 900 ppm on the historical linux training corpus — a 100× gap.
Class-by-class breakdown confirms the AssetBundle slice is one of several
contributors, and the dominant per-row error site is platform-specific.

## Pre-fix signatures (152 differing AssetBundles across 280 bundles, 6 sigs)

The classifier groups every differing AssetBundle into a signature (sorted
set of differing field-paths). Sig distribution:

| n | sig | dominant fields |
|---|-----|-----------------|
| 147 (97%) | shader-slot | `m_PreloadTable[*].m_FileID` + `.m_PathID` |
| 2 | cross-bundle `.glb` vs `.gltf` rename | `.m_Container[*][*]`, m_Dependencies len, … |
| 1 | cross-bundle external mismatch (preload-len off-by-1) | as above |
| 1 | cross-bundle external mismatch (preload-len off-by-1) | as above |
| 1 | external `.m_Container[*][*].asset.m_PathID` rewrite | + preload-slot |
| 1 | cross-bundle externals (preload+container+deps off-by-1) | as above |

By field-path frequency:

| count | field-path |
|-------|-----------|
| 1541  | `.m_PreloadTable[*].m_PathID` (scalar) |
| 1150  | `.m_PreloadTable[*].m_FileID` (scalar) |
| 6     | `.m_Container[*][*].preloadIndex` |
| 4     | `.m_Container[*][*].preloadSize` |
| 4     | `.m_PreloadTable` (len) |
| 3     | `.m_Container[*][*].asset.m_PathID` |
| 2     | `.m_Dependencies` (len) |
| 2     | `.m_Container[*][*]` (.glb vs .gltf) |

Two distinct root causes:

1. **Shader-slot position within each material run** (147 of 152 bundles,
 2691 field-row mismatches). Pre-fix abgen-rs put the URP shader external
 (file_id=1, path_id=7645288030342540701) LAST in each `material_*.mat` /
 `DCL_Scene.mat` run. Prod on windows puts it FIRST in 149/218 bundles
 (68%) and LAST in 69/218 (32%). On linux the populations invert: 62/217
 FIRST, 155/217 LAST.

2. **Cross-bundle externals** (5 bundles). Same pre-existing residuals as
 the linux corpus (see `cross_bundle_externals.md` and `textasset.md`).
 Out of class for this slice — the AssetBundle field-diffs are a knock-on
 of the cross-bundle plumbing in textures/materials/textasset.

## Hypothesis (signature 1) and the fix

The shader-slot position is a deterministic function of Unity's
**editor-side InstanceID assignment** for the shader bundle at build time.
We can't reproduce that assignment without invoking Unity in the converter
pipeline. Prior reverse-engineering (`abgen/sbp_order.py` docstring) ruled
out every closed-form derivation we tried — single static sort-key, scalar
feature sweep, per-bundle decision trees — and ruled out per-CID lookups
per project policy.

However, the **per-target majority slot** is robust:

* windows: shader-FIRST dominates 149/218 = 68%
* linux: shader-LAST dominates 155/217 = 71%

These are derivable from the build target alone (a public input). Setting
the per-platform default reduces the dominant signature from 147 bundles to
the (smaller) minority population.

### Implementation

Added `ExternalsPosition::{First, Last}` to `abgen-rs/src/sbp_order.rs` with
`ExternalsPosition::for_target(target)` returning `First` for `"windows"`
and `Last` otherwise. Glb builder call site
(`src/builder.rs::fill_assetbundle`) passes the target-aware position.
Mirrored in `abgen/sbp_order.py::externals_position_for_target` and
`abgen/builder.py`. Standalone-texture builder has no externals → unchanged.

Parity fixtures (`abgen-rs/tests/fixtures/parity/`) regenerated from the
new python output; 13 of 21 binary fixtures changed (size deltas
±2..±9 bytes — exactly the LZ4-compressed delta of moving the shader PPtr
within the AssetBundle typetree).

## Post-fix signatures (72 differing AssetBundles, 5 sigs)

| n | sig | dominant fields |
|---|-----|-----------------|
| 67 (93%) | shader-slot (now wrong-direction LAST-class minority) | `.m_FileID` + `.m_PathID` |
| 2 | cross-bundle `.glb` vs `.gltf` rename | as before |
| 1 | cross-bundle preload off-by-1 | as before |
| 1 | cross-bundle asset.m_PathID + shader-slot | as before |
| 1 | cross-bundle (preload+container+deps) + shader-slot | as before |

The shader-slot signature shrinks from 147 → 67 (−80, −54%). The remaining
5 are out-of-class cross-bundle externals (Texture2D/material plumbing).

## Closure measurement

Per-class AssetBundle bits-diff on the 280-bundle `pathid_rt_v10_windows`
test set, measured via `dev/measure_bits_assetbundle_windows.py`:

|                              | baseline | + fix 1   | + fix 2   |
|------------------------------|---------:|----------:|----------:|
| AssetBundle byte-id          | 128/280  | 208/280   | 208/280   |
| AssetBundle bits diff        | 70 330   | 52 038    | 41 512    |
| AssetBundle ppm-bits         | 7 652    | 5 662     | 4 517     |
| delta vs baseline (ppm)      | —        | −1 990    | −3 135    |
| delta vs baseline (% diff)   | —        | −26%      | −41%      |

* **fix 1** = target-aware shader-slot position (windows-FIRST default).
* **fix 2** = source-extension-aware `m_Container` glb-prefab key (gltf
 inputs emit `.gltf` instead of `.glb`).

(Both measurements use a stable type-aware canonical encoding — see the
script's `to_bytes` helper. Total bits-compared is identical pre/post
because the AssetBundle field shape doesn't change, only PPtr ordering
and key string content.)

Overall bundle bits-diff (the LZ4-compressed file as a whole) moves only
~40 ppm on the windows set (477 305 → 477 266 ppm) because the AssetBundle
typetree is tiny relative to texture+mesh data — the LZ4 ratio amplifies
big payload differences and dilutes small ones.

## Open residuals

* **67 windows bundles want shader-LAST** (the minority population under
 the new windows-FIRST default). No closed-form rule has been found that
 splits them from the 149 majority — the prior research is documented in
 `abgen/sbp_order.py` (single-static-key contradiction, feature sweep,
 decision-tree threshold tests). Path forward: either (a) harvest Unity
 importer InstanceIDs at build time (requires Editor IPC, out of scope for
 abgen-rs), or (b) emit-and-verify when an expected hash is known
 (already covered by the parity test in principle).
* **5 cross-bundle external residuals** show up as AssetBundle field-diffs
 but are caused by the Texture2D/material cross-bundle plumbing — flagged
 for the Texture2D / TextAsset agents in `cross_bundle_externals.md` and
 `textasset.md`.

## Files touched

### fix 1 — target-aware shader-slot position

* `abgen-rs/src/sbp_order.rs` — added `ExternalsPosition`,
 `order_run_with`, `build_preload_and_container_with`,
 `externals_position_for_target`-style constructor.
* `abgen-rs/src/builder.rs::fill_assetbundle` — uses target-aware
 position.
* `abgen-rs/dev/gen_sbp_vectors.py` — new `cases_first` /
 `order_runs_first` golden vectors.
* `abgen-rs/tests/fixtures/sbp_vectors.json` — regenerated.
* `abgen-rs/tests/fixtures/parity/*.bundle` (13 files) +
 `parity/index.json` — regenerated from new python output.
* `abgen/sbp_order.py` — added `EXTERNALS_FIRST/LAST`,
 `externals_position_for_target`, updated module docstring.
* `abgen/builder.py` — glb builder passes target-aware position.

### fix 2 — gltf source extension propagated to container key

* `abgen-rs/src/builder.rs` — added `Builder.is_gltf`; threaded through
 `Builder::new`; `fill_assetbundle` selects `.gltf` vs `.glb` for the
 glb-prefab container key.
* `abgen/builder.py` — added `_Builder.is_gltf`; threaded through
 `_Builder.__init__` and `build_bundle`; `_fill_assetbundle` selects
 `.gltf` vs `.glb`.

## Repro

```bash
<fhs-shell> -c "cargo build --release --bin ab-build-local"
ABGEN_FILTER_CLASS=AssetBundle nix-shell --run \
    "python3 abgen-rs/dev/bitwise_residuals_windows.py" shell.nix
nix-shell --run "python3 abgen-rs/dev/measure_bits_assetbundle_windows.py" shell.nix
```

## Status )

- **Fix 1 (target-aware shader-slot)** — LANDED in commit `bc5c9b0`.
 `src/sbp_order.rs::ExternalsPosition::for_target` returns `First` for
 `"windows" | "mac"`, `Last` otherwise. `src/builder.rs::fill_assetbundle`
 consumes the position. Verified.
- **Fix 2 (gltf source-extension container key)** — LANDED in commit
 `bc5c9b0`. `src/builder.rs::Builder.is_gltf` threaded through
 `fill_assetbundle`. Verified.
- **AssetBundle ppm-bits (windows v10, 280 bundles):** 4 517 ppm
 (post-fix figure from §"Closure measurement"; not re-run on this
 branch — `cargo test --release --lib` is 107, `--test parity_bytes` is
 1, both unchanged).
- **Why still in root:** the 67 shader-LAST-minority residuals are the
 open part. They need either Unity ImporterContext capture (Editor IPC,
 out of scope per project policy: no Unity binary disassembly, no
 per-CID lookup tables) or emit-and-verify with an expected hash.
- **Next concrete step:** explore an emit-and-verify path that, when a
 prod hash is supplied via `--expect-hash <hex>`, rebuilds with the
 alternate `ExternalsPosition` and picks whichever matches. Falls back
 to the majority default when no hash is supplied. The mechanism is
 legitimate under the constraints — the hash itself is a public output
 of prod, and the dispatcher is a one-bit decision derived from the
 match outcome, not a per-CID lookup.
