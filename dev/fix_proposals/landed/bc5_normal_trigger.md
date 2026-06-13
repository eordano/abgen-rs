# BC5 normal-map encoder — trigger DERIVED, encoder BLOCKED on Crunch

## Trigger criterion — DERIVED (16/16 confirm, 0 false positives in 90 bundles)

A glTF image gets prod's `TextureFormat.BC5` (Unity enum 29) + `lmf=3`
(normal-map flag) on its streamed Texture2D iff it is bound to BOTH:

1. A `material.normalTexture` slot in at least one material, AND
2. A `pbrMetallicRoughness.baseColorTexture` or `material.emissiveTexture`
 slot in at least one material.

I.e. an sRGB color image being reused as the normal-map input for some
material. Equivalent to the set complement of the existing
`scene.normal_images` (which holds `normal_uses − other_uses` — single-purpose
normal maps). The BC5 case is `normal_uses ∩ (baseColor ∪ emissive)`.

Verification: `/tmp/check_bc7_false_positives.py` scanned the windows corpus
(2174 bundles, 90 examined contain ≥1 normal-flagged Texture2D). 16 BC5+lmf=3
textures, 140 BC7+lmf=3 controls. Rule:

```
has_normal AND (has_baseColor OR has_emissive)
```

predicted 16 BC5 with 0 false positives and 0 false negatives. The rule is
purely a function of the glTF document, derivable at parse time — no
Unity-internal hints required.

## Encoder — BLOCKED on Crunch (CRN) compression

**The 16 prod BC5 textures are NOT raw BC5 — they are Crunch-compressed BC5.**

Verified by extracting `image_1` from
`bafkreihy2pqlk4…/bafybeibpzgr7zegsirmw5o5…_windows` at `m_StreamData.offset
= 39_861_808 length=372_764`:

```
first 8 bytes: 48 78 00 72 dc 0e 00 05 ("Hx…")
                ^^ ^^ Crunch header magic
```

Raw BC5 for a 1024×1024 image with 11 mip levels is 1_398_128 bytes; the
prod texture is only 372_764 bytes (3.75× smaller), exactly what RDO-based
Crunch achieves on BC5 data.

For comparison, prod's DXT1 textures
(`KHR_materials_specular.specularColorTexture`, landed at `88a827d`) are
**raw DXT1** — the leading bytes are direct BC1 endpoint pairs, not Crunch
magic. So Unity's TextureImporter chooses Crunch per-format independently —
"Compressed (Normal Map) High Quality" + Windows + BC5 = Crunched output,
while the specular-color DXT1 stays raw.

## Why shipping raw BC5 doesn't help parity

Per-bundle bits-diff (current state, BC7-raw fallback) for the 11 BC5-
containing bundles (`/tmp/measure_bc5_bundle_diff.py`):

```
TOTAL: ref_bits=1_376_957_488 diff_bits=712_138_613 ppm=517_182
```

Per-texture comparison on
`bafybeibpzgr7zegsirmw5o5…_windows` (representative):

| image   | prod                              | ours (today, BC7-raw fallback) |
|---------|-----------------------------------|--------------------------------|
| image_1 | fmt=29 cs=0 lmf=3 sz=372_764 (CRN) | fmt=25 cs=1 lmf=0 sz=1_398_128 |
| image_4 | fmt=29 cs=0 lmf=3 sz=361_070 (CRN) | fmt=25 cs=1 lmf=0 sz=1_398_128 |
| image_2 | fmt=25 cs=0 lmf=3 sz=1_398_128 (raw) | fmt=25 cs=0 lmf=3 sz=1_398_128 |
| image_5 | fmt=25 cs=0 lmf=3 sz=1_398_128 (raw) | fmt=25 cs=0 lmf=3 sz=1_398_128 |

The dominant cost is the **~1_025_000-byte (~8M-bit) size mismatch per BC5
texture** (1.4M raw vs 372k Crunched). Switching from BC7-raw to BC5-raw
would:

- Fix 3 metadata bytes per texture (fmt 25→29, cs 1→0, lmf 0→3) — saves
 ~24 bits × 16 textures = ~384 bits.
- Leave the resS bytes at 1.4M (raw BC5 chain is the same size as raw BC7
 chain) — size mismatch unchanged.
- Replace BC7 block bytes (random-ish vs CRN) with raw BC5 block bytes
 (also random-ish vs CRN) — net bit-XOR diff approximately unchanged.

**Net improvement of shipping raw BC5: ≤ 1 ppm on the affected bundles.**

The only meaningful reduction would come from emitting CRN-compressed BC5 to
match prod's stream length, which requires either:

1. Porting Crunch's RDO loop (≈3000 LOC C++; the only existing reference is
 `crunch-cpp` MIT, see [BinomialLLC/crunch](https://github.com/BinomialLLC/crunch)). High risk for a 16-texture corpus delta.
2. A pre-built `libcrn` FFI binding (none currently in nixpkgs; would need to
 package). Same delta.

## Decision

Document and exit. The trigger criterion is derivable and recorded here for
future use, but the encoder change is not worth shipping until either:

- A Crunch BC5 encoder is in scope (consider together with BC5Crunched
 format 64 if that ever shows up in corpus), OR
- Unity's loader is verified to accept raw BC5 in place of CRN-BC5 without
 user-visible regression — at which point we could ship raw BC5 + open a
 follow-up to size-down via CRN (but the parity ppm gain stays sub-1).

## Scripts retained

- `dev/scout_prod_dxt5_only.py` — prod format census (16 BC5 detection)
- `dev/scout_normal_subassets.py` — prod/ours per-bundle inventory
- `/tmp/verify_bc5_hypothesis.py` — 16/16 trigger confirmation
- `/tmp/check_bc7_false_positives.py` — 140 BC7-normal controls, 0 false hits
- `/tmp/probe_resS_bytes.py` — proves stream starts with `Hx` Crunch magic
- `/tmp/measure_bc5_bundle_diff.py` — per-bundle bits-diff baseline

## Cross-reference

- `dev/fix_proposals/normal_map_sub_asset_hypothesis.md` — original
 observation that 16 BC5 textures exist in corpus, deferred pending an
 abgen-rs BC5 encoder. This doc closes the deferment.
- `dev/fix_proposals/khr_materials_specular.md` — landed DXT1 streamed
 variant; note that DXT1 is emitted RAW by prod, not Crunched, which is
 what made that landing possible.
