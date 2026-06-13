# glb-scene ±1..±4 byte size class — re-confirmation at scale (val300)

> **Status: NEGATIVE, re-confirmed with hard raw-size evidence on all 925
> bundles.** The dominant glb-scene size class (925 bundles with
> `0 < |on-disk Δ| ≤ 4`) carries **zero structural inserts/deletes**. Every one
> of the 925 decompresses to a byte-for-byte *size-identical* node set; the
> on-disk ±1..±4 is pure LZ4HC frame-length noise downstream of an
> already-known in-place **bit-value** divergence. No code change.

This re-runs the `glb_scene_size_session.md` investigation against the newer
`ad0564d-val300-windows` reference corpus (vs. the older test corpus the prior
pass used) and replaces the prior sampled evidence with a **full-population**
decompress-and-compare over all 925 bundles.

## Population (val300, kind=glb-scene)

```
glb-scene total                3194  (1288 byte-identical, ppm 297375)
glb-scene 0 < |disk Δ| ≤ 4       925   <- this size class
```

On-disk delta histogram (signed, ours − ref):

```
-4: 60   -3: 49   -2: 100   -1: 204   +1: 320   +2: 136   +3: 42   +4: 14
```

(+1 and −1 alone = 524 / 925.)

## SOURCE — what produces the ±1..±4

**LZ4HC compressed-frame-length noise.** The decompressed payload is identical
in length between ours and ref; the compressor merely emits a frame 1..4 bytes
longer/shorter because the *content* of an already-divergent block changed
(an in-place bit/byte swap, never an insert). `size_delta_v2.md` already proved
our LZ4HC frame encoder is byte-exact with the reference's, so a
same-decompressed-size pair that differs by ±1..±4 on disk is *definitionally*
the compressor reacting to a content difference, not a length difference.

The underlying content (bit-value) divergence that the compressor reacts to is,
across the 925:

| Underlying divergence (per-bundle, exclusive combos) | bundles |
|---|--:|
| AssetBundle `m_PreloadTable` PPtr ordering only | 666 |
| `m_PreloadTable` + Material `m_TexEnvs.m_FileID` swap | 125 |
| `m_PreloadTable` + Material `m_Shader.m_FileID` + `m_TexEnvs` swap | 124 |
| `m_PreloadTable` + Transform other-field | 2 |
| only `.resS` (mesh/BC7) content differs, object typetrees identical | 8 |

`m_PreloadTable` ordering is present in **917 / 925** bundles; the externals
list is permuted (`shader`/`ext_tex` slot transposition) in **249 / 925**, which
cascades into the Material `m_FileID` swaps. **Transform `-0.0` is now 0/925** —
the canonicalization fix from `landed/glb_scene_cosmetic_bitflips.md` has
landed, so the dominant pattern on val300 has shifted from the Transform sign
bit (old test corpus) to the m_PreloadTable PPtr ordering wall.

All of these are **known, documented, non-recoverable bit-value walls**:
m_PreloadTable / external-slot ordering is governed by the converter's
build-time InstanceID allocation (`landed/assetbundle_shader_slot_rule_v2.md`,
`PARITY_STATUS.md`); the `.resS` cases are real-mesh / real-BC7 content noise.

## Recoverable vs irreducible — EVIDENCE

**Irreducible (no recoverable structural component).**

Hard raw-size evidence, **full population, not a sample**:

1. **Decompressed node-set + node-size identity — 925 / 925.**
   `dump_decomp` on both ours and ref, comparing the set of decompressed
   UnityFS dir-nodes (CAB + any `.resS`) and each node's byte length:
   **all 925 bundles: identical node set, every node identical in size
   (raw Δ = 0).** Zero structural inserts/deletes anywhere in the size class.
   (`/tmp/classify_val300.py`, 925/925 `RAW_IDENTICAL`.)

2. **The diff is in-place, not length.** Where content differs it is a
   same-length byte swap inside one block:

   | example | disk Δ | decompressed CAB | diff bytes |
   |---|--:|--:|--:|
   | `bafkreia2cd…/bafkreihjld…` (+3) | +3 | 32660 = 32660 | 43 |
   | `QmTVaRe…/Qmct7zF…` (+1) | +1 | 25192 = 25192 | 18 |

3. **Where it lands (CAB vs .resS), full population:**

   ```
   CAB-only content diff:  919
   .resS-only content diff:  3
   both CAB and .resS:       3
   pure frame noise (0 content diff): 0
   ```

   The 6 `.resS`-touching cases are mesh-vertex-stream / BC7-texel content that
   happens to be the same size but different bytes (e.g. 4684 / 760 diff bytes
   in a same-size `.resS`) — content noise, explicitly out of scope and not a
   string/varint/alignment insert. The 3 `objs_diff==0` cases have *identical*
   object typetrees and an identical-size differing `.resS`: confirms the
   on-disk delta there is downstream of mesh/texture content, with no object
   re-layout at all.

There is **no variable-length name/path written one char off, no varint count
off-by-one, and no 4/8/16-byte alignment pad** anywhere in the 925. If any
such structural source existed it would show as a non-zero decompressed
node-size delta; the sweep finds exactly zero.

## Fix proposal

**None for the size axis** — it is fully explained by non-recoverable
bit-value walls plus content noise, with zero structural component to fix. The
remaining bundle-count in this class is gated entirely by:

- m_PreloadTable / external-slot PPtr ordering (917/925) — non-recoverable
  per `landed/assetbundle_shader_slot_rule_v2.md`; the `expect_hash` retry path
  already exists for the 249 externals-swap subset.
- `.resS` mesh/BC7 content drift (6/925) — the shared content long-tail, a
  separate bit-value axis.

Recommendation: keep this size class closed; do not re-drill. Direct effort to
a true structural axis (decompressed node sizes that actually differ — e.g.
mesh vertex-stream length on glb-wearable).

## FRACTION of the 925: noise vs structural

```
structural (decompressed length differs):    0 / 925   (0.0 %)
compression-frame noise downstream of a
  known in-place bit-value diff:           925 / 925   (100 %)
    ├ CAB-only (m_PreloadTable / FileID swap): 919
    └ touches .resS (mesh/BC7 content):          6
```

**100 % noise, 0 % structural.** The Area-#3 premise (a systematic structural
±byte) is refuted at full population scale.

## Verification

- `examples/dump_decomp` (prebuilt) — per-node decompression.
- `examples/diff_classify` (rebuilt this session) — typetree field-level
  attribution; 925/925 classified, 0 skips.
- `/tmp/classify_val300.py` — full-population raw-size classifier (925/925
  RAW_IDENTICAL).
- `/tmp/resS_split.py` — CAB-vs-.resS content-diff split.
- `/tmp/classify.csv` — per-bundle pattern attribution.
- No source change; parity gate untouched (working tree clean at HEAD
  `68181ba`).
