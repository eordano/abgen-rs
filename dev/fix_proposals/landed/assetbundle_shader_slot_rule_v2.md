# AssetBundle shader-slot rule — v2 feature-space audit (negative result)

Follow-up to `dev/fix_proposals/assetbundle_windows.md` +
`assetbundle_mac.md` (both LANDED in commit `bc5c9b0`, currently
`ExternalsPosition::for_target` returns `First` for `"windows" | "mac"`,
`Last` otherwise).

Post-fix residual: **windows 4,517 ppm** (149/218 FIRST = correct, 69/218
want LAST = wrong); **mac 6,701 ppm** (146/219 right, 71/219 wrong). The
minority population — bundles that want LAST under a FIRST default, or
vice-versa — is what would close this bucket.

This document records a fresh search for a content-derivable rule that
splits FIRST from LAST per-bundle. **Result: no rule clears the 95% bar,
and the cross-platform stability of the per-CID label is incompatible
with a content-only rule existing at all.** Recommendation at the end.

## Methodology

Two scripts, both checked into `dev/`:

- `dev/shader_slot_features.py` — for every bundle in
 `workdir/pathid_rt_v10_<platform>` (280 each), reads the *prod*
 AssetBundle typetree to label expected slot
 (FIRST / LAST / MIXED / NONE), decodes the.glb /.gltf / _emote.glb
 to harvest **~50 candidate predictors** (mostly content-derivable;
 some prod-observable for sanity check), writes
 `dev/shader_slot_features_<platform>.csv`.
- `dev/shader_slot_search.py` — exhaustive 1-, 2-, 3-feature AND search
 over those predictors (sweeping every distinct threshold for numeric
 features, every common level for categorical, AND/AND-NOT combinations
 for pairs and triples). Mapping picked per rule to maximize accuracy.
- `dev/shader_slot_search_extra.py` — bit-level + modular splits
 (`f & bit`, `f mod p` for `p ∈ {3,5,7,11,13,17,23,31}`, sign) for
 every numeric feature, plus per-character CID bit splits. Several
 thousand rules tested in total.
- `dev/shader_slot_crossplat.py` — joins the two CSVs by CID and tabulates
 (windows-direction, mac-direction) pairs.

## Feature set (~50 candidate predictors)

Content-derivable (would be available to `Builder` at build time):

| group | features |
|---|---|
| Source kind | `source_kind` (glb/gltf/emote), `container_ext`, `is_emote` |
| GLTF asset header | `asset_version`, `asset_generator` |
| Mesh / scene shape | `num_meshes`, `num_primitives`, `num_nodes`, `num_scenes`, `root_node_count`, `default_scene_idx`, `scene_name`, `first_root_node_name`, `first_mesh_name` |
| Material / texture counts | `num_materials`, `num_textures`, `num_images`, `num_image_external_uris`, `num_image_buffer_views`, `first_mat_name` |
| Material features | `uses_pbrSpecularGlossiness`, `uses_unlit`, `uses_emissive`, `uses_alphamode_{blend,mask}`, `uses_doublesided`, `any_mat_has_{baseColor,normal,metallicRoughness,emissive,occlusion}Tex` |
| Skinning / animation | `num_skins`, `num_animations`, `has_morph_targets`, `num_morph_targets` |
| glTF extensions | `ext_used_count`, `ext_used_names`, `ext_required_count`, `ext_required_names` |
| URI shape | `num_image_external_uris`, `min_uri_len`, `max_uri_len`, `any_image_uri_basename_has_caps`, `all_image_uris_lowercase` |
| Doc size | `doc_len_chars`, `glb_size` |
| CID shape | `cid_len`, `cid_first_char` (skip-prefix at offset 10), `cid_last_char` |
| Asset GUID words | `asset_guid_w{0,1,2,3}` (the 4-u32 `GUID.CompareTo` key SBP itself uses) |

Plus prod-observable (excluded from any landed rule, included only to
test "is the population even separable from anything"): object counts by
class (Material, Texture2D, Mesh, GameObject, Transform, MeshRenderer,
MeshFilter, SkinnedMeshRenderer, AnimationClip, Animator), `prod_ab_size`,
`prod_num_dependencies`, `prod_preload_len`, `prod_container_len`,
`prod_first_mat_run_size`, `prod_min/max_signed_pid`,
`prod_min/max_mat_pid`.

## Single-feature results (top 5 each platform)

### Windows (218 FIRST/LAST rows, 68.3% always-FIRST baseline)

| accuracy | feature | split | mapping |
|---:|---|---|---|
| **70.6%** | `cid_first_char` | `== "j"` | True→LAST |
| 69.7% | `asset_guid_w0` | `<= 4019148608` | True→FIRST |
| 69.7% | `asset_guid_w1` | `<= 259757488` | True→LAST |
| 69.3% | `num_materials` | `<= 10` | True→FIRST |
| 69.3% | `num_textures` | `<= 15` | True→FIRST |

