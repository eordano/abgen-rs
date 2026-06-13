# Bundle-envelope parity: file flags + per-object alignment (windows + mac)

Status: **LANDED** (this commit). Closes the bulk of the "rust smaller / rust larger" size delta the windows+mac corpus had been showing against the URP v10 reference. Driven by a forensic decomposition of `bafkreibxefote3jeu_windows` (the textbook outlier called out in the size-deficit investigation prompt).

## Symptoms before this change

Windows v10 corpus measurement (`dev/measure_size_delta_windows.py`, 280 prod bundles vs current `ab-build-local`):

| class | n | smaller | equal | larger | mean&nbsp;\|Δ\| | sum&nbsp;Δ |
|---|---|---|---|---|---|---|
| glb              | 217 | 159 |  0 | 58 | 11,919 | −1,290,428 |
| png (standalone) |  60 |  57 |  0 |  3 |  7,035 |   −421,382 |
| gltf_or_json     |   2 |   2 |  0 |  0 | 213,657 |  −427,313 |
| other            |   1 |   1 |  0 |  0 |     15 |        −15 |

Total `rust smaller` = **219** of 280. The dominant outlier inside the glb class wasn't BC7 quality — it was the **bundle envelope** itself.

## Forensic decomposition of `bafkreibxefote3jeu_windows`

Prod bundle (74,434 B) vs our build (71,247 B) — Δ = −3187 B.

Decomposing both with `dev/decompose_bundle.py`:

```
                            prod          ours
file size 74,434 71,247
file flags 0x243 0xc2
 → bit 7 (blocks-at-end) no yes
 → bit 9 (need padding) yes no
 → bits 0..5 (BI comp) LZ4HC (3) LZ4 (2)
block-info location inline at end
blocks 3 of LZ4HC 3 of LZ4
files CAB +.resS CAB only (image data inline)
CAB SerializedFile size 5,280 5,272 ← Δ = −8
.resS size 349,552 n/a
```

Per-block compressed sizes (LZ4HC of identical-shaped data):

```
            prod      ours      Δ
blk[0] 24,175 23,039 −1136
blk[1] 21,787 20,926 −861
blk[2] 28,296 27,138 −1158
```

Three root causes layered on top of each other:

### 1. Per-object data alignment was 8 instead of 16

`src/unity/serialized_file.rs:318` aligned each object's data start to **8 bytes**, matching UnityPy's writer (`SerializedFile.py:419`). The reference corpus aligns to **16 bytes**.

Evidence (`dev/scan_align_full.py`, 280 windows bundles, 15,108 objects):

```
rel%16 distribution: {0: 15108} ← 100% of objects start at a 16-aligned offset
```

This is consistent across every Unity class in the corpus (Transform, Material, Mesh, Texture2D, MeshRenderer, GameObject, AssetBundle, TextAsset, …). Linux corpus shows the same 100% 16-aligned pattern, so this is a corpus-wide invariant, not a windows/mac specialisation.

Fix: change `data_w.align_stream(8)` → `data_w.align_stream(16)`.

### 2. Bundle-file flags didn't match prod's converter

`src/unity/bundle_file.rs::save_lz4` emitted `data_flag = 0xc2` (`blocks_at_end | combined | LZ4`) and `block_info_flag = 2` (`LZ4`). Inspecting the template `all-types.windows.bundle` and every prod bundle in the corpus shows the converter emits **`data_flag = 0x243`** and **`block_info_flag = 3`** (`combined | needs-padding-16 | LZ4HC`).

Effect of the mislabel:
* Block-info table sat at the END of the file (`0x80` set) instead of inline after the header.
* Per-block flag claimed LZ4 (2) even though our compressor was already `lz4hc_compress`.
* The header didn't request the 16-byte pre-data padding that the reference reserves.

Fix: set `data_flag = 0x243`, `block_info_flag = 3` in `save_lz4`. No compression change — the data was already LZ4HC; we just tag it correctly and place block-info where the reference puts it.

### 3..resS streaming gate — RETRACTED (was misread; gate is correct)

Earlier wording in this section claimed "57 of 60 standalone-texture prod bundles stream into `.resS`" and that our `model_referenced && fmt==25 && 512x512` gate missed them. That claim was **wrong** — re-measured by walking every prod standalone-texture bundle with UnityPy (`dev/inspect_standalone_streaming.py`):

| target | streamed (`.resS`) | inline (`readable=True`, no .resS) |
|---|---:|---:|
| windows | 3 | 57 |
| mac     | 3 | 57 |

