# Sub-block mip padding: tile, don't replicate

**The rule.** Block-compressed formats (BC7, DXT1/5, BC5) encode 4x4 pixel
blocks, but the smallest mip levels of a texture are smaller than one block:
a square power-of-two chain ends ... 4x4, 2x2, 1x1, and rectangular chains
pass through 2x4 or 1x2. Unity fills the missing texels of such a block by
**tiling (wrapping) the mip image**, not by replicating its last row/column.
A 2x2 mip with pixels

```
A B
C D
```

is compressed as the 4x4 block

```
A B A B
C D C D
A B A B
C D C D
```

An edge-replicating pad (`A B B B / C D D D / C D D D / C D D D`) produces a
different encoded block: the encoder weighs all 16 texels when choosing
endpoints and indices, so the padding contents change the output bytes even
though only 4 texels are ever sampled.

**Why this was hard to see.** The padding never changes which *meaningful*
pixels exist, only what the encoder believes surrounds them. The visible
symptom was a single differing 16-byte block per texture — always the 2x2
mip (or 1x2/2x4 on rectangular chains), never 1x1. That pattern is itself
the fingerprint: for a 1x1 mip every padding scheme degenerates to 16 copies
of the same pixel, so wrap and replicate agree there, and 4x4-and-larger
mips need no padding at all. Only the in-between sizes expose the rule.

**How it was derived.** Decoding the differing 2x2-mip block from a
reference bundle and from our output side by side (`examples/mip22probe.rs`)
showed identical endpoint bytes and index rows repeating with period 2 —
the wrapped layout read straight off the decoded pixels. No mip-chain
*downsampling* difference was ever involved: the 1x1 mip (computed from the
2x2) already matched, proving the 2x2 pixel values were correct and only
the block packaging differed.

**Scope.** The same fill is applied before every block-compressed encode
(BC7, DXT1, DXT5, BC5). It is not alpha-specific and not format-specific;
earlier observations that "only alpha-bearing textures diverge at 2x2" were
sample bias in the corpus being checked.