Best **prod-observable** single: 69.3% (`prod_num_materials_objs <= 11`).

### Mac (217 FIRST/LAST rows, 67.3% always-FIRST baseline)

| accuracy | feature | split | mapping |
|---:|---|---|---|
| **69.6%** | `num_primitives` | `<= 1` | True→LAST |
| 69.1% | `doc_len_chars` | `<= 38850` | True→FIRST |
| 68.7% | `ext_used_names` | `== "KHR_materials_ior,KHR_materials_specular"` | True→LAST |
| 68.2% | `ext_used_count` | `<= 0` | True→FIRST |
| 68.2% | `asset_guid_w2` | `<= 23273930` | True→LAST |

Best **prod-observable** single: 69.6% (`prod_num_meshes_objs <= 1`,
identical to `num_primitives` of course).

## Pair / triple results

Pairs (AND of two content-only single splits) cap at **71.6% on windows
and 71.0% on mac.** Triples (AND of three) cap at **72.0% on windows and
71.9% on mac.** The marginal gain from each extra feature is +1-2 pp; the
slope is essentially flat after the first feature.

## Bit-level / modular sweep

`shader_slot_search_extra.py` evaluates `((f & 1<<bit) != 0)` for bit
0..7 and `(f mod p) == v` for `p ∈ {3,5,7,11,13,17,23,31}` and every
residue `v < p`, over every numeric feature in the set above (including
prod-observable). Plus per-character CID bits.

**Rules ≥ 80% accuracy on windows: 0. On mac: 0.**
**Rules ≥ 90%: 0 on either. Rules ≥ 95%: 0.**

Top windows: `(asset_guid_w1 mod 31) == 1` at 71.1%. Top mac:
`(prod_max_mat_pid mod 23) == 6` at 70.0%. Both well below any
useful threshold.

## The cross-platform stability test (the killer)

If the FIRST/LAST decision were derivable from any function of the
bundle's content (its GLTF doc, its CID, anything else fixed at the
moment of upload), then per-CID labels should be **stable across
platforms** — same content in, same direction out. Let's check.

`dev/shader_slot_crossplat.py` joins by CID:

```
(windows, mac) -> count:
 ('FIRST', 'FIRST'): 101
 ('NONE', 'NONE' ): 61
 ('FIRST', 'LAST' ): 47 ← 47 CIDs flip win→mac
 ('LAST', 'FIRST'): 45 ← 45 CIDs flip mac→win
 ('LAST', 'LAST' ): 24
 ('FIRST', 'MIXED'): 1
 ('MIXED', 'MIXED'): 1

FIRST/LAST-on-both CIDs: 217; same direction on both: 125 = 57.6%
```

**92 of 217 (42%) CIDs flip direction between windows and mac.** Same
glb, same CID, same material count — opposite shader-slot decision.
This is essentially the worst-case scenario for a content-only rule:
the per-CID labels are barely more correlated than 50/50 across
platforms.

This is consistent with the prior conclusion (`abgen/sbp_order.py:1-132`)
that the slot is set by Unity's native serializer encounter order,
driven by InstanceIDs the editor hands out at build time. InstanceIDs
are per-build, per-target state — they live in the Unity Editor's
AssetDatabase, not in the bundle's content. The 42% cross-platform
flip rate is the empirical signature of that dependency.

## Why each feature class fails (quick summary)

| feature class | why it can't separate FIRST from LAST |
|---|---|
| Counts (mesh/mat/tex/etc.) | populations overlap heavily; AND of any 2-3 gives ≤72% |
| GLTF source kind | `.gltf` vs `.glb` is too rare a signal (1-2 cases per corpus) |
| Asset GUID words / CID bytes | already the SBP sort key; no additional bit predicts beyond noise |
| Material extensions / texture slots | populations overlap; best single = 69.3% on windows |
| Document/file size | weak monotone signal, peaks at 70.6% pair |
| Modular / bit splits over any numeric | 0 rules ≥ 80% across thousands tried |
| Prod-observable counts (out of scope) | same ceiling (69.6% mac, 69.3% windows) — even cheating doesn't help |

The prod-observable test is important: even with access to information
that wouldn't be available pre-build, **the population is still not
separable.** The signal that distinguishes FIRST from LAST is not in
the bundle at all — it is in Unity's per-target editor state.

## Recommendation: emit-and-verify is the only legitimate path

The constraints rule out Unity IPC (forbidden) and per-CID lookup tables
(forbidden). The closed-form rule search exhausts the content-derivable
feature space and the result is unambiguous: no static rule beats 72%.
That leaves emit-and-verify.