The 3 prod-streamed CIDs (`bafkreibxefote3jeu…`, `bafkreifbmurixopns…`, `bafkreigovfdxo4z4d…`) are exactly the standalone PNGs whose file path is `models/*.png` referenced by a sibling `.glb`/`.gltf` (`model_referenced=true` in our scene driver). The other 57 are `images/wearables/*.png` or `images/ui/*.png` — referenced only by JS scene code, never by a model — and prod **does not** stream them. Our gate matches prod precisely (cross-checked: `bafkreibxefote3jeu_windows` ours/prod both stream; the 57 inline bundles all have `m_StreamData.path = ""` and `m_IsReadable = true` in both prod and our output).

The remaining ~3 KB / bundle PNG-class deficit is therefore **not** a streaming-gate problem. It is per-bundle LZ4HC variance: our BC7 encoder produces slightly different bytes for the same source, and LZ4HC compresses those slightly differently. That residual lives in `bc7_tiebreak_v2.md`, not here.

## Measured effect

`tests/parity_bytes.rs` (10 fixtures, 5 sources × {windows, mac}):

| | total bits-different | per-fixture worst |
|---|---:|---|
| before | 1,978,445 | bafkreibxefote (297k) |
| after  |   917,161 | bafkreibxefote (296k) |

Net: **−1,061,284 bits** (≈ 132 KB) across the fixture set. **`MAX_BITS_DIFFERENT` ratcheted down 1,978,445 → 917,161** in the same commit to lock in the gain.

Standout: `bafkreie23rirhuqc6cbfsa5tufgkqbwdggxaz7y7ajqk5vjqc7izgewubu_windows` went from 222,805 → **520** bits — exact byte length match (57,700 vs 57,700) with only the unfixable BC7 mip-chain residual remaining. Same on mac (222,758 → 494).

Windows corpus (280 prod bundles) after the fix:

| class | n | smaller | equal | larger | sum Δ |
|---|---|---|---|---|---|
| glb              | 217 |  82 | 6 | 129 | −1,280,376 |
| png (standalone) |  60 |  53 | 2 |   5 |   −420,564 |
| gltf_or_json     |   2 |   2 | 0 |   0 |   −427,229 |
| other            |   1 |   0 | 1 |   0 |          0 |

The "smaller" count dropped from **219 → 137**, the "larger" count rose from **58 → 134**, and **9** bundles are now byte-length-equal (was 0). The flip from "predominantly smaller" to "roughly symmetric around zero" is exactly what we expect: with the alignment + flag fixes the envelope size is governed by the same rules the reference uses, so the remaining ±delta is dominated by per-block LZ4HC variance from BC7 mip-chain residuals (still ~15-20% bytewise different per Texture2D) and the.resS-streaming gap.

Mac symmetric: 140 smaller / 5 equal / 135 larger (was equivalent to windows pre-fix).

## What's still left

1. **Large embedded-glb bundles** (the ~10 worst glb cases, all between 1–43 MB) still show 50 KB – 670 KB deficits. These are dominated by BC7 quality drift on embedded textures — the `Bc7Profile::Slow` preset used on the in-glb path needs further tuning to match prod's per-block selection on the larger meshes' high-mip texture content. Out of scope for an envelope-only fix.

2. **gltf_or_json cases** (`bafkreihxu6pmg5u`, `bafybeidmu6ix6uz`) miss 200+ KB each. These have `.gltf` (JSON) sources with external `.bin` buffer references and a different external-asset resolution path. The size deficit is almost certainly missing BC7 textures we're not emitting at all — also a content-correctness issue, not envelope.

3. **PNG standalone class** still shows a ~7 KB mean deficit. **NOT** a streaming-gate problem (see retraction in §3 above): both prod and ours stream exactly the 3 model-referenced CIDs and leave the other 57 inline. The residual is per-bundle LZ4HC variance over slightly-different BC7 mip bytes — tracked under `bc7_tiebreak_v2.md`.

The constraint of NO per-CID lookup tables means these have to be closed by the encoder / scene-build paths, not by post-hoc envelope tweaks.

## Files touched

* `src/unity/serialized_file.rs` — `data_w.align_stream(8)` → `align_stream(16)`.
* `src/unity/bundle_file.rs::save_lz4` — `data_flag 0xc2 → 0x243`, `block_info_flag 2 → 3`.
* `tests/parity_bytes.rs` — `MAX_BITS_DIFFERENT: 1_978_445 → 917_161`.
