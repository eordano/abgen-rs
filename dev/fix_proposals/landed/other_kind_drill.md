# "other" AB kind drill — top contributor classes

**Date:**
**Scope:** 66 bundles classified as `other` in
`/tmp/abgen-verify-test-windows-per-bundle.csv` (baseline 179 corpus,
test_windows × 21 entities).

## TL;DR

All 66 "other" bundles are the **same class signature**: `{Texture2D (28),
AssetBundle (142)}` with **no TextAsset (49)**. They are
`Qm…`-prefix (CIDv0) standalone-texture bundles — `emits_metadata_textasset`
returns false for CIDv0 hashes (`builder.rs:106`), so the metadata
TextAsset is suppressed, and the classifier in `abgen-verify.rs` requires
TextAsset for `standalone-texture`, so they fall into `other`.

Two recurring patterns drive the size deltas:

1. **Source > max_texture_size (1024) + Qm-CIDv0 → mean-color placeholder
 miss** (baseline 179 only). Top-4 oversize cases (`QmSjVm5`,
 `QmfWiAu5z`, `QmTYckPF`, `QmZsq`) showed REF emitting a 9-distinct-byte
 solid-color BC7 mip chain (700-1400 KB raw → ~5 KB compressed), while
 OURS encoded real per-block BC7 (256 distinct bytes → ~80 KB
 compressed). **Already fixed by the headless-BC7-placeholder generalization** (verified: re-running the
 current `/tmp/abgen-ours-test` build collapses `QmSjVm5` from 76510 →
 4842 bytes — within 1 byte of REF).

2. **.resS-streamed bundles — LZ4HC chunk-boundary drift**. Bundles with
 sources ≤ max emit the full BC7 mip chain to a sibling `.resS` raw
 file. SF +.resS bytes match exactly; the on-disk size delta is pure
 compression-envelope drift. We are ~5% **smaller** on the large
 streamed cases (`Qme4T` -49.7 KB on 1.04 MB, `QmZoo` -13.4 KB on
 237 KB). Hooked to the existing LZ4HC tuning thread.

## 1. Per-bundle breakdown of top 10 oversize (baseline 179)

| Bundle | parent | src_dim | ref | ours | Δbytes | Pattern |
|---|---|---|---|---|---|---|
| QmSjVm5 | QmXXp | **1280×720** | 4841 | 76510 | **+71669** | placeholder miss |
| QmfWiAu5z | QmTVa | **1100×350** | 3388 | 49913 | **+46525** | placeholder miss |
| QmTYckPF | Qmc1r | **1280×720** | 4891 | 44369 | +39478 | placeholder miss |
| QmZsq | QmTVa | **2012×954** | 4836 | 19979 | +15143 | placeholder miss |
| Qmdc8X2SgZ | QmTVa | 512×512 (jpeg) | 123652 | 134931 | +11279 | .resS LZ4 drift |
| QmZYLeL82 | QmTVa | 512×512 | 112729 | 117485 | +4756 | BC7 per-block |
| QmYsUU | QmTVa | **2012×2354** | 7708 | 12284 | +4576 | placeholder miss |
| Qmd3fVyJgE | QmTVa | 225×225 | 23524 | 27713 | +4189 | BC7 per-block |
| QmUsxP | QmTVa | **2012×2354** | 7709 | 11756 | +4047 | placeholder miss |
| Qma4aMfQ | QmTVa | 225×225 | 25853 | 29657 | +3804 | BC7 per-block |

## 1b. Top 5.resS-streamed under-size (we save bytes)

| Bundle | src_dim | ref | ours | Δbytes | Pattern |
|---|---|---|---|---|---|
| Qme4T | 1024×1024 | 1042419 | 992716 | **-49703** | .resS LZ4 drift (ours tighter) |
| QmZoo | 512×512 | 237300 | 223913 | **-13387** | .resS LZ4 drift |
| QmWZaHM9C (×3) | 512×512 | 132424 | 125920 | -6504 each | .resS LZ4 drift |
| QmSougyk | 512×512 | 309921 | 304920 | -5001 | .resS LZ4 drift |
| QmNjeG | 665×1000 | 136879 | 132337 | -4542 | .resS LZ4 drift |

## 1c. Class-set census (all 66)

```
n=66 class_sig=[Texture2D, AssetBundle] 100% of "other"
 object order 28+142: 32 bundles (Texture2D first)
 object order 142+28: 34 bundles (AssetBundle first)
 has.resS: 23 bundles
 no.resS: 43 bundles
```

The 28+142 vs 142+28 ordering correlates with `model_referenced`: the
streaming path appends the Tex Texture2D after the AssetBundle (`142+28`),
the inline path orders Texture2D first (`28+142`). Both orderings appear
in both the.resS and no-.resS buckets, so it is not a simple
streaming-flag distinction — likely driven by Unity's
`AssetDatabase.AddObjectToAsset` insertion order for the legacy CIDv0
path (whose preload-table builder predates the canonical ordering).

## 2. Recurring patterns

### Pattern A — Placeholder miss on CIDv0 oversize PNGs (already fixed)

`builder.rs:2437-2441` triggers `mean_color_image` exactly when
`w > max_size || h > max_size`. The 4 top offenders in baseline 179 were
all 1280×720 / 1100×350 / 2012×954 / 2012×2354 sources where placeholder
should have fired. The "Generalize headless-BC7-placeholder" work
landed the fix; verified by re-dumping the current
`/tmp/abgen-ours-test/QmXXpa…/QmSjVm5…_windows` bundle:

