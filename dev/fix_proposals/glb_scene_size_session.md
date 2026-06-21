# glb-scene structural size parity (RESEARCH_AREAS #3) — negative finding

> **Status: negative.** The Area #3 hypothesis — that the dominant glb-scene
> ±1..±4 byte size cluster is a single systematic *structural* insert/delete
> (variable-length string written one char different, a varint count, or
> 4/8/16-byte alignment padding at an object boundary) — is **refuted**. The
> tiny size deltas are LZ4HC compressed-size noise downstream of already-known,
> already-documented *bit-value* divergences (m_PreloadTable PPtr ordering +
> BC7/float-LSB), not any structural insert. No code change landed.

## Baseline (windows test-set, kind=glb-scene)

```
kind        bundles  byte-id  smaller  larger  pair-bits      diff-bits     ppm
glb-scene      1464      398      551     270  3828479032  1327621126  346775.1
```

Size triage of the 1464 glb-scene bundles:

| group | count |
|---|--:|
| size-identical (decompressed AND on-disk delta = 0) | 643 (398 byte-identical, 245 same-size/bits-differ) |
| size-mismatched on disk | 821 |
| └ \|disk Δ\| ≤ 4 | 378 |
| └ 5 ≤ \|disk Δ\| ≤ 4096 | 316 |
| └ \|disk Δ\| > 4096 | 127 |

Small-delta on-disk histogram (the cluster Area #3 targets):
`-1: 104, +1: 121, -2: 34, +2: 45, -3: 32, +3: 14, -4: 22, +4: 6` —
+1 and -1 alone are 225 bundles.

## Method

For every size-mismatched glb-scene bundle I decompressed both ours and ref to
their raw UnityFS dir-nodes (CAB + any .resS) with `examples/dump_decomp` and
compared **decompressed** sizes (the structural truth, free of LZ4 amplification):

- **378 / 378** small-delta (\|disk Δ\| ≤ 4) bundles: decompressed CAB
  **byte-identical in size** (delta = 0). Zero structural inserts.
- **306 / 316** mid-delta (5..4096) bundles: decompressed CAB same size.
- **10 / 316** mid-delta bundles: CAB size differs — but by ±174752 / ±43680 /
  ±3008 / ±1392 (full mip-pyramids / BC7 blocks), *not* a ±1-string insert.
  These are BC7 .resS content, not a structural string/varint/alignment bug.
- **127** big-delta (>4096) bundles: in this rebaselined corpus 126/127 are
  **ours-smaller**, the opposite sign of `glb_scene_large_outliers.md`. The BC7
  placeholder rule already landed; the residual is real-BC7-vs-real-BC7 RDO
  quality drift on high-mip textures (a bit-value axis, out of scope here).

**Net: across 684 of the 821 size-mismatched bundles the decompressed CAB is
byte-identical in length. There is no systematic structural insert/delete in
the glb-scene size cluster.**

## Root cause of the ±1..±4 disk deltas (worked example)

`bafkreiexf46ljdv…/bafkreiazxt6hvoh7faus5szlw56du6usgboofct45zljhkkgc45wjqat3q_windows`
(ours 8370, ref 8369, +1 byte, only 720 diff-bits — a near-clean miss):

- Decompressed CABs are both exactly 25460 bytes.
- `cmp -l` shows 18 differing bytes in one window (~offset 24537), a clean
  swap of two 8-byte runs.
- `examples/dump_ab` localizes it to **`AssetBundle.m_PreloadTable` ordering**:

  ```
  ours: [1] fid=1 pid=7645288030342540701   (external shader)
        [2] fid=0 pid=-1596252151501307853  (material_0)
  ref:  [1] fid=0 pid=-1596252151501307853  (material_0)
        [2] fid=1 pid=7645288030342540701   (external shader)
  ```

  The material PPtr and the external-shader PPtr are transposed in the preload
  list. This is exactly the **m_PreloadTable PPtr ordering wall** proven
  non-recoverable in `PARITY_STATUS.md` /
  `landed/assetbundle_shader_slot_rule_v2.md` (governed by the converter's
  build-time InstanceID allocation, content-indistinguishable, Windows↔Mac label
  agreement 53.4%). The +1 disk byte is pure LZ4HC re-compression noise from
  the transposed bytes; the decompressed payload is identical in length.

The remaining small-delta bundles split between this same preload transposition
and `Transform.m_LocalPosition` sign-bit / material color float-LSB flips —
all same-decompressed-size, all already documented in
`landed/glb_scene_cosmetic_bitflips.md` and `landed/transform_signed_zero.md`.

## Why Area #3's premise inverted

`size_delta_v2.md` already proved LZ4HC is byte-exact between our compressor and
the converter's, so a same-decompressed-size pair that differs by ±1..±4 on disk *must*
be the compressor reacting to a content (bit) difference, not a length
difference. Area #3 assumed the small disk deltas implied a structural
insert; the decompression sweep shows they do not. The structural size axis for
glb-scene is effectively closed: 643 bundles already size-match, and every
remaining on-disk size delta resolves to a known bit-value wall (preload PPtr
order) or known content drift (BC7 RDO on high-mip textures), neither of which
is a string/varint/alignment insert.

## Verification

- Baseline build: `abgen-corpus --from-reference …/ad0564d-windows` (ABGEN_ROOT
  must point at `<repo-root>` so the template bundle
  resolves from a worktree).
- `cargo test --release --test parity_bytes` → 2 passed (gate green; no code
  change made).

## Recommendation

Do not re-drill glb-scene structural size. The bundle-count wins remaining in
this kind are bit-value (preload ordering — non-recoverable; BC7 RDO — the
shared BC7 long-tail). Mark RESEARCH_AREAS #3 as resolved-negative and redirect
effort to a true structural axis (e.g. #4 mesh vertex-stream length on
glb-wearable, where decompressed sizes *do* differ).

## Artifacts

- `/tmp/classify_decomp.py` — small-delta (\|Δ\|≤4) decompressed-size classifier.
- `/tmp/classify_decomp2.py` — mid-delta (5..4096) classifier (CAB vs .resS).
- `examples/dump_decomp`, `examples/dump_ab` — used for the worked example.
