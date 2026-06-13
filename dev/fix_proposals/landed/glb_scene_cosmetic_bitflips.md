# glb-scene cosmetic bit-flip drill — root cause

Many glb-scene bundles have `|delta_bytes| <= 4` but `ppm` in the tens of
thousands. The sizes match; the bits differ across the same field shapes
("cosmetic divergence"). The work below identifies the single dominant pattern.

## Sample selection

From `/tmp/abgen-verify-test-windows-per-bundle.csv` (test corpus, 232
glb-scene bundles):

| Sample | ref | ours | delta | ppm |
|---|--:|--:|--:|--:|
| Low-ppm  `QmTrJkT3RhWbfy72nCudXAwN…` | 40,654 | 40,651 | -3 | 437 |
| Mid-ppm  `QmRtnHcD65NEvmwFzps5EyP7…` | 8,767  | 8,763  | -4 | 54,822 |
| High-ppm `QmcLrgC6HPBuQjvEHYDHLDh3…` | 8,759  | 8,755  | -4 | 93,647 |

All three live under entity `QmTVaReJESifeKYoWrNMHFcCyGDuCBgN3J3CbCdu1LyQLK`.

## 1. Object-set comparison

Built `examples/dump_bundle_objects.rs` (existing) and a new
`examples/diff_objects.rs` (per-object byte diff + typetree field-level diff
on divergent objects only) for the comparison. Both bundles in each pair
have **identical object count, identical PathID set, identical class
distribution, identical per-object data sizes**. Order matches. The
divergence is purely *inside* typetree field bytes.

| Sample | objs | PathIDs same | classes same | sizes same | divergent objs |
|---|--:|:--:|:--:|:--:|--:|
| Low-ppm  | 10 | yes | yes | yes | 1 (Transform) |
| Mid-ppm  | 11 | yes | yes | yes | 2 (AssetBundle, Transform) |
| High-ppm | 11 | yes | yes | yes | 5 (AssetBundle, Transform, 3× Material) |

No sub-asset emission order delta, no PathID rotation, no hash-slot
rotation. The "low-ppm" sample's full divergence is one bit at offset 31 of
a single Transform.

## 2. The recurring divergent field

Across all 3 samples, **every divergent object is one of**:

- `Transform.m_LocalPosition.x` — IEEE 754 sign bit (`-0.0` ours vs `+0.0`
 ref). One bit at object-byte offset 31. **All 3 samples have this.**
- `AssetBundle.m_PreloadTable[i].m_FileID` — externals slot index permuted
 (entries that ours puts at `fid=1` ref puts at `fid=2`, and vice versa).
 Cascades into `m_PathID` slots adjacent to the swap.
- `Material.m_Shader.m_FileID` and `Material.m_SavedProperties.m_TexEnvs[*]
.m_Texture.m_FileID` — same FileID swap, because the externals list
 itself is permuted (`SerializedFile.externals[0..2]` is `[shader,
 ext_tex]` ours vs `[ext_tex, shader]` ref on QmcLrgC6).

### Pattern frequency (120 small-delta glb-scene bundles)

`examples/diff_classify.rs` runs the classifier across all 120 glb-scene
bundles with `|delta_bytes| <= 8` and `bits_diff > 0`:

| Pattern | bundles affected | total events |
|---|--:|--:|
| **Transform `m_LocalPosition.{x|y|z}` -0 vs +0** | **118 / 120 (98.3 %)** | 119 |
| AssetBundle `m_PreloadTable` reorder | 90 / 120 (75 %) | 582 |
| Material `m_Shader.m_FileID` swap | 3 / 120 (2.5 %) | (cascades from externals) |
| Material `m_TexEnvs.m_FileID` swap | 8 / 120 (6.7 %) | (cascades from externals) |
| externals list swapped (shader fid 1↔2) | 8 / 120 (6.7 %) | — |
| Other class | 0 / 120 | 0 |

The AssetBundle PreloadTable changes are downstream of either (a) the
externals-list permutation (shader-slot FIRST/LAST already tracked in
`landed/assetbundle_shader_slot_rule_v2.md`), or (b) a within-run ordering
where prod sorts external entries in source-walk order rather than
ascending `(file_id, path_id)`. They are NOT the dominant root cause of the
cosmetic divergence.

**The single recurring pattern is `Transform.m_LocalPosition.x` `-0.0`.**

