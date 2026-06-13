# glb-scene "large outliers" — BC7 mip-0 placeholder pattern

> Drill ran against `/tmp/abgen-verify-test-windows-per-bundle.csv`
> (post-rebaseline test corpus). All numbers are bytes-on-disk for the LZ4HC-
> compressed bundle.

## TL;DR

The 21 glb-scene bundles ≥ +40 KB ALL trace to the same root cause:
**asset-bundle-converter (reference) ships a fixed BC7 placeholder block for
every block of every mip of every BC7 texture in these bundles**. We compute
real BC7 from the GLB's RGBA32 source. Same uncompressed payload size, very
different LZ4HC compressibility — and the entire delta lives in the.resS.

Cross-kind total recoverable if we match the placeholder:
**~10.3 MB of positive delta** in the windows test corpus
(glb-animated 6.6 MB · glb-scene 3.1 MB · glb-wearable 0.7 MB).

## Per-bundle class-byte breakdown (top 5 + 2 small + control)

Reference is the rebaselined converter output in `/tmp/abgen-ref-out/test_windows/`.

|         bundle (CID prefix) | ours bytes | ref bytes |    Δ disk | Δ SF in-CAB | BC7 texs | BC7 blocks | %placeholder in ref |
| ---------------------------:| ----------:| ---------:| ---------:| -----------:| --------:| ----------:| -------------------:|
|         Qmbq (+938k, 49 GO) |  2,996,466 | 2,058,365 | +938,101  | +0 / +3,292* |     5    |     54,272 |              100 %  |
|              QmQxC (+359k)  |    563,728 |   204,208 | +359,520  | +0 / +484*   |     2    |     81,920 |              100 %  |
|             QmXQ75 (+295k)  |    812,383 |   516,832 | +295,551  | +16 / +176*  |     1    |     16,384 |              100 %  |
|              QmWgd (+203k)  |    587,544 |   384,313 | +203,231  | +0 / +592*   |     3    |     12,288 |              100 %† |
|               Qmez (+137k)  |    240,307 |   102,740 | +137,567  | +0 / +484*   |     2    |     32,768 |              100 %  |
|             QmRLZA (+76k)   |    115,662 |    39,595 |  +76,067  | +0 / +188*   |     1    |     16,384 |              100 %  |
|             QmW5U1 (+75k)   |    107,039 |    31,244 |  +75,795  | +0 / +188*   |     1    |     16,384 |              100 %  |
|              Qmec9 (=ctrl)  |     10,250 |    10,250 |        0  |   0 / 0      |     0    |          0 |          n/a (no BC7)|

\* "ours-vs-asset-bundles-mirror" CAB byte delta (older mirror, drifted); the
true rebaselined CAB SF bytes are byte-identical for every outlier. **All disk
delta is in the.resS block.**

† QmWgd uses the alt-canonical block `20ffbfd6aff53737afaaaaaa00000000` for 1
of its 3 BC7 textures; the other 2 use the standard placeholder. Both are
still uniform-block-throughout-mip.

### Class counts: identical between ours and ref

For every outlier:
- GameObject / MeshFilter / MeshRenderer / Transform / MeshCollider counts: **identical**.
- Mesh count: **identical**, byte size matches to within 16 B.
- Material count: **identical**.
- AssetBundle: **identical**.
- Texture2D count: **identical** (the apparent 2× from a stale mirror was
 noise — the rebaselined ref pairs match ours exactly).
- The only structural delta is in the **.resS payload bytes** themselves.

## The pattern (concrete)

The reference.resS for every BC7 texture in these bundles consists of the
single 16-byte block

```
20 5a bf d6 af f5 37 37 af aa aa aa 00 00 00 00
```

(BC7 mode 5 — `0x20` has trailing zeros 5, mode prefix `100000`; selectors
`afaaaaaa00000000` = uniform anchor index across all 16 pixels). For one
texture in QmWgd it's the variant `20 ff bf d6...`; same shape, different
endpoint pair.

