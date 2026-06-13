# BC7 kernel-matching study

Goal: settle whether the residual standalone-texture BC7 divergence vs the
converter reference can be matched by compiling the REAL `bc7e.ispc` kernel (the same
MIT/Apache-2.0 Binomial source our Rust port mirrors) with some
(ISPC version x target x opt flags) combination, instead of reimplementing it.

## Pieces

- `examples/bc7kernel_probe.rs` (in the crate) — builds a probe file of
  `(mip0 input pixels, our block, reference block, is_diff)` triples for the
  standalone-texture no-resize population. Inputs are the EXACT pixels abgen fed
  its bc7e port, recovered via the `ABGEN_BC7_CAPTURE` hook in
  `bc7_pure::encode_blocks` (env-gated, off in production). The hook appends
  `output(16B)+input(64B)` per block; the probe builder streams that capture and
  joins by output-block, so no pipeline reconstruction is needed.
- `dev/bc7_kernel/bc7kernel_harness.c` — links a compiled `bc7e.ispc` object and
  runs `bc7e_compress_blocks` with the `_init_basic` profile (the converter's
  `CompressedHQ` BC7 maps to bc7e basic) on the probe inputs, scoring
  block-exact concordance vs our blocks (sanity) and vs the reference (the prize),
  split by the diff / match populations.
- `dev/bc7_kernel/sweep_one.sh` — compile one (target, opt) combo and score it.

## How to reproduce

1. Fetch the kernel:
   `git clone https://github.com/richgel999/bc7enc_rdo` (Apache-2.0); the file
   is `bc7e.ispc`. Field names match our `Params` 1:1.
2. Build the capture: run `abgen-corpus --from-reference <ref-subset> <out>
   --platform windows --real-textures` with `ABGEN_BC7_CAPTURE=<file>` set.
   (`--real-textures` so standalone textures encode real BC7 instead of stubs.)
3. Build the probe: `bc7kernel_probe <capture> <pairs.tsv> <probe.bin>`
   (`ABGEN_PROBE_SAMPLE=N` to subsample N per class for fast sweeps).
4. Compile + score:
   `ispc --target=avx2-i32x8 --pic -O2 -o bc7e.o -h bc7e_ispc.h bc7e.ispc`
   then `gcc -fPIE bc7kernel_harness.c bc7e.o -o h && ./h probe.bin 1 basic`.

## Verdict (2026-06-11) — NO MATCH; route closed

Concordance matrix on the standalone-texture mip-0 population (perc=1 = sRGB,
the dominant class; "RECOVERED" = diff blocks the candidate flips to ==ref,
"KEPT" = match blocks it preserves):

| candidate                                  | RECOVERED (of diff) | KEPT (of match) |
|--------------------------------------------|---------------------|-----------------|
| bc7e **basic** (our default), any ISPC/opt | 0.10 - 0.21%        | 96.9 - 99.3%    |
| bc7e basic + pbit_search                   | 3.93%               | 83.9%           |
| bc7e **slow** (-O2)                         | 12.37%              | 79.0%           |
| bc7e slow + disable-fma                     | 12.79%              | 79.2%           |
| bc7e veryslow / slowest                     | 3.0 - 3.3%          | 76 - 78%        |

ISPC version is irrelevant: builds are byte-identical across ISPC 1.13 - 1.30
(LLVM 10 - 21) at default opt (IEEE-strict, no version dependence). fast-math /
disable-fma shift a few blocks but never toward the reference.

The profile is the only large lever, and it is a mirage: SLOW recovers ~12% of
diff blocks but BREAKS ~21% of match blocks (block-level ref-match: basic 70.4%,
slow 59.5% over the full 124M-block corpus). A per-texture oracle picking the
better of basic/slow gains only +6 fully-mip0-perfect textures over basic alone
(703 vs 697 of 6,239). The SLOW recoveries are coincidental landings of a
different search, not the reference's true settings.

Conclusion: no ISPC build and no bc7e profile (global or per-texture) reproduces
the reference. The residual is the reference binary's per-candidate float
arithmetic, unreachable from the open source. This route is closed; keep the
harness as the standing proof. Full writeup in `docs/walls/bc7_float_order_taxonomy.md`.