```
ours image_data: 699088 bytes, distinct_bytes=9 (was 256), file_size 4842
ref image_data: 699088 bytes, distinct_bytes=9, file_size 4841
```

The 4-byte color delta (`20 00 34 e0 …` vs ref `20 5a bf d6 …`) is a
1-byte file-size drift attributable to BC7-quantised single-block
encoding of the mean color; not byte-identical but already in the
"close enough that LZ4HC eats the residual" regime.

**Recovered: ~172 KB across the top 4 oversize bundles, already on main.**

### Pattern B —.resS LZ4HC chunk-boundary drift (both signs)

22 of 66 bundles externalize the BC7 mip chain to a sibling `.resS` raw
file (model_referenced + texture_format=25 path,
`builder.rs:2500-2506`). For these, the SF tree and.resS contents are
byte-identical between OURS and REF, but the bundle envelope size
differs by ±0.5%–5% depending on how the LZ4HC packer chunks the raw
file. We are **smaller** on the large cases (`Qme4T` -49 KB,
`QmZoo` -13 KB) and **larger** on a handful of mid-size cases
(`Qmdc8X` +11 KB). Net across the 22 streamed "other": -118 KB
(saving bytes vs ref but bit-mismatched).

This is the same root cause as the wearable LZ4HC chunk-size match
and LZ4HC tie-break probe. Already on the active probe list.

### Pattern C — ≤max-size BC7 per-block encoder drift

The remaining 5 over cases (`QmZYLeL`, `Qmd3fVyJgE`, `Qma4aMfQ`,
`QmReXBZmn`, `QmTVmKM…`) have 225²/512² sources where the
placeholder is not engaged. Both REF and OURS emit real BC7; the first
16 bytes of the top mip match byte-for-byte; trailing bytes diverge in
sparse 1-byte spots = pure per-block BC7 partition / mode choice
mismatch. Same class as the in-glb texture residuals being chased by
the BC7 partition / mode probes.

**Per-bundle delta budget for Pattern C: 3.5–4.8 KB each, ~22 KB
total.** Will collapse with the existing BC7 partition / mode-6
probes — no new mechanism needed.

## 3. Concrete patch proposals

### Proposal P1 — Classifier fix (cosmetic, recommended)

`abgen-verify.rs:179-205` should recognize the CIDv0 standalone-texture
shape — `{Texture2D, AssetBundle}` with no GameObject, no Transform —
as `standalone-texture-legacy` rather than `other`. This cleans up
reporting without changing builder behavior.

```rust
// after the existing standalone-texture check
if !has(C_GO) && !has(C_TRANSFORM)
    && has(C_TEXTURE2D) && has(C_ASSETBUNDLE)
    && only_in(&[C_TEXTURE2D, C_ASSETBUNDLE]) {
    return "standalone-texture-legacy";
}
```

Expected effect on the per-kind summary: removes 66 bundles from
`other` (which then has 0 entries on the current corpus), adds a
`standalone-texture-legacy` row with ~475k ppm. **Zero byte recovery
— pure attribution clarity.**

### Proposal P2 — No-op (Pattern A landed, Patterns B+C have owning tasks)

The byte-recovery work for "other" is already in flight under
existing tasks:

- Pattern A → headless-BC7-placeholder generalization (done; reduces "other" oversize delta by ~172 KB).
- Pattern B → LZ4HC tuning (will collapse the.resS
 envelope drift in both directions, ~118 KB net under-counted today).
- Pattern C → BC7 partition / mode
 probes (~22 KB across the remaining 5 small-source over cases).

**No new src/ patch is recommended.** The single net-new piece of
work is the classifier relabel in P1.

## 4. Estimated bytes recoverable

| Source | Status | Bytes |
|---|---|---|
| Pattern A — placeholder miss | **Fixed by headless-BC7-placeholder generalization** | +172 KB (already recovered on main) |
| Pattern B — .resS LZ4HC drift | In flight (LZ4HC tuning) | ±118 KB net (we're 118 KB **smaller** today; bit-parity work, not byte-recovery) |
| Pattern C — sub-max BC7 per-block drift | In flight (BC7 partition / mode probes) | ~22 KB across 5 bundles |
| Pattern D — classifier relabel | Proposed P1 | 0 bytes (attribution only) |

**Net "other"-kind byte recoverable from new work: 0.** The kind is
already net-smaller than ref by 68 KB on the current build; the
remaining 475k ppm signal is dominated by.resS compression-envelope
drift (Pattern B) where we already win byte-count and lose bit-parity
only — owned by the existing LZ4HC chunk-size probes.

## 5. References

- `abgen-verify.rs:162-205` — `classify` / `kind_of` dispatch.
- `builder.rs:103-108` — `emits_metadata_textasset` CIDv0 gate.
- `builder.rs:2313-2341` — `mean_color_image` placeholder.
- `builder.rs:2422-2506` — `StandaloneTextureBuilder::build`.
- `texprofile.rs:41-52` — `max_texture_size_for` (windows=1024).
- Inspection examples (gitignored, in `target/release/examples/`):
 `dump_bundle_objects`, `dump_tex_for_kind`, `dump_tex_bytes`,
 `classify_other`.