That single block is **replicated across every block of every mip level**, so
e.g. a 512×512 BC7 mip chain (10 mips, 349,552 bytes) becomes 21,845 copies
of those same 16 bytes. LZ4HC compresses a 131,072-byte block of repeated
16-byte runs to **540 bytes**; ours encodes real BC7 (entropy ≈ 2 bits/byte)
and compresses to **40–130 KB per block**.

Demonstrated for QmRLZA `image_0` (512×512, mips=10):
```
mip0.. mip9 distinct_blocks_in_ref = 1 # all 21,845 blocks identical
mip0.. mip9 distinct_blocks_in_ours = 6442 # real BC7 of the source
underlying RGBA32: 116,088 px of ff2c2c2c (dark gray, 44 %), 958 distinct colors
```

So **the source IS NOT a single-color image** — the converter is just not
actually encoding BC7 for these textures. The most likely converter mechanism
is one of:
1. **Uninitialized GPU buffer.** The converter allocates the BC7 mip pyramid,
 never runs a real compressor (no D3D11 device available in batchmode? mip not
 committed?), and writes whatever the texture importer left in the buffer.
2. **`TextureImporter.compressionQuality = 0`** (fastest). The fast BC7 path
 collapses to a degenerate "fill with mode-5 default" output when given
 non-trivial inputs in a headless/no-GPU context.
3. **Memoised placeholder from `TextureImporter.SetPlatformTextureSettings`**
 with crunched format + RGBA32 fallback — the converter ships the RGBA32 as
 the "real" mip-0 (which we already match byte-for-byte) and the BC7 mip pile
 is the cached placeholder.

Whichever it is, the visible result is identical across all 26/232 BC7-bearing
glb-scenes, all 3/5 BC7-bearing glb-animateds, and 2/2 BC7-bearing glb-
wearables in the test corpus.

## Cross-kind impact

| kind                | bundles | with BC7 | all-placeholder | positive-delta (B) |
|---------------------|--------:|---------:|----------------:|-------------------:|
| glb-animated        |       5 |        3 |             3   |        6,596,313   |
| glb-scene           |     232 |       26 |            26   |        3,071,605   |
| glb-wearable        |       2 |        2 |             2   |          676,946   |
| glb-scene-collider  |      10 |        0 |             0   |                0   |
| standalone-texture  |       1 |        0 |             0   |        1,257,814 ‡ |
| other               |      66 |       24 |             0   |          232,306   |

‡ standalone-texture delta is a separate root cause, not BC7
placeholder — those bundles do contain real-content BC7 in both ours and ref.

**Total recoverable by matching the placeholder pattern: ~10.34 MB across
3 AB-kinds in the test corpus.** The same pattern is expected to scale up
~6× on the full val2 corpus (untested here but the per-kind placeholder
rate is 100 % everywhere we sampled).

## Concrete patch proposal

The encoder site is `src/builder.rs:199-219`
(`encode_texture_bc7`), feeding `bc7_pure::encode_bc7_mip_chain_with_profile`.
We do not have to (and almost certainly should not) regress our BC7 to
single-block output. The clean shape is a **per-bundle policy switch**:

```rust
enum Bc7Policy {
    Real,          // current behaviour — proper BC7 of the RGBA32
    Placeholder,   // emit a fixed 16-byte block, replicated across mip chain
}

// In src/bc7_pure.rs, add a stand-alone fast path:
pub fn encode_bc7_placeholder_chain(w: u32, h: u32, mips: i32) -> (Vec<u8>, i32) {
    const PLACEHOLDER: [u8; 16] = [
        0x20, 0x5a, 0xbf, 0xd6, 0xaf, 0xf5, 0x37, 0x37,
        0xaf, 0xaa, 0xaa, 0xaa, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut out = Vec::new();
    for m in 0..mips {
        let mw = ((w >> m).max(1) + 3) / 4;
        let mh = ((h >> m).max(1) + 3) / 4;
        for _ in 0..(mw * mh) {
            out.extend_from_slice(&PLACEHOLDER);
        }
    }
    (out, mips)
}
```

