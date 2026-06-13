# Typetree common-string interning: AABB-at-offset-0 fix

`src/unity/typetree_node.rs::intern_string` was emitting `"AABB"` (offset 0 in
`COMMON_STRINGS`) as a local string literal in every typetree blob that
referenced it (every Mesh, SkinnedMeshRenderer, AnimationClip — 89.6 % of
typetree slots on windows). Root cause was a guard on the `Some(common_off)`
arm that excluded the legitimate zero-offset case:

```rust
// pre-fix
let off = match common_strings::offset_of(s) {
    Some(common_off) if common_off != 0 => common_off | 0x80000000,
    _ => { /* emit local copy of s */ }
};
```

`AABB` is the **first** entry in `COMMON_STRINGS`, so `offset_of("AABB") ==
Some(0)`. The `if common_off != 0` guard then fell through to the local-emit
branch, wasting 5 bytes per affected typetree and shifting every later
node's `name_str_offset` / `type_str_offset` by 5. Downstream this also
shifts the SerializedFile `metadata_size` / `file_size` / `data_offset`
header fields.

UnityPy's writer (`UnityPy/helpers/TypeTreeNode.py`) carries the **same**
bug (`if common_offset:` — Python's truthiness, falsy when `common_offset
== 0`). The python-abgen fork inherited it, so prior python-built bundles
also carry the redundant `"AABB\0"`. Real Unity bundles do not — which is
why the fix moves Rust toward Unity but away from python-abgen.

## The fix

```rust
// post-fix
let off = match common_strings::offset_of(s) {
    Some(common_off) => common_off | 0x80000000,
    None => { /* emit local copy of s */ }
};
```

## Measured impact

Parity gate (`tests/parity_bytes.rs`) over the 10-fixture reference
set (5 CIDs × {windows, mac}, all sourced from
`workdir/pathid_rt_v10_{windows,mac}/` so they reflect Unity output, not
python-abgen output):

| Fixture | pre-fix bits-diff | post-fix bits-diff | delta |
|---|---:|---:|---:|
| bafkreihfx3a6srd6q windows | 24 840 |  8 248 | **−16 592** |
| bafkreihfx3a6srd6q mac     | 24 808 |  8 225 | **−16 583** |
| bafkreif7fy5hinexy windows | 40 612 |     11 | **−40 601** |
| bafkreif7fy5hinexy mac     | 55 139 |     13 | **−55 126** |
| bafkreihbgn43gqc3k windows | 88 868 | 88 868 | 0 |
| bafkreihbgn43gqc3k mac     | 88 866 | 88 866 | 0 |
| bafkreie23rirhuqc6 windows |    520 |    520 | 0 |
| bafkreie23rirhuqc6 mac     |    494 |    494 | 0 |
| bafkreibxefote3jeu windows | 296 421 | 296 421 | 0 |
| bafkreibxefote3jeu mac     | 296 593 | 296 593 | 0 |
| **TOTAL** | **917 161** | **788 259** | **−128 902** |

`bafkreif7fy5hinexy` (both platforms) is now effectively bit-exact
(11 / 13 bits = only LZ4 window tie-breaks).

`MAX_BITS_DIFFERENT` lowered from `917_161` → `788_259` in the same commit.

## Earlier audit pessimism (now obsolete)

`dev/fix_proposals/windows_class_audit.md` ("Why the fix is NOT landed in
this branch") computed against the audit's base commit (`af10673`,
ceiling 1 978 445). That section concluded the fix was blocked unless
fixtures were regenerated, because on the OLDER fixture set the
post-fix bundle bits-different went up on 4 of 10 fixtures even though
SF-uncompressed bits dropped. That conclusion no longer holds — the
current fixtures (already Unity-built per `workdir/pathid_rt_v10_*`)
strictly improve on every fixture that changes.
