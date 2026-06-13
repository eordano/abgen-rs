# Texture2D — alpha-bleed v2: banker's rounding, K=32 + 4-conn retained

Re-validates the K=32 / 4-conn alpha-bleed parameters from
`landed/alpha_bleed_standalone.md` against the 22-entity, 2,174-bundle
`workdir/pathid_rt_v10_windows` corpus (7.7× larger than the 280-bundle set
used in the original probe). Lands one tightening: **banker's rounding
(half-to-even) replaces round-half-up in the per-pixel RGB mean**.

## Re-measure: alpha-bleed impact on the new 22-entity corpus

Per-class `dev/per_class_bits_mac.py` Texture2D row:

| platform | pre-bleed Tex2D ppm | post-bleed Tex2D ppm | delta |
|---|---:|---:|---:|
| windows (4,656 paired objs) | 272,007 | 267,890 | -4,117 ppm (-1.5%) |
| mac (1,004 paired objs)     |  19,704 |  15,637 | -4,067 ppm (-20.6%) |

Windows ppm only drops 1.5% because the new corpus is dominated by 4 huge
**in-glb** textures (138-328 MB each) where we deliberately do **not** bleed
(`CustomGltfImporter` doesn't flip `alphaIsTransparency`; bleeding there
regresses the corpus by 7.4%). The standalone-texture closure is real
(-5.2M bits absolute on windows) but diluted by the residual mass that lives
behind the in-glb-path gate. Mac corpus is unchanged (no new entities), so
the mac delta is just the original -20% restated.

Total raw-byte parity across all classes:
- windows: 248,780 → 247,121 ppm (-1,659 ppm, -14.5M bits)
- mac: 14,659 → 11,654 ppm (-3,005 ppm, -5.2M bits)

## Probe: K, kernel, rounding on 4 representative CIDs

`dev/bc7_probe/probe_bleed_v2.py` sweeps K ∈ {16,24,32,40,48},
kernel ∈ {4-conn, 5×5, 8-conn}, rounding ∈ {int-mean, float-round} against
prod's decoded mip-0 BC7 at α=0 pixels (within-8/channel close% + exact%).

CIDs picked from the new corpus: bafybeicydagq67… (1024², 91% α=0),
bafybeifvu5awizl… (1024², 90% α=0), bafybeicqewfizn… (1024², 88% α=0),
bafybeigs5ygjyxj… (1024², 64% α=0).

Aggregate (α=0-pixel-weighted across all 4 CIDs):

| K  | kernel | round       | close% | exact% | MAD  |
|---:|---:|---:|---:|---:|---:|
| 32 | 4-conn | float-round | **95.44** | **73.48** | 2.12 |
| 32 | 4-conn | int-mean    | 95.43 | 73.20 | 2.12 |
| 24 | 4-conn | int-mean    | 92.94 | 72.39 | 3.33 |
| 16 | 4-conn | int-mean    | 89.72 | 71.23 | 4.92 |
| 16 | 8-conn | int-mean    | 89.57 | 70.57 | 4.90 |
| 48 | 4-conn | int-mean    | 86.50 | 69.54 | 6.87 |
| 32 | 8-conn | int-mean    | 85.20 | 68.67 | 6.04 |
| 16 | 5×5    | int-mean    | 84.86 | 68.22 | 6.07 |
| 32 | 5×5    | int-mean    | 72.37 | 59.54 | 16.0 |

Conclusions:
- **K=32 remains optimal corpus-wide.** Three of four CIDs peak at K=32.
 The one exception (bafybeifvu5awizl74qvplqn) peaks at K=16 because that
 texture's α=0 regions are small enough to converge in 8-16 passes; the
 weighted aggregate still puts K=32 first.
- **4-conn strictly dominates 5×5 and 8-conn.** 5×5 is dramatically worse
 (96% → 73% at K=32). 8-conn loses ~10 pp at every K above 16. Diagonal
 neighbors break Unity's L1-style propagation distance.
- **Banker's rounding beats round-half-up by +0.28 pp exact-match** at
 K=32 4-conn, and ties on close%. Float-round in the probe is equivalent
 to round-half-to-even on `sum/cnt` for integer inputs.

## Implementation

`src/alpha_bleed.rs`: replaced `(sum + cnt/2) / cnt` with
`round_half_to_even(sum, cnt)` for all three RGB channels in the per-pass
update. No other changes to passes (32), kernel (4-conn), or termination
(early-out on `!any_added`).

```rust
fn round_half_to_even(sum: u32, cnt: u32) -> u8 {
    let q = sum / cnt;
    let r = sum % cnt;
    let twice = r.wrapping_mul(2);
    if twice < cnt      { q as u8 }
    else if twice > cnt { (q + 1) as u8 }
    else                { if q & 1 == 0 { q as u8 } else { (q + 1) as u8 } }
}
```

A new unit test (`round_half_to_even_matches_python_round`) covers the
exact-half tie behavior on cnt ∈ {2, 3, 4} — the only counts that ever
appear in 4-connectivity.

## Measured delta (banker's rounding vs round-half-up)

| platform | round-half-up Tex2D ppm | banker Tex2D ppm | delta |
|---|---:|---:|---:|
| windows | 267,890 | 267,880 | -10 ppm (-35K bits) |
| mac     |  15,637 |  15,625 | -12 ppm (-15K bits) |

Total absolute closure: -50K bits across both platforms. Small but free,
no regressions, and aligns with what the offline probe predicted.

## Tests

- `cargo test --release --lib` — 116 passed, 0 failed (was 115; +1
 banker's-rounding unit test).
- `cargo test --release --test parity_bytes` — 1 passed; bits-different
 = 773,674 = ceiling unchanged.

## Files

- `src/alpha_bleed.rs` — added `round_half_to_even`, wired into the
 per-pass update; doc-comment refreshed.
- `dev/bc7_probe/probe_bleed_v2.py` — sweep harness for (K, kernel,
 rounding) on the new corpus.
- `dev/fix_proposals/alpha_bleed_v2.md` (this doc).

## What did NOT land and why

- **5×5 box kernel**: -23 pp close% at K=32. Hypothesis (Unity uses a wider
 kernel) is wrong — Unity's dilation propagates strictly N/E/S/W.
- **8-connectivity**: -10 pp close% at K=32. Diagonals carry RGB
 asymmetrically; same conclusion as the v1 probe.
- **K=16/24/40/48**: lose 2-9 pp to K=32 on the aggregate. K=32 is the
 prod stop point; under-iterating leaves fringe gaps, over-iterating
 bleeds into pixels prod left at their unbled values.
- **Per-CID K tuning**: would require a lookup table keyed on source
 content (forbidden by hard constraint). The fixed K=32 captures 95.4%
 of the corpus-wide close-match potential.
