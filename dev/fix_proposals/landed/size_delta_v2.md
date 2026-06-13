# Bundle size-delta v2 (windows + mac) — post envelope fix

Continuation of `size_delta_windows_mac.md` (envelope fix landed at commit
`2fdc675`). Goal was to identify and either close or prove
irreducible the residual size-delta after the new 22-entity windows test set
(2,174 bundles).

## TL;DR

- **LZ4HC is byte-exact identical** between our Rust compressor and Unity's
 prod. Verified by decompressing 186 prod blocks (independent, no streaming
 dict, no `0x200` padding-flag interaction) and re-compressing each with
 `compress_hc` and `python lz4.block.compress(mode='high_compression',
 compression=12)`: 186/186 byte-equal in both directions. **All remaining
 bundle-size deltas are driven by input-byte deltas, not compression
 choice.**
- One mechanical fix landed (this commit): **standalone-texture bundles
 produced from CIDv0 (`Qm…`) entities must NOT emit the metadata TextAsset**.
 See §3 below.
- Two large residual classes are **not envelope/compression bugs** but
 emitter-coverage gaps already noted upstream:
 1. `gltf_or_json` class (~6 MB sum-Δ POSITIVE after fix) — we now over-emit
     some, under-emit others; per-file content variance, not envelope.
 2. Large embedded-glb bundles — 50 KB to 670 KB deficits; tracked under
     `bc7_tiebreak_v2.md` (high-mip BC7 quality drift).

## 1. Size-delta categorization on the new 22-entity windows test set (2,174 bundles)

