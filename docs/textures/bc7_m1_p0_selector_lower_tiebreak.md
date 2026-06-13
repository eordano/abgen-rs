# BC7 selector "walk-down" tiebreaks: removed (they diverged from the reference)

**Why it matters:** abgen-rs once carried two selector post-processing passes
that fired after the encoder had chosen its final mode and endpoints:

- a mode-1 / partition-0 pass that, for each pixel, decremented the selector
  while the palette entry below it decoded to the same RGB triple; and
- a mode-6 pass that collapsed runs of palette-equal selectors down to the
  run's lowest index.

Both rested on the same idea: when several selectors decode to the same pixel,
the choice is a free tiebreak, and the encoder supposedly prefers the lowest index.
They were derived against an early reference corpus and helped there.

**What turned out to be true:** against the current canonical reference,
these passes were net negative. The cases they targeted are real ties — every
differing block decodes bit-for-bit identically to the reference — but the
converter's bc7e output does **not** consolidate the selectors downward. It keeps the encoder's raw
per-pixel index projection, which is often a spatially structured pattern (for
example a mode-1 block where the projection lands columns on `2,2,1,1` rather
than a uniform `1,1,1,1`). The walk-down passes rewrote that raw pattern into a
consolidated one, so the very blocks they were meant to fix diverged instead.

The pure-Rust encoder is faithful to bc7e at its basic preset, so its raw
selector projection already matches the reference for these blocks. The extra
post-processing was the only thing standing between us and a byte-identical
result.

**The fix:** remove both passes. With them gone, the affected standalone
textures whose only remaining difference was decode-identical selector blocks
become byte-identical, and no previously matching bundle regresses — the raw
projection is what the reference encodes.

**Scope.** This only touches blocks where multiple selectors decode to the
same color, i.e. near-flat or low-gradient regions. Blocks with real gradients
have a unique best selector per pixel and were never affected by either pass.
The remaining BC7 standalone-texture residual is unchanged in character: it is
the source-pixel and float-order wall (alpha bleed, NPOT resize, JPEG decode,
AVX2-vs-ISPC ordering), not selector tiebreaking.
