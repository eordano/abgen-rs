# Mip-chain arithmetic is correct: the deep-mip diffs are the encoder

**Question.** For the textured bundles whose BC7 diffs are "spread across mips",
where does pixel divergence begin in the mip chain, and is the divergence the
mip *downsampling* arithmetic (sRGB rounding, filter domain, accumulator width)
or the BC7 *encoder*?

**Method.** Across the spread family (every CAT3/CAT4/CAT6 pair carrying texture
diffs), each BC7 texture present in both bundles at identical
dimensions/mip-count/payload-length was walked mip by mip. For every mip we
recorded the first mip whose encoded bytes differ, the first mip whose *decoded*
pixels differ (decoding each mip's blocks and comparing reconstructed RGBA), and
per-channel delta histograms. Because mip-0 is bit-identical for these pairs, its
pixels are identical *by construction* — so a divergence that starts at a deeper
mip can only come from the downsampling chain, and the chain can be tested
offline by replaying candidate arithmetics from the (shared) mip-0 source.

**The first-divergence-mip map.** Of the textures examined:

- The overwhelming majority diverge at **mip 0** itself — encoded bytes differ at
  mip 0, and decoded pixels differ at mip 0. Mip-0 input pixels are identical, so
  this is purely the encoder choosing a different block. This is the same
  encoder population the tiebreak track already owns.
- A small remainder (≈3%) have a **bit-identical mip 0** and first diverge at a
  deeper mip (mostly mip 1, a few at mip 2/3/5). In *every* one of these the
  encoded-byte divergence and the decoded-pixel divergence start at the **same**
  mip — there is no case where bytes diverge but the decode stays identical
  across a level, which would have implicated a pure bitstream choice.

**Chain arithmetic: the current rule is the best candidate.** For the
deeper-divergence population, five candidate downsample arithmetics were replayed
from the shared mip-0 source and scored against the reference's decoded mip-k
pixels:

- linear-f32 box filter, sRGB via the 256-entry LUT (the current chain),
- linear-f32 box filter, sRGB via exact `powf`,
- linear-f64 box filter, sRGB via exact `powf`,
- straight box filter in the sRGB u8 domain with round-half-up,
- straight box filter in the sRGB u8 domain with truncation.

Results that settle the suspect operations:

- **sRGB transfer (LUT vs exact powf): identical.** The 256-entry table and the
  exact float transfer produced byte-identical downsampled mips in every case.
  The table rounding is not a divergence source.
- **Filter domain: linear wins, decisively.** The linear-domain candidates beat
  both u8-domain candidates everywhere it mattered; box-filtering in the sRGB u8
  domain is wrong. Truncation is worse than round-half-up, as expected.
- **Accumulator width (f32 vs f64): f32 is at least as good.** The f32 chain
  matched or beat the f64 chain in the large majority; f64 never produced a
  systematically closer result. The current f32 accumulator is correct.
- **Alpha:** straight (non-premultiplied) averaging of the alpha channel matches;
  no alpha-weighted variant was needed.

In short, among the candidate arithmetics the **current chain (linear f32 box,
LUT sRGB, straight average) is the closest to the reference** — no alternative
operation reduces the residual.

**Verdict: the residual is the encoder, not the chain.** The deeper-divergence
population is not a chain bug. The decisive evidence:

1. The textures that first diverge at mip 5 are *bit-identical through mips 1-4* —
   four box-halve iterations reproduced byte-for-byte. A wrong filter, wrong
   domain, or wrong rounding boundary would have diverged at mip 1, not mip 5.
2. Block-field diffing at the divergent mips shows the encoder signature, not an
   arithmetic offset: endpoint deltas are overwhelmingly one quantization step
   (a few are two), the rest are mode-selection mismatches that recompute
   endpoints in a different mode. This is the same float-order encoder family as
   the mip-0 population — it simply surfaces first at a deeper mip when mip 0
   happens to encode identically.

So the "spread across mips" framing resolves cleanly: it is the BC7 encoder
spreading its float-order noise across whatever mips it touches. The mip
downsampling arithmetic — sRGB rounding, filter domain, accumulator width, alpha
handling — is already correct and is not a separate wall.

**Tools.** `examples/mipdiverge.rs` produces the first-divergence-mip map
(per-mip encoded-byte vs decoded-pixel divergence + per-channel deltas).
`examples/mipcand.rs` replays the candidate downsample arithmetics from the
shared mip-0 source and scores each against the reference decode.