## 3. Why ours emits `-0.0`

Commit `683bd95`  *removed* the normalization in
`src/gltf.rs`:

```rust
// glTF→Unity basis flip on translation.x; preserve -0.0 sign bit (prod parity).
let tx = -t[0];
// let tx = if tx == 0.0 { 0.0 } else { tx }; // <- removed
```

The commit's rationale: "Prod does not collapse -0.0 to +0.0 after the
glTF→Unity basis-flip negation … Dropping the canonicalization yields ~16
net byte-identical bundle wins on v3." That measurement was taken against
the prod corpus prior to the upstream `localIdentifierInFile` patch.

After the upstream patch (asset-bundle-converter sets explicit
`localIdentifierInFile`) and the re-run of the converter against the test
corpus, prod's emitted bytes now **DO collapse -0.0 → +0.0** for nodes
that lack an explicit translation. Concrete check on QmTrJkT3RhWbfy…: the
source glTF has no `translation` field on either node (so the basis flip
turns the implicit `[0, 0, 0]` into `[-0, 0, 0]`), and ref emits the byte
`0x00` at object-offset 31 while ours emits `0x80`.

Net effect: the assumption the `683bd95` revert was built on inverted between corpora.
The fix is to **re-introduce the canonicalization** that `683bd95` removed.

## 4. Proposed fix

`src/gltf.rs`, in the per-node loop where `tx = -t[0]` is computed:

```rust
let tx = -t[0];
let tx = if tx == 0.0 { 0.0 } else { tx }; // canonicalize -0 → +0
nodes.push(Node {
    translation: [tx, t[1], t[2]],
    ...
});
```

This is the same one-line revert of `683bd95`. Per `landed/transform_signed_zero.md`,
the y/z, rotation, and scale paths must **not** be touched — the only
mechanical source of `-0.0` in this code is the basis-flip on x.

Per-fixture `max_ppm` caps in `tests/fixtures/parity/index.json` that were
raised by `683bd95` (specifically `bafkreihfx3a6srd6q…`) will lower again; the
re-baseline that already ran should be re-checked against this
revert.

## 5. Estimated ppm recovery

Direct contribution from this pattern: **119 single-bit flips across the
sampled 120-bundle small-delta subset** (one bit per affected Transform).
Each bit-flip costs 1 bit-diff, but the bit-flip is also the *only* reason
those bundles aren't byte-identical, so each flip turns a near-miss into
a confirmed byte-equal bundle (worth one entry in the `byte_identical`
counter, which is the harder gate to clear).

Extrapolating to the full 925-bundle glb-scene population (per README): if
the 98.3 % rate holds, roughly **900+ bundles carry this single-bit
residual**. The headline glb-scene `1,778 ppm` figure is dominated by the
30-ish high-ppm bundles; the long tail of single-bit bundles each
contributes very little to the bit-count, but the fix is essentially free
(one line) and the byte-identical gain compounds with the `683bd95` original
intent inverted.

**Conservative recovery estimate** on the test corpus (232 glb-scene
bundles): ~110 Transform-only bundles flip to byte-identical, dropping
the glb-scene byte-identical-bundle count by roughly that much against
ours. The ppm-bits delta is small (perhaps -30 to -80 ppm-bits on the
glb-scene kind alone, depending on bit accounting) but the
byte-identical metric — the harder one to move — gets the lift.

The 8-bundle externals-swap subset (6.7 %) is a separate, *known*
unresolvable problem already documented in
`landed/assetbundle_shader_slot_rule_v2.md` and handled by the
`expect_hash` opt-in retry path. No new fix proposed for it here — that
infrastructure already exists.

## Verification artifacts

- `examples/dump_bundle_objects.rs` — per-object PathID/class/size dump (existing).
- `examples/diff_objects.rs` — per-object byte-diff + typetree field-level
 diff for divergent objects (added during this investigation).
- `examples/dump_externals.rs` — externals list dump (added).
- `examples/dump_ab.rs` — AssetBundle `m_Dependencies / m_Container /
 m_PreloadTable` dump (added).
- `examples/diff_classify.rs` — bulk classifier, reads pair-paths from
 stdin, writes per-bundle pattern counts CSV (added).

All five build under `dcl-shell cargo build --release --example <name>`
and read both ours and ref bundles directly via `Bundle::load`.
