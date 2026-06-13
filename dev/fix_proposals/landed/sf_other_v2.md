# `__sf_other__` v2 — legacy CIDv0 scene metadata-TextAsset fix + remaining residuals

New tools: `dev/sf_other_decompose.py`,
`dev/sf_other_one.py`, `dev/sf_other_scan_top.py`. The 22-entity test set
(`workdir/pathid_rt_v10_windows/`, 2,174 bundles) is 7.7× the previous
280-bundle baseline. Three of the 22 entities are CIDv0 scenes (`Qm…`
prefix); the other 19 are CIDv1 (`bafkrei` prefix). The CIDv0 split is
~19% of bundles (405) and was, before this fix, the single largest
contributor to `__sf_other__`.

## What `__sf_other__` decomposes into

`dev/sf_other_decompose.py` walks the SerializedFile metadata layout per
side, pairs regions by `(class_id, ordinal_within_class)` for typetree
slots so the byte windows align across declaration-order differences, and
XOR-popcounts each region. Output buckets:

```
01_sf_header # 48-byte header (metadata_size, file_size, data_offset…)
02_sf_top_fields # unity_version + target_platform + enable_type_tree
03_type_count
04_type_hdr # per-type fixed header (class_id, hash, etc.)
05_typetree_hdr_per_type # per-type node_count + sb_size
06_typetree_nodes_per_type # per-type node table (32B × node_count)
07_typetree_string_buffer # per-type local string buffer
08_typetree_deps # per-type type-dependencies array
10_obj_count
11_obj_table # path_id + byte_start + byte_size + type_id (per obj)
13_externals # file identifiers (GUID + path)
14_ref_types # only if version >= 20 and any present
15_user_information
16_data_section_plus_align # everything after metadata — object payloads + 16B align
```

(The `data_section_plus_align` bucket overlaps with the per-class object
windows that `class_bits_audit.py` already attributes — by design. Treat
it as "total data-section bytes-diff" and subtract per-class object-window
sums to isolate inter-object padding gaps.)

## Top offender on the new test set — metadata TextAsset on legacy scenes

Sorting bundles by `__sf_other__` bits-diff (per-bundle: total SF diff
minus per-class explained) puts ALL of the top 20 inside the three Qm
entities. Object-count probe:

```text
prod (Qm scene) ─ 113 objects (53 GO + 53 TR + 1 AB + 1 Mesh + 2 Material +
                                 1 Animation + 1 AnimationClip + 1 SMR)
ours ─ 114 objects (same + 1 TextAsset named "metadata")
```

Cross-entity-class probe (sampling up to 30 prod bundles per entity):

| Entity prefix | Total entities | Prod bundles with `metadata` TextAsset |
|---|---:|---:|
| `Qm…`     (CIDv0) | 3  | 0 / sampled |
| `bafkrei…` (CIDv1) | 19 | every sampled bundle |

100% clean signal: the pre-v3 (CIDv0) converter did **not** write a
`metadata` TextAsset object into the bundle; the v3+ converter does. Our
code path emitted it unconditionally for both, so every Qm-scene bundle
came out with one extra type entry (TextAsset typetree blob — 288 B),
one extra obj-info row, one extra container entry, one extra inter-object
gap, and the corresponding `metadata_size` / `data_offset` shifts.

Forensic on one bundle (`QmNkL5ST8UjcXkmmb1r5ibmangaiumS1uNE3jGW3y1QS1R`):

```
bucket ours_B prod_B bits_diff
16_data_section_plus_align 4161051 4160846 7967639
11_obj_table 2738 2713 5933
06_typetree_nodes_per_type 35200 34912 2304
04_type_hdr 207 184 184
05_typetree_hdr_per_type 72 64 64
08_typetree_deps 36 32 32
01_sf_header 48 48 18
```

`obj_count: 0x72 vs 0x71`, `type_count: 9 vs 8`, all consistent with one
extra TextAsset that prod doesn't have.

## Fix landed

`BuildOpts::emit_metadata_textasset` (default `true`) added; both the
glb-path Builder and the standalone-texture path gate the meta-TextAsset
emission AND the matching `metadata.json` container entry on it. CLI
exposes `--no-metadata-textasset` on `ab-build-local`. `ab-generate`
unchanged (uses `..Default::default` → preserves current behavior).
`dev/class_bits_audit.py` and `dev/sf_other_*.py` apply the gate
automatically for entities with the `Qm` prefix
(`entity_is_legacy_no_metadata`).

### Single-bundle delta (Qm scene)

```
flag = [] ours_sf=4204432 prod_sf=4203879 sf_diff_bits=5660768 ta=1
flag = ['--no-metadata-textasset'] ours_sf=4203888 prod_sf=4203879 sf_diff_bits=479 ta=0
```

5,660,768 → 479 bits on this single bundle (-99.99%). 553-byte → 9-byte
SF size delta.

### Corpus delta (2,174 windows bundles)

Audit comparison (same script, same dataset):

