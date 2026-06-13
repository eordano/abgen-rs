# SF +8 pad strip (834d5a7) — bafkreie23 cap raise is principled

**Status:** verified, no action required. The `bafkreie23rirhuqc6` cap raises
landed in 834d5a7 (+28 ppm windows / +32 ppm mac) are real per the
`bits_different` metric but reflect LZ4 compression length-noise, not a
worsening of the underlying produced content. The architectural fix is
correct.

## Method

`examples/parity_decompose.rs` builds each fixture and splits the parity
`bits_different` into:

- `common_xor` — sum of XOR-popcount over the common `min(len_ours, len_prod)`
 prefix.
- `len_pen` — `|len_ours - len_prod| * 8` (the length-delta penalty added by
 `tests/parity_bytes.rs::bits_different`).

Measurements come from running the example on (a) `main` (post-834d5a7) and
(b) a one-line revert that re-enables the trailing-object SF pad.

## bafkreie23rirhuqc6 — pre vs post 834d5a7

| target  | state          | ours_B | ref_B  | ΔB | common_xor | len_pen | total | ppm  |
|---------|----------------|-------:|-------:|---:|-----------:|--------:|------:|-----:|
| windows | pre (with pad) |  57700 |  57700 |  0 |        520 |       0 |   520 | 1126 |
| windows | post (no pad)  |  57698 |  57700 | -2 |        517 |      16 |   533 | 1154 |
| mac     | pre (with pad) |  57692 |  57692 |  0 |        494 |       0 |   494 | 1070 |
| mac     | post (no pad)  |  57690 |  57692 | -2 |        493 |      16 |   509 | 1102 |

The commit's claim — *"bits-diff actually decreased; only abs_diff(len)
penalty shifted +16 bits"* — is confirmed exactly:

- windows: `common_xor` 520 → 517 (-3 bits), `len_pen` 0 → 16 (+16) ⇒ net +13.
- mac: `common_xor` 494 → 493 (-1 bit), `len_pen` 0 → 16 (+16) ⇒ net +15.

## Why the length flips by exactly 2 compressed bytes

Pre-834d5a7, the SF for bafkreie23 was 8 bytes longer (the trailing
align(16) after the last object — zeros). Removing those highly-compressible
trailing zeros shrinks LZ4HC output by ~2 bytes here. The converter does the
same strip; our pre-state happened to coincide with the reference's compressed
length by the LZ4-noise lottery, so the metric appeared better than it was.
Post-strip we now match the reference's compressed length within 2 bytes from
below — a more honest representation of the residual.

## Cross-fixture context (parity_decompose on all 10)

| fixture (target)              | pre-ppm | post-ppm | Δppm | note                          |
|-------------------------------|--------:|---------:|-----:|-------------------------------|
| bafkreihfx3a6srd6q windows    |      80 |        0 |  -80 | now byte-identical            |
| bafkreihfx3a6srd6q mac        |      80 |        0 |  -80 | now byte-identical            |
| bafkreif7fy5hinexy windows    |    4687 |     4649 |  -38 | small improvement             |
| bafkreif7fy5hinexy mac        |    4703 |     4651 |  -52 | small improvement             |
| bafkreihbgn43gqc3k windows    |  107935 |   107928 |   -7 | flat                          |
| bafkreihbgn43gqc3k mac        |  107939 |   107932 |   -7 | flat                          |
| **bafkreie23 windows**        |  **1126** |   **1154** | **+28** | **LZ4 length-noise**       |
| **bafkreie23 mac**            |  **1070** |   **1102** | **+32** | **LZ4 length-noise**       |
| bafkreibxefote3 windows       |  487252 |   487252 |    0 | flat                          |
| bafkreibxefote3 mac           |  487294 |   487294 |    0 | flat                          |

Net across the 10-fixture suite: +60 ppm on bafkreie23, -334 ppm on the
others. 50-bundle validation_2 sample (per 834d5a7 commit) shows 0 → 10
byte-equal vs prod; the architectural change is unambiguously right.

## Outcome

No code change. The example `parity_decompose` lands as a permanent
diagnostic so the next time a fixture cap moves, the question *"did the
underlying content get worse, or did LZ4 length-noise shift the penalty
term?"* can be answered in one command:

```
cargo run --release --example parity_decompose [<cid-prefix>]
```

A longer-term option, if LZ4-noise cap movement becomes a recurring
distraction, is to switch `tests/parity_bytes.rs::bits_different` to a
length-normalised metric (e.g. report `common_xor` and `|ΔB|` separately
with their own ceilings) — explicitly out of scope here.