then in `encode_texture_bc7` dispatch on the policy. The policy is set
**per-Texture2D per-bundle** by the caller — which is the load-bearing
decision. The drill data is consistent with the policy being **always
Placeholder for any glb-* bundle BC7 texture in this corpus**, but
that's a strong claim. Recommended landing path:

1. Add the fast path to `bc7_pure.rs` (≈15 lines).
2. Plumb a `Bc7Policy::Placeholder` enum into `encode_texture_bc7` (≈10 lines).
3. Set it on the in-glb material-texture BC7 emission site in
 `src/builder.rs` (the spot near line 1080 where the streamed
 BC7 sibling is constructed) — opt-in, gated behind a one-line check.
4. Validate end-to-end with `abgen-verify`. Expected impact:
 ~+10.3 MB recovered in test, ~+60 MB recovered in val2 (scaled).

The alt-canonical block (`20 ff bf d6...`, seen on 1 of 32 textures) is rare
enough to defer; emitting the primary placeholder will close 31/32 BC7
textures byte-exactly and leave the 32nd at a much smaller residual.

**Risk:** zero on textures that are already uniform-colour or that never load
from BC7 at runtime. Quality risk on bundles where the runtime *does* sample
the BC7 mip (rather than the RGBA32 sibling) is the same as prod — by
definition, since prod ships the same bytes.

## Estimated total bytes recoverable across the 21 outliers

| bundle prefix | excess (B) | est recoverable (B) |
|---------------|-----------:|--------------------:|
| QmbqV5LR4neZ  |    938,101 |             ~720,000 |
| QmQxCuH4yKt   |    359,520 |             ~340,000 |
| QmXQ75bHvjM   |    295,551 |             ~280,000 |
| QmWgdWCv8S4   |    203,231 |             ~190,000 |
| Qmez15ErajP   |    137,567 |             ~130,000 |
| QmbWeNHEvJL   |     85,481 |              ~80,000 |
| QmUyyrngyg9   |     84,565 |              ~80,000 |
| QmRLZA9JEMw   |     76,067 |              ~73,000 |
| QmW5U1KSkHM   |     75,795 |              ~73,000 |
| QmfXv28SBsT   |     75,793 |              ~73,000 |
| QmPQ9ChAXah   |     75,783 |              ~73,000 |
| QmaHsmJvcCg   |     75,761 |              ~73,000 |
| QmYmHjr7yJQ   |     75,445 |              ~72,000 |
| Qma7Q3DxuZM   |     55,608 |              ~52,000 |
| QmVxguW4KeP   |     52,401 |              ~50,000 |
| QmSoXH2rrf3   |     52,393 |              ~50,000 |
| Qmdp4TBw5fY   |     43,688 |              ~41,000 |
| QmYM4LFSx6A   |     43,467 |              ~41,000 |
| QmW42VSpgru   |     43,127 |              ~40,000 |
| QmXvkQWuJC7   |     42,846 |              ~40,000 |
| QmZaM8wKFW6   |     41,176 |              ~38,000 |
| **TOTAL**     | **2,933,366** |     **~2.6 MB** in glb-scene alone |

Across the full test corpus (all kinds with all-placeholder BC7):
**~10.3 MB recoverable**. Per the cross-kind table this is ~95 % of all
positive bytes-on-disk delta in glb-animated + glb-scene + glb-wearable
combined.

## Artefacts (under `/tmp/`)

- `drill_classes.py` — UnityPy per-class enumeration ours vs ref
- `drill_blocks2.py` — UnityFS bundle block-info dump
- `diff_ress_via_unitypy.py` — byte-diff of CAB +.resS payloads
- `quantify.py` — outlier-list placeholder-block census
- `all_glb_scene.out` — corpus-wide BC7 placeholder histogram
- `all_kinds.out` — cross-kind BC7 placeholder histogram
