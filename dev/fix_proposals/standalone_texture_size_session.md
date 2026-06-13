# standalone-texture SIZE / mip-split parity — NEGATIVE FINDING

Area: RESEARCH_AREAS #5 (Tier A, size parity). Baseline commit `12e0ff4`,
windows test-set reference `ad0564d-windows`.

## Hypothesis under test

"For size-divergent standalone-textures, ours/ref disagree on mip count,
m_CompleteImageSize, or the inline-vs-.resS split point — a structural miss
that masks all texel work. Get the byte COUNT right first."

## Result: the structural size is ALREADY 100% correct

Built ours from the windows test-set reference and compared every
standalone-texture (Texture2D + TextAsset + AssetBundle) pair. Two probes
(`examples/tex_size_probe.rs`, `tex_size_probe2.rs`):

| determinant                                   | mismatches / 2440 |
|-----------------------------------------------|-------------------|
| m_Width / m_Height                            | 0 |
| m_MipCount                                     | 0 |
| m_TextureFormat                                | 0 |
| m_CompleteImageSize                            | 0 |
| m_IsReadable                                   | 0 |
| streaming state (inline vs .resS)              | 0 |
| `image data` byte length                       | 0 |
| `.resS` sidecar byte length                    | 0 |
| **uncompressed SerializedFile payload length** | **0** |

Yet 2200 / 2440 bundles differ in final (LZ4-compressed) file size, and for
ALL 2200 the category is "image data & .resS byte-equal in length but bundle
size differs" — i.e. the difference is purely the *content* of the compressed
payload changing the LZ4 output length.

## Where the divergence actually lives (`tex_size_probe3.rs`)

Comparing the uncompressed serialized payloads byte-for-byte:

| category                                        | count / 2440 |
|-------------------------------------------------|--------------|
| serialized payload byte-identical               | 504 |
| **differ ONLY in `image data` (BC7 texels)**    | **1924** |
| differ only in header / non-image bytes         | 8 (same length) |
| differ in both                                  | 4 (same length) |

The 12 with header diffs are same-length (the uncompressed-payload-length
column is 0 mismatches), so they do NOT drive any bundle-size mismatch — they
are bit-value (color-space / float-LSB) residuals, not size. The 1924
image-data-only diffs are pure BC7 encoder output: same target dims, same mip
chain, same byte count, different texel bytes → different LZ4 compressed length
→ different bundle file size.

## Conclusion

The assigned hypothesis is false. abgen-rs already nails the texture mip
count, m_CompleteImageSize, and the inline-vs-.resS split point for every
standalone-texture in the corpus. The 2200-bundle "size mismatch" and the
~1.14e9 standalone-texture diff-bits are NOT a structural size problem — they
are entirely downstream of BC7 texel values, which the brief explicitly assigns
to a separate agent ("Do NOT chase BC7 bit-values"). There is no size/mip/split
fix available in this area; the byte COUNT is already correct.

The 504 serialized-identical vs only 233 byte-identical bundles gap (271) is
the AssetBundle container / LZ4 block framing producing different compressed
bytes for an identical serialized payload — also not a Texture2D size issue and
out of this area's scope.

## Verify numbers (kind = standalone-texture, unchanged — probes only)

baseline == after (no code change to the converter; only read-only example
probes added):

bundles 2440, byte-id 233, smaller 1212, larger 988, diff-bits 1138407360,
ppm 493280.6.

Parity gate `cargo test --release --test parity_bytes`: green (2 passed).

## Artifacts

- `examples/tex_size_probe.rs`  — per-field structural-size diff counts
- `examples/tex_size_probe2.rs` — image/.resS/uncompressed-payload length vs
  bundle-size mismatch attribution
- `examples/tex_size_probe3.rs` — serialized payload byte-diff localization
  (image-data vs header)
