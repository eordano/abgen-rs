# Preload-table run ordering: the cab-merge rule

## The wall

At the time this rule was derived, 1,695 val300 bundles (across the CAT2 /
CAT4 / CAT6 buckets of the classify_pair taxonomy) differed from the
reference ONLY in id-ordering windows with
`ids_changed=false`: identical `(fileID, pathID)` sets, permuted order, inside
the `AssetBundle` (class 142) object. Kinds: glb-scene (1457), glb-wearable
(158), glb-animated (51), glb-scene-collider (16), world (13).

## Localizing the diff

`preload_probe` (examples/preload_probe.rs) dumps the AssetBundle's
`m_Dependencies`, `m_Container` (with preloadIndex/preloadSize), the full
`m_PreloadTable`, the serialized-file externals table, and per-pathID classes.

Sampling 124 affected bundles across kinds and diffing each container entry's
preload run (`m_PreloadTable[preloadIndex .. preloadIndex+preloadSize]`)
ours-vs-ref showed **all 310 differing runs were `material_*.mat` runs**, and
every one was a pure permutation of the same entry set. The glb run, texture
runs, and metadata runs already matched.

## The rule

Pooled orderings from the reference side and hypothesis-tested. Winner, exact
on every sampled run:

> **Each container entry's preload run is its dependency set sorted by
> `(CAB name, signed pathID)` ascending, where internal objects
> (`fileID == 0`) sort under the bundle's OWN CAB name and external objects
> under their dependency's CAB name (all lowercase).**

So whether the shader external lands before or after the local
texture/material entries in a material run depends on how this bundle's own
`cab-<md5>` compares to `cab-51fbd4c9...` (the shader bundle) and to any
cross-bundle texture CABs — which is why the old `ExternalsPosition::First` /
`order_run_by_cab` heuristics matched some bundles and not others: they were
two fixed points of a rule that actually varies per bundle name.

Hypothesis battery on 620 reference material runs (249 bundles):

| hypothesis | matches |
|---|---|
| cab-merge, signed pathID within cab | **620/620** |
| cab-merge, serialized order within cab | 620/620 (degenerate: serialized order == pathID order in these files) |
| cab-merge, unsigned pathID within cab | 495/620 (signed comparison confirmed) |
| internals-then-externals (`ExternalsPosition::Last`) | 440/620 |
| externals-then-internals by cab (old `order_run_by_cab`) | 90/620 |

Universality check: the same rule reproduces **1467/1467** container runs
(glb, material, png, controller, metadata) across 600 random val300 reference
bundles, including byte-identical CAT1 bundles — it is the single preload-run
ordering rule for windows, not a material-specific one.

## Implementation

- `sbp_order::order_run_cab_merge` replaces `order_run_by_cab`.
- `Builder::fill_assetbundle` (src/builder.rs): for windows/mac with no
  externals-position overrides, every entry's run is cab-merge ordered with
  `cab_for(0) = cab_name(bundle_name)`; the per-material `use_cab` heuristic
  (`n_material_entries >= 2 || n_cross >= 2 || (is_last && mp < 0)`) is gone.
- The `ExternalsPosition`/`CrossBundlePosition` override machinery (used by
  the `expect_hash` brute-force retry) is untouched and still bypasses the
  rule when set.

Landing the rule flipped the entire classified id-only cluster to
byte-identical — and more: the gain exceeded the 1,695 classified bundles
because some bundles' "other" residuals were themselves downstream of the
preload permutation (e.g. compressed-block realignment windows attributed to
other categories). Re-derive the current score with `abgen-verify`.
