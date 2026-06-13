# `__sf_other__` v3 — CIDv0 CAB hash uses lowercased bundle filename

Base: `438a255`. New tools:
`dev/sf_other_topn_bucket.py`, `dev/sf_other_tt_nodes_topn.py`,
`examples/cab_match.rs` (reverse-lookup which bundle filename produces a
given CAB hash). Test set: 2,174 windows bundles under
`workdir/pathid_rt_v10_windows/`.

## Decomposition on the new corpus (before this fix)

```
bucket bits_diff ppm_of_sf
16_data_section_plus_align 1,306,746,306 158,273.5
11_obj_table 656,153 79.5
13_externals 151,515 18.4
06_typetree_nodes_per_type 23,552 2.9
01_sf_header 9,427 1.1
07_typetree_string_buffer 7,056 0.9
10_obj_count 774 0.1
04_type_hdr 368 0.0
05_typetree_hdr_per_type 128 0.0
08_typetree_deps 64 0.0
03_type_count 2 0.0
```

(2,158 bundles paired; total SF bits = 8.26 Gbit; total SF bits-diff =
1.348 Gbit, 163,315 ppm — i.e. 99.5% of SF-diff is the data section.)

Way below the 5,000–7,000 ppm budget for non-data sub-regions —
the prior `sf_other_v2` work already absorbed the dominant legacy CIDv0
metadata-TextAsset contribution. The non-data sub-regions total **~103 ppm**
before this fix.

## Root cause for the `13_externals` sub-region — CAB hash on Qm bundles

`dev/sf_other_topn_bucket.py` with `ABGEN_TARGET_BUCKET=11_obj_table`
+ inspection of the `Qm…` scene corpus found the dominant non-data
divergence on CIDv0 bundles is the **CAB hash of sibling-bundle
filenames**. Forensic on `Qmbpmix9tidkYGmYng5U4v53cBQ5FLa4eAFNqoMFFQe4Uo`
(QmQhaA entity):

```
prod externals[1] path: archive:/CAB-47e301709dccb6ad1665b73c05a50b0f/...
ours externals[1] path: archive:/CAB-dafa19eed997a9ba1bbb08f593851276/...
```

The `m_deps` we walk is correct (`QmR9Q3YAAT5Bt2Jg4Xhg7corw2659AioZmm2x9BjJdZMQx_windows`).
Reverse-lookup via `cab_match`:

```
SpookyHash("QmR9Q3YAAT5Bt2Jg4Xhg7corw2659AioZmm2x9BjJdZMQx_windows")
 = dafa19eed997a9ba1bbb08f593851276 (ours)
SpookyHash("qmr9q3yaat5bt2jg4xhg7corw2659aiozmm2x9bjjdzmqx_windows")
 = 47e301709dccb6ad1665b73c05a50b0f (prod)
```

The pre-v3 converter (CIDv0 era) **lowercased the bundle filename before
hashing**. We were preserving the original mixed-case `Qm…` CID.

This affects the same bundle in three places:
1. The bundle's own CAB filename (`CAB-{hash(bundle_name)}`). Pre-fix
 ours = `CAB-93c2bddd5c944e316a65d781deaafcc0`; prod =
 `CAB-f83fe987589e22f6cc2eb8fdf0b6ef26` (= hash of lowercase).
2. The SerializedFile `externals[i].path` for each sibling content
 bundle referenced via `external_texture`.
3. The AssetBundle `m_Dependencies` entries (already lowercased after
 `cab_name(...).to_lowercase`, but the underlying hash was still
 wrong).

CIDv1 (`bafkrei…`/`bafybei…`) hashes are entirely lowercase ASCII, so
`to_ascii_lowercase` is a no-op for the v3+ corpus. Verified by
re-running the parity_bytes ceiling — unchanged.

## Fix landed

`src/cabname.rs::cab_hash` now lowercases the input before SpookyHash.
One-line change plus a regression test against the prod-observed CAB
filename for `Qmbpmix9tid…` and the prod-observed externals hash for
`QmR9Q3YAAT…`. All existing fixtures (`bafkrei…_linux` etc.) are
already lowercase so they pass unchanged.

```rust
pub fn cab_hash(bundle_name: &str) -> String {
    let lower = bundle_name.to_ascii_lowercase();
    let (h1, h2) = spooky_short(lower.as_bytes(), 0, 0);
    // …
}
```

## Corpus delta — windows (2,158 paired bundles)

|  | pre-fix | post-fix | delta |
|---|---:|---:|---:|
| `13_externals` (bits) | 151,515 | 104,459 | **−47,056 (−31%)** |
| `13_externals` (ppm of SF) | 18.4 | 12.7 | −5.7 |
| total SF bits-diff | 1,348,374,833 | 1,348,358,082 | −16,751 |
| `11_obj_table` | 656,153 | 656,153 | 0 (separate issue) |
| `01_sf_header` | 9,427 | 9,427 | 0 |

The 47K-bit reduction in `13_externals` is precisely the byte-window
sum of the 405 Qm bundles' sibling-CAB path strings now matching prod.
The remaining 104K bits in `13_externals` come from a different cause
— externals **slot ordering** on multi-external CIDv1 bundles (the
shader CAB ends up at index 1 in prod, not 0; verified against
`bafkreigkkiotu6ebimh4r3jknvzfzzksdfqsgpefxhqykmhojzrhj5ltkq_windows`,
same 7 hashes both sides, different positions). That's the next-level
fix (would need a per-target `ExternalsPosition`-style rule applied to
the SF externals list, not just the material PPtr run).

## What's NOT in scope for v3

* **`11_obj_table` (79.5 ppm)** — caused by either (a) 10 path_ids
 differing in Qm CIDv0 scene bundles (path-id allocation upstream)
 or (b) 2-extra-objects per Qm bundle from the.gltf primitive child-
 GO split (noted but not implemented in v2). Not an SF metadata
 formatting issue.
* **`06_typetree_nodes_per_type` (3 ppm, class 28 only)** — 2 bundles
 where ours is missing a Texture2D object that prod has (one
 bafy_xxx_windows wearable). Object-emission bug, not a typetree
 defaults bug.
* **`13_externals` remaining 104K bits** — multi-external slot
 ordering on CIDv1 bundles. Independent of CAB hashing.

## Parity test gate

* `cargo test --release --lib`: **117 passed** (was 116; +1 new
 `cab_hash_lowercases_input` regression test).
* `cargo test --release --test parity_bytes`: total bits-different
 unchanged at **773,032** (ceiling). All 10 parity fixtures are
 CIDv1, so the lowercase fix is a no-op for them by construction.

## Files touched

* `src/cabname.rs` — `cab_hash` lowercases input; new regression test.
* `dev/sf_other_decompose.py` — drop now-removed `--no-metadata-textasset`
 forwarding (subsumed by 9d33fdc's auto-detect via `root_hash`).
* `dev/sf_other_topn_bucket.py` (new) — per-sub-region top-offender
 scanner.
* `dev/sf_other_tt_nodes_topn.py` (new) — typetree-nodes top-offender
 with per-cid breakdown.
* `examples/cab_match.rs` (new) — reverse-lookup which bundle filename
 hashes to a given CAB. Two modes: `scan <hex> <ent_dir>` and
 `one <name>…`.