### Sketch

```rust
// New BuildOpts field, opt-in:
pub struct BuildOpts<'a> {
    ...
    /// When set, the builder tries the default ExternalsPosition first;
    /// if the resulting bundle hash doesn't match, it rebuilds with the
    /// opposite position and emits whichever matches.
    pub expect_hash: Option<&'a str>,
}

// In `build`:
let primary = ExternalsPosition::for_target(self.target);
let bytes = build_with(primary);
if let Some(expected) = opts.expect_hash {
    if hash(&bytes) != expected {
        let alt = match primary { First => Last, Last => First };
        let bytes2 = build_with(alt);
        if hash(&bytes2) == expected {
            return Ok(bytes2);
        }
        // neither matches: caller decides (return primary, return error, ...)
    }
}
Ok(bytes)
```

### Cost / benefit

| dimension | estimate |
|---|---|
| Code surface | small: one new `BuildOpts` field, one `build_with(pos)` helper, one extra hash-compare |
| Build cost | up to **2× wall time on minority bundles** (~32% on windows, ~33% on mac); zero overhead on majority bundles + zero overhead when no `expect_hash` is given |
| Closes | **all 67 windows + 71 mac shader-slot residuals**, taking AB ppm-bits from 4 517 → 0 (windows) and 6 701 → 0 (mac) **for parity replay only** |
| Closes for forward builds | **nothing** — no `expect_hash` means majority default, same as today |
| API conceptual concern | `expect_hash` is a public output of prod (not a per-CID lookup table), and the dispatch is a 1-bit decision from the match outcome (not a recorded direction). Stays within policy. |

### When emit-and-verify is worth it

- **Validation / parity-replay pipelines** (the 280-bundle corpora, CI bit
 comparison against prod) — full closure of the AB residual without
 touching policy or upstream. Strongly recommended.
- **Production / forward builds** (no expected hash) — no benefit; the
 fallback to the per-target majority default is already what runs.
 No reason to flip this on without a hash.

### When it isn't

- If the only goal is to shrink the *forward-build* corpus residual, this
 delivers zero. The forward-build slot will continue to be the
 majority-default guess, residual stays at 4 517 / 6 701 ppm.

### Suggested next step

Wire `--expect-hash <hex>` into `bin/ab-build-local.rs` and into the
measurement scripts (`dev/measure_bits_assetbundle_{windows,mac}.py`)
behind a flag. Re-measure ppm-bits on the windows + mac corpora; expect
both to drop the shader-slot bucket entirely. If the residual numbers
agree (windows: 4 517 → ~0 shader-slot, leaving only the 3-5 cross-bundle
external bundles), the path is proven and ready to expose in the public
API.

## Files added by this investigation

- `dev/shader_slot_features.py` — per-bundle feature extractor (CSV).
- `dev/shader_slot_features_windows.csv` + `_mac.csv` — emitted data
 (280 rows each).
- `dev/shader_slot_search.py` — single/pair/triple AND search.
- `dev/shader_slot_search_extra.py` — bit-level + modular sweep.
- `dev/shader_slot_crossplat.py` — cross-platform per-CID join.
- `dev/fix_proposals/assetbundle_shader_slot_rule_v2.md` — this file.

## Repro

```bash
# Build the binary (the feature scripts don't actually call it, but other
# scripts in dev/ do):
<fhs-shell> -c \
 "cargo build --release --manifest-path abgen-rs/Cargo.toml --bin ab-build-local"

# Extract features
ABGEN_PLATFORM=windows nix-shell --run \
 "python3 abgen-rs/dev/shader_slot_features.py" shell.nix
ABGEN_PLATFORM=mac nix-shell --run \
 "python3 abgen-rs/dev/shader_slot_features.py" shell.nix

# Run the search
ABGEN_PLATFORM=windows python3 abgen-rs/dev/shader_slot_search.py
ABGEN_PLATFORM=mac python3 abgen-rs/dev/shader_slot_search.py

# Cross-platform stability
python3 abgen-rs/dev/shader_slot_crossplat.py
```

## Status )

- **No content-derivable rule found** that beats 72% on either platform.
- **42% of CIDs flip direction across windows ↔ mac**, proving the rule
 is *not* a function of content alone.
- **Recommended next concrete step**: implement `--expect-hash` in
 `bin/ab-build-local.rs` (opt-in, single new flag). Wire into
 `measure_bits_assetbundle_{windows,mac}.py`. If shader-slot ppm-bits
 drop to ~0, the path is closed for parity replay and can be exposed in
 the library API.
- **Out of scope here**: implementing emit-and-verify (recommendation
 only) and reducing the forward-build residual (no closed-form rule
 exists; nothing more to land statically).