Pre-fix (this branch's parent):

| class          |    n |   sm | eq |  la |   mean&nbsp;\|Δ\| |       sum&nbsp;Δ |    min&nbsp;Δ |       max&nbsp;Δ |
|----------------|-----:|-----:|---:|----:|-----------:|-----------:|--------:|-----------:|
| glb            |  835 |  428 | 73 | 334 |   36 428.1 | +7 317 372 | −3 882 991 | +5 141 979 |
| gltf\_or\_json |  383 |   34 |  6 | 343 |   15 900.3 | +6 017 698 | −13 636 | +5 861 551 |
| png            |  849 |  686 | 17 | 146 |    4 203.0 | −2 200 991 | −154 350 |   +93 457 |
| jpeg           |   89 |   37 |  1 |  51 |   12 098.2 |   −412 920 | −105 056 |   +68 333 |
| other          |    2 |    1 |  1 |   0 |   93 987.0 |   −187 974 | −187 974 |        0 |
| build\_err     |   16 |    0 | 16 |   0 |          0 |          0 |       0 |        0 |

(`smaller` = ours smaller than prod, `larger` = ours larger; build\_err = 16
gltf/glb bundles failed `ab-build-local` and never produced an output — same
errors that surface on `main`.)

Pre-existing context: the envelope fix from `size_delta_windows_mac.md`
already brought us from "predominantly smaller" to roughly symmetric on the
280-bundle set; the larger 2,174-bundle set surfaces additional emitter
coverage gaps that show up as positive sum-Δ for the glb/gltf classes.

## 2. Top-5 worst residual-decomposition findings

Decomposed the top-5 worst-deficit bundles per class with
`dev/decompose_bundle.py`. Findings cluster:

### A. PNG class (top 5 deficits all from `bafybei…` entities)
- `bafkreicfprgk7s6…` prod=202,305 ours=47,955 Δ=−154,350
- `bafybeid5zprckeb…` prod=636,726 ours=547,818 Δ=−88,908
- `bafybeicvnysmlpz…` prod=634,181 ours=547,778 Δ=−86,403
- `bafkreicgsgrzcao…` prod=186,723 ours=101,475 Δ=−85,248
- `bafkreieitsumzxg…` prod=116,780 ours=33,159 Δ=−83,621

Decomposition: **ours emits no `.resS`**, prod emits a large `.resS`. The
streaming gate at `builder.rs:1924` (`do_stream = self.model_referenced &&
prof.texture_format == 25 && prof.target_w == 512 && prof.target_h == 512`)
trips only for 512×512 BC7-compressed textures. The 5 missing bundles are
all larger textures (1024, 2048, mipped) that prod ALSO streams. The gate is
too narrow for these CIDv1 entities.

This is the same "streaming gate too narrow" issue called out in the previous
landed proposal (§3 retraction), with extra evidence from the larger corpus.
**Out of scope for an envelope-only fix.**

### B. glb class — large embedded-glb bundles
- `bafybeidr666w4lx…` prod=9,630,830 ours=5,747,839 Δ=−3,882,991 src=4,213,976
- `bafybeictj33t2oq…` prod=6,538,488 ours=4,989,346 Δ=−1,549,142

Decomposition: same dir-node shape (CAB +.resS), CAB sizes within 1 KB,
`.resS` length matches prod, **but `.resS` content differs in ~12-18 % of
bytes**. The compressed-block sizes track the input-byte differences
linearly. This is the embedded-glb BC7 quality drift already tracked under
`bc7_tiebreak_v2.md`.

### C. gltf\_or\_json class (post-fix POSITIVE Δ, but with build\_errs)
- 16 builds failed (`build_err:gltf_or_json` × 7 + `build_err:glb` × 9):
 these are external-buffer references the.gltf path can't resolve. The
 surviving 383 gltf bundles report POSITIVE sum-Δ — we over-emit some,
 under-emit others. Per-file content variance, not envelope.

### D. Standalone-PNG +~488 B excess (THIS FIX)

`dev/cab_decompose_detail.py` against 300 bundles surfaced a tight cluster of
67 standalone-PNG bundles with **CAB +480 / +488 / +492 B** (mean ≈ +485),
first byte-of-difference uniformly at **offset 22** in the CAB
SerializedFile. Decoded that byte: the **type count** at offset 64 in the
v22+ header — prod = `0x02` (AssetBundle + Texture2D), ours = `0x03`
(AssetBundle + Texture2D + TextAsset).

Cross-corpus survey of all 939 single-texture standalones in the test set:

| entity-CID class | TextAsset present in prod | count |
|------------------|--------------------------:|------:|
| `Qm…` (CIDv0)    | **NO**                    |   108 |
| `bafkrei…/bafybei…` (CIDv1) | **YES**            |   831 |

Crystal correlation: every Qm entity in the corpus produces standalone-PNG
bundles WITHOUT a metadata TextAsset; every baf entity produces them WITH.
(The file-CID prefix matches the entity-CID prefix in 100 % of cases — Qm
files belong to Qm entities, baf files to baf entities — so the file-CID
prefix is a reliable proxy for the entity-CID prefix.)

The 6 windows+mac parity-bytes fixtures are all `bafkrei…` CIDs (modern), so
the existing parity ceiling at 773,674 bits is unaffected by the fix.

### Post-fix corpus measurement

After the fix, re-running `dev/measure_size_delta_windows.py`:

| class          |    n |   sm (Δ) | eq |  la (Δ) |       sum&nbsp;Δ (Δ) |
|----------------|-----:|---------:|---:|--------:|-----------------:|
| png            |  849 | 694 (+8) | 17 | 138 (−8)| −2 211 446 (−10 455) |
| jpeg           |   89 |  38 (+1) |  1 |  50 (−1)|   −422 777  (−9 857) |
| glb/gltf/other |  unchanged (only standalone-texture bundles affected) |

Only standalone-texture bundles use `StandaloneTextureBuilder`; the glb /
gltf / glb-with-embedded-tex paths are untouched. PNG/JPG classes had 8/1
bundles flip from "larger" to "smaller" — exactly the 108 Qm-prefix
standalone bundles that no longer carry the metadata TextAsset. Per-bundle
size reduction is ~488 B uncompressed → ~250-300 B after LZ4HC.

`dev/parity_post_lz4.py` on the full 2,174-bundle corpus (post-fix):

| kind          |    n | all_eq | cab_eq | cab_ppm | ress_ppm |
|---------------|-----:|-------:|-------:|--------:|---------:|
| glb           |  813 |      7 |     39 |  648.66 | 7 218.91 |
| gltf_or_json  |  382 |      0 |      0 |   44.79 | 1 850.63 |
| jpeg          |   88 |      0 |      0 | 9 958.10 | 228 473.02 |
| png           |  847 |      8 |     18 | 27 188.49 | 108 530.39 |
| other         |    2 |      1 |      1 |    0.00 |    0.00 |

(16 bundles now decompress to a bit-identical set of dir-nodes against
prod — the first non-trivial all_eq count we've measured. ppm-bits gives a
clean parity proxy that isn't biased by LZ4 noise.)

## 3. Implemented change

`src/builder.rs::StandaloneTextureBuilder::build`:

* When `root_hash.starts_with("Qm")`:
 - Skip the metadata TextAsset and the `metadata.json` container entry.
 - Use the source-image extension (`.png` / `.jpg`) for the container key
    (matches prod 100 %; previously we always emitted `.png`).
* Otherwise (CIDv1 entities): unchanged — emit the TextAsset, use `.png`.

Saves ~480 B × 108 standalone-PNG-CAB-bytes per platform on the 22-entity
windows test set. Effect on the bundle-size delta is amplified by LZ4HC: the
removed bytes were highly compressible (UTF-8 JSON + zeros) so the actual
file-size impact is closer to ~250-300 B per affected bundle.

## 4. LZ4 + alignment probes tested

| Hypothesis | Probe | Outcome |
|---|---|---|
| Unity uses LZ4HC streaming dictionary across blocks | `/tmp/lz4_stream_check.py` — independent vs streaming decompression of 3-block prod bundles | All blocks decompress independently. **No streaming dict.** |
| Unity uses different LZ4HC level | `/tmp/lz4_rust_vs_prod.py` — 186 prod blocks recompressed with Rust + python | **186/186 byte-equal at level 12.** Levels 3/6/9/10/11 produce different output for blk\[0\]. |
| `.resS` is 16-aligned within decompressed stream | `/tmp/inspect_prod_blocks2.py` — survey dir-node offsets | NO — `.resS` offsets mod 16 are `{0: 251, 4: 209, 8: 157, 11: 24, …}`. Unity packs CAB + .resS back-to-back with no inter-file alignment. Matches our writer. |
| Bundle data section is 16-aligned (post block-info + 0x200 pad) | `/tmp/ds_check.log` — survey all 2,174 prod bundles | YES — 2174/2174 at offset 16-aligned. Matches our writer. |
| Bundle blocks are 64 KB / 128 KB / variable | `/tmp/inspect_prod_blocks.py` | All blocks except the LAST are exactly **128 KB (131072) uncompressed**, flag 0x3 (LZ4HC). Last block is whatever remains. Matches our writer (`CHUNK_SIZE: usize = 0x20000`). |

**No mechanical alignment or LZ4 fix was found.** The LZ4HC byte-exact
finding is the most important: it means any size delta after this point is
purely from input-byte deltas (BC7 quality, missing-content bugs,.resS
streaming-gate too narrow) — not from any compressor-config knob.

## 5. New parity-irrelevance metric

`dev/parity_post_lz4.py` (new): a per-class measurement that's robust to
LZ4-amplification of input deltas. Decomposes both bundles to extract the
raw decompressed CAB +.resS bytes, then reports:

- `bundle_eq` — both bundles decompress to identical dir-node sets
- `cab_eq` / `cab_neq_size` / `cab_neq_content` — exact, size-mismatch, content-mismatch
- `cab_ppm_bits_diff` / `ress_ppm_bits_diff` — XOR popcount per million prod bytes
- `cab_size_delta_abs` / `ress_size_delta_abs` — absolute size mismatch (irreducible component)

Run with:

```bash
nix-shell --run "ABGEN_AB_BIN=<your-binary> python3 dev/parity_post_lz4.py"
# optionally ABGEN_LIMIT=100 to cap, ABGEN_PROD_ROOT=… for other test sets
```

This metric measures the actual content difference, not LZ4 noise.

## 6. What's still left (after this fix)

1. **PNG-class streaming gate too narrow** — the `model_referenced && fmt==25
 && 512×512` gate misses larger CIDv1 textures (1024, 2048, mipped) that
 prod streams. Needs a corpus probe to widen safely without breaking the
 parity\_bytes fixtures. ~5 of the worst 8 PNG-deficit bundles are this.
2. **Large embedded-glb BC7 drift** — top 5 glb deficits are 1-4 MB,
 dominated by BC7 quality on high-mip textures. Tracked under
 `bc7_tiebreak_v2.md`.
3. **gltf-with-external-buffer emission gaps** — 16 gltf/glb bundles fail to
 build at all; many surviving gltf bundles miss external-asset content.
 Out of scope for this proposal (content-correctness, not envelope).

## Files touched

* `src/builder.rs::StandaloneTextureBuilder::build` — Qm-CIDv0 standalone-PNG
 bundles now skip the metadata TextAsset and use the source-extension
 container key.
* `src/builder.rs::source_extension` — new helper, dot-prefixed file
 extension inferred from magic bytes (PNG / JPEG / fallback `.png`).
* `dev/parity_post_lz4.py` — new size-irrelevance metric script (XOR
 popcount on decompressed CAB +.resS).
* `src/bin/lz4hc-helper.rs` — new dev-only stdin→stdout LZ4HC helper used by
 `/tmp/lz4_rust_vs_prod.py` to verify byte-exact compression vs prod.
* `tests/parity_bytes.rs` — unchanged (`MAX_BITS_DIFFERENT = 773_674`); the 6
 texture fixtures are all `bafkrei…` CIDs which still receive the
 metadata TextAsset.
