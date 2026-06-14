# BC7 mode-6 texel values — the walk-down heuristic (negative finding)

> **SUPERSEDED BY [bc7_m1_p0_selector_lower_tiebreak.md](bc7_m1_p0_selector_lower_tiebreak.md).**
> The walk-down pass was removed from the encoder; the lower-selector tiebreak is the
> shipped rule. Kept for the paper trail.

**Why it matters:** Standalone-texture bundles still diverge from the reference, and one suspected cause was abgen's mode-6 selector heuristic (`m6_walk_down_palette_eq_runs`), which snaps runs of equal palette entries to their run start. If that heuristic were mispredicting the reference's index assignments it would be a recoverable source of texel-level divergence blocking byte-identical output.

**How it works:** This is a negative finding — the heuristic is not the problem and no change was made. To compare texel values meaningfully you must restrict to bundles whose compressed size already matches the reference, so that block byte counts line up and diffs aren't masked by size mismatch. On that clean slice, mode-6-versus-mode-6 divergence is tiny, and toggling the heuristic off leaves it bit-for-bit unchanged. Decoding the few residual mode-6 blocks shows each is an isolated search-boundary tie — an alpha index search, an endpoint/p-bit LSB at a clamp boundary, or an upstream run-choice decision made in the solution evaluator at a midpoint-rounding boundary — none of which the heuristic controls. Its only corpus-wide effect is on size-mismatched bundles, where changing index bytes shifts the compressed size; that is compression noise, not a texel-parity signal.

The heuristic must nonetheless stay: it is load-bearing for pinned reference fixtures with alpha-varying, low-run palettes that the current size-matched corpus simply doesn't surface. The real standalone-texture parity mass lives in mode selection (mode-5 over-pick versus modes 4/6/7 and the two-subset modes), a different area entirely from mode-6 texel values.
