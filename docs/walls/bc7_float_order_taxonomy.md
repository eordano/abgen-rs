# BC7 "float-order" divergence: decision-level taxonomy and verdict

This is the closing analysis of the standalone-texture BC7 wall: same-dimension
lossless-PNG sources whose input pixels are provably identical on both sides,
yet whose compressed BC7 blocks differ from the converter reference and decode
differently. The question was whether the divergence is a recoverable
float-summation-order artifact in our encoder, or an irreducible difference
between our faithful Rust port of bc7e and the actual ISPC-compiled bc7e the
asset-bundle-converter fork was built with.

The verdict is: **irreducible.** It is not our SIMD lane order, not an effort
level we can raise, and not a single tie rule. It is the per-candidate
floating-point math of our port versus the ISPC binary's codegen.

## What was measured

A clean taxonomy was built over every mip-0 block of the no-resize subset
(`examples/bc7forensic`), restricting to blocks whose 16 input pixels are
identical on both sides (same source PNG, same dimensions, no resize). For each
differing block it records which BC7 decision diverges first: mode, partition,
endpoints, p-bits, or indices.

Across the differing texture pairs, the no-resize blocks split roughly:

- **~43% pick a different MODE.** This is strongly directional, not random:
  our encoder over-selects the cheap modes (1, 4, 6 — single-subset / dual-index)
  while the reference over-selects the thorough modes (0, 2, 3, 5). The dominant
  flows are ours-mode-6 to ref-mode-3, ours-mode-1 to ref-mode-3, ours-mode-4 to
  ref-mode-5.
- **~47% are SAME mode and partition but different ENDPOINTS.** Of these, about
  75% are within one quantization step (max endpoint delta <= 1). Mode 6
  dominates this class.
- The remainder are partition-only, rotation/index-mode-only, p-bit-only, and
  index-only differences (each a small minority).

## The float-order hypothesis is empirically dead

The standing hypothesis was that our 8-wide SIMD error reductions sum in a
different order than ISPC's gang lanes, producing different winners. Two
independent lines of evidence refute it:

1. **Code reading.** The reference's main per-solution evaluator
   (`evaluate_solution` in bc7e.ispc) accumulates block error as a scalar float
   over the 16 pixels in sequential pixel order; ISPC's gang dimension is the
   set of candidate *solutions* evaluated in parallel, not the pixels, so there
   is no cross-lane reduction of the error sum at all. Our `eval_solution_n16`
   kernels (both scalar and AVX2) accumulate the same per-pixel error in the
   same left-to-right channel order and the same sequential pixel order. The
   final per-block float total is truncated to integer once, identically on
   both sides. (The integer per-pixel `compute_color_distance_rgb` path in the
   reference applies only to the degenerate single-color-block special case, not
   the general path.)

2. **A/B test.** Encoding the standalone-texture sample with all SIMD error and
   estimation kernels disabled (`ABGEN_BC7_SCALAR=1`, which forces the pure
   scalar path) produces **byte-identical output** to the AVX2/AVX-512 build
   across the full sample. Our SIMD and scalar paths already agree to the bit,
   so SIMD lane order cannot be the source of any divergence from the reference.

## Raising encoder effort does not recover blocks

The directional mode bias (reference prefers thorough modes) looked like a
quality-level mismatch — as if the reference build explored more candidates.
This was tested directly by overriding the encoder's quality knobs on the
sample and comparing byte-id recovery against the reference:

- restoring full partition counts + enabling mode 2 + p-bit search,
- restoring p-bit search alone,
- restoring full partitions alone,
- raising the uber refinement level.

Every variant **changed roughly 30 bundles in the sample and recovered exactly
zero** byte-identities (and regressed none). Giving our encoder more effort does
not move its choices toward the reference's. The reference's mode and endpoint
choices are not reachable by exploring more candidates with our math — they
follow from the ISPC binary computing slightly different per-candidate error
values (PCA axis, least-squares endpoint refinement, and the candidate-error
accumulation all run in float, and ISPC's compiled codegen — including any
FMA contraction and fast-math relaxation chosen at its build — rounds
differently than our explicit non-fused Rust arithmetic). Reproducing those
values bit-for-bit would require the ISPC binary's exact instruction selection,
which is not recoverable from the MIT source alone.

## The recoverable ceiling

Of the texture-bearing differing pairs:

- About **2,991 texture objects are no-resize** (input pixels identical) and
  diverge purely in BC7 block choices. These are the only ones a bit-exact
  encoder could flip; they map to roughly **1,100 distinct textures**, of which
  about **1,095 are standalone-texture bundles** that would flip 1:1 to
  byte-identical, plus a fraction of ~650 glb-embedded textures whose host
  bundle has no other divergence.
- About **2,213 texture objects are resize** (NPOT or downscaled). Their input
  pixels differ from the reference before the encoder ever runs, so they are
  governed by the separate NPOT-resize-filter and JPEG-decoder walls, not the
  encoder, and an encoder fix would not touch them.

So the realistic ceiling for an encoder-parity fix is on the order of **~1,100
bundles** — but that ceiling is unreachable without bit-reproducing the ISPC
binary's float results, which the experiments above show is not achievable by
re-ordering, re-truncating, or re-tuning our port.

## Disposition

No code change is warranted. The encoder is faithful to bc7e.ispc as written;
the residual is our correct-but-distinct float math versus the reference's
ISPC-compiled binary. The val300 windows gate stays at its baseline byte-id
count with zero regressions. The `ABGEN_BC7_SCALAR` switch is kept as a
forensic tool (it proves the SIMD path is clean); `examples/bc7forensic` is the
taxonomy tool behind these numbers.

## Build-the-real-kernel study (closes the "compile the source" route)

The remaining open question was whether compiling the genuine `bc7e.ispc`
Binomial source (the same Apache-2.0 kernel our port mirrors, from
`richgel999/bc7enc_rdo`) with some ISPC version / target / opt-flag combination
would reproduce the reference's blocks where our port cannot. It does not. We
built a black-box harness (`dev/bc7_kernel/`, plus `examples/bc7kernel_probe`,
`bc7kernel_rustscore`, `bc7profile_split`) that recovers the exact mip-0
encoder-input pixels our pipeline fed bc7e (via the `ABGEN_BC7_CAPTURE` hook),
runs candidate compiled kernels on them, and scores block-exact concordance
against the reference, split into the *diff* population (our block != reference)
and the *match* population (our block == reference; must be preserved).

Findings on the standalone-texture population (~124M mip-0 blocks):

- **ISPC version is irrelevant.** The compiled kernel is byte-identical across
  ISPC 1.13 through 1.30 (LLVM 10 through 21) with default optimization — ISPC's
  default float codegen is IEEE-strict, so there is no version-dependent answer
  to find. `--opt=fast-math` and `--opt=disable-fma` shift a handful of blocks
  (disable-fma pulls the kernel closest to our explicit non-fused Rust math, as
  the FMA hypothesis predicts) but neither moves toward the reference: every
  combo recovers under 0.2% of the diff blocks.
- **Quality profile is the only large lever — and it is a mirage.** bc7e BASIC
  (Unity `CompressedHQ`'s tier, our standalone-texture default) recovers ~0.1%
  of diff blocks; bc7e SLOW recovers ~12%. Bisected, `pbit_search=true` is the
  dominant cause. But SLOW simultaneously *breaks* ~21% of the blocks BASIC gets
  right, so it is net-negative: BASIC matches the reference on ~70.4% of blocks,
  SLOW on ~59.5%. The Rust port reproduces both numbers (SLOW 12.0% vs ISPC-SLOW
  12.4%), reconfirming the port is faithful and that the gap is not kernel
  codegen.
- **The split is not per-texture either.** A per-texture oracle that picks the
  better of BASIC/SLOW for each texture yields only ~+6 fully-mip0-perfect
  textures over BASIC alone (703 vs 697 of 6,239 differing textures). The blocks
  SLOW recovers are scattered inside textures that are otherwise BASIC, so no
  whole-texture profile choice captures them.

Conclusion: there is no ISPC build and no bc7e profile (global or per-texture)
that reproduces the reference. The recovered blocks under SLOW/pbit are
coincidental landings of a different search, not evidence of the reference's
true settings. This upgrades the verdict from "irreducible float-order" to
"irreducible, and confirmed unreachable by building or re-tuning the genuine
kernel": the reference's per-candidate float results come from its specific
compiled binary's arithmetic, which is not recoverable from the open source with
open compilers. The `dev/bc7_kernel/` harness is kept as the standing proof.