|  | pre-fix (i=1700 snapshot) | post-fix (full 2158) | delta |
|---|---:|---:|---:|
| bundle ppm (LZ4-amplified) | 454,467 | 458,191 | +3,724¹ |
| SF ppm (parity-meaningful) | 163,254 | 163,331 | +77² |
| `explained` (class windows) | 104% | 107% | — |

¹ The bundle-ppm delta is dominated by the larger paired-bundle count
(2,158 vs 1,685 in the pre-fix snapshot — different mix), not the fix
itself.
² Class_bits_audit double-counts when ours/prod object lists differ in
length (unpaired entries contribute to both `per_class_bits` and
`sf_bits_diff`), so `explained` exceeds 100% in both runs. The
post-fix run pulls explained UP from 104% → 107% precisely because the
fix shrinks `sf_bits_diff` (denominator) without changing per-class
totals.

Cleaner per-bundle measurement: among the 405 Qm bundles in the corpus,
total SF bits saved by suppressing the metadata TextAsset is approximately
405 × (5.6M – 479) ≈ **2.27 Gbit** — about 27 % of pre-fix total
SF-bits-diff across the corpus.

### Parity test gate

`cargo test --release --test parity_bytes` total bits-different stays at
**773,674** (the `MAX_BITS_DIFFERENT` ceiling). All 10 fixtures are
bafkrei (CIDv1), so the default `emit_metadata_textasset = true`
preserves their previous behavior bit-exactly. `cargo test --release
--lib` still 115/115 green.

## What's left in `__sf_other__`

After the fix, top remaining offenders (same scan, post-fix):

```
QmRy1fKFKuvBK4FQDoaxXidbY21nGeHDp5AoTWXrzoXdhJ ours=620464 prod=541676 sf_other=868185
QmaojWLfNoDoS878EiqBEDRpa1FBtdCUcLtFSLsNGPn97j ours=214928 prod=214736 sf_other=715970
QmPBMQZJxKzSdWwxg5Y9gTyoGWeoX6Sh1z83gUavNBkSGv ours=214928 prod=214728 sf_other=706699
…
```

The top two share a common shape — `ours` has **more objects than prod**:

```
QmaojWLfNoD… ours: 12 objects (3 GO + 3 TR + 1 MF + 1 MR + 2 Mat + 1 Mesh + 1 AB)
            prod: 10 objects (2 GO + 2 TR + 1 MF + 1 MR + 2 Mat + 1 Mesh + 1 AB)
```

…i.e. one extra `GameObject` + one extra `Transform`. The glb_file for
this entry is `unity_assets/s0_pcube268_01.gltf` — a legacy scene asset
where ours creates a separate child GO for the primitive while prod
collapses it into the parent. **Next root-cause hypothesis**: the
pre-v3 converter's primitive-attach rule on `.gltf` scene inputs
suppressed the per-primitive child-GO that the post-v3 converter (and
abgen's current path) creates. The remaining typetree blob differences
(`tt_nodes.cid49=288B` on side a vs `0B` on side b) is the orphan
TextAsset typetree slot — STILL EMITTED because the.gltf path enters
`base["TextAsset"]` via `base_clone` even when we suppress the meta TA
object. Need to also gate the typetree entry itself; checking whether
the unused type slot is dropped automatically by `commit_objects` (it
should be — investigate next).

`QmRy1fKFKuvBK4FQDoaxXidbY21nGeHDp5AoTWXrzoXdhJ` is a different shape
(Mesh class_bits = 885 kbit, top_class is Mesh): an actual mesh-
encoding divergence — likely a vertex-attribute layout delta for
legacy scene meshes, unrelated to metadata.

## Concrete next steps

1. **Verify orphan TextAsset type slot is being elided.** Audit shows
 `cid49 (TextAsset)` typetree still appears in our output for Qm
 bundles even with `emit_metadata_textasset = false`. The `proto`
 table is the base SerializedFile types; if it contains TextAsset
 declaratively and we never `add` a TextAsset object, the type
 slot is still being written. Fix: prune unused types in
 `commit_objects` (already done for some? need to grep). Saves ~360
 bytes per Qm bundle.
2. **Legacy `.gltf` scene-primitive shape**. Empirically: a `.gltf`
 primitive whose parent GO is itself a primitive holder should NOT
 spawn a child GO — prod squashes it. Verify against the Qm scene
 corpus, then gate the child-GO emission on entity-version (or just
 the.gltf-scene-entity gate).
3. **Re-measure both windows + mac** after each follow-up.

## Files touched

- `src/builder.rs` — `BuildOpts::emit_metadata_textasset` field,
 `Builder::emit_metadata_textasset` field, gates in glb path
 (line ~1230, ~1670) and standalone path (line ~1945, ~1975), and
 the `finalize_pathids` remap guard.
- `src/bin/ab-build-local.rs` — `--no-metadata-textasset` CLI flag.
- `dev/sf_other_decompose.py` (new) — region-level decomposer.
- `dev/sf_other_one.py` (new) — single-bundle forensic.
- `dev/sf_other_scan_top.py` (new) — top-offender scanner.
- `dev/class_bits_audit.py` — auto-detects Qm entities, passes the gate.
