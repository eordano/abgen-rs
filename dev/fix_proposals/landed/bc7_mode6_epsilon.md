# BC7 mode-6 preference epsilon — eps sweep results

## Hypothesis

abgen-rs under-picks mode 6 (1-subset RGBA) vs Unity prod by ~45k blocks
(top-20 windows BC7 textures, 1.35M blocks). Adding a small bias to
non-mode-6 candidate errors before the per-mode-selector comparison
should flip ties toward mode 6.

## Implementation

`src/bc7_pure.rs` adds `nm6_score(err) = err + (err >> MODE6_PREF_SHIFT)`
applied at every non-mode-6 acceptance check in `handle_opaque_block` and
`handle_alpha_block`. Mode 6's own check remains raw — mode 6's err is
compared against the (already-scaled) `best_err`, so a single shift
encodes the asymmetric bias.

## Sweep

Per-class corpus: `workdir/pathid_rt_v10_windows` (22 entities, 2173
bundles, 4708 Texture2D objects). Parity ceiling: 773032 bits.

| `MODE6_PREF_SHIFT` | eps     | T2D ppm | parity total | parity pass | notes                  |
|--------------------|---------|---------|--------------|-------------|------------------------|
| (no bias)          | 0       | 243641  | 773032       | yes         | HEAD baseline          |
| 16                 | 1/65536 | 243641  | 773032       | yes         | chosen                 |
| 15                 | 1/32768 | 243641  | 773032       | yes         | identical effect to 16 |
| 14                 | 1/16384 | n/m     | 774976       | no          | breaks ceiling by 1944 |
| 12                 | 1/4096  | 243643  | 1166645      | no          |                        |
| 10                 | 1/1024  | n/m     | 1166573      | no          |                        |
| 8                  | 1/256   | 243763  | 1266628      | no          |                        |

## Validation_2

Per-class corpus: `workdir/validation_2` (181 entities, 3088 bundles,
5979 Texture2D objects).

| variant            | T2D ppm |
|--------------------|---------|
| no bias            | 213276  |
| `SHIFT=16`         | 213276  |

## Mode histograms (top-20 BC7 windows textures, 1,354,450 blocks)

| variant       | mode 1  | mode 4 | mode 5  | mode 6  | mode 7 |
|---------------|---------|--------|---------|---------|--------|
| prod (target) | 386946  | 21836  | 204005  | 729558  | 12105  |
| no bias       | 411344  | 23511  | 225772  | 684075  | 9748   |
| `SHIFT=16`    | 411344  | 23460  | 226581  | 683313  | 9752   |

## Outcome

The bias is in place and does not regress any tested corpus, but it is
load-bearing only as a tie-break on high-error blocks (err > 65536) and
those blocks barely surface in either corpus. Net Texture2D ppm delta on
test is `0`, on validation_2 also `0`. The chosen shift of 16 is the
largest value compatible with the parity_bytes ceiling — anything
smaller breaks one specific fixture (`bafkreihbgn43gqc3k`) whose 4×4
blocks have mode-1 wins by margins below the per-bit scale of the
shifted error, so the bias flips ~970 of its blocks into mode 6 and
costs ~1944 bits across windows+mac.

The diagnostic narrative in the parent ticket (mode 6 under-pick by 45k
blocks → 142k T2D ppm closable) reflects a stale-binary measurement.
HEAD's encoder already matches no-bias T2D ppm = 243641 on the same
corpus; the 386k baseline that motivated the fix was measured against a
binary built before commits `b5ba6c2`, `70d3da6`, `11191a0`. The
remaining 244k T2D ppm is dominated by the
`TextureImporter.alphaIsTransparency` RGB-bleed gap documented in
`landed/bc7_tiebreak_v2.md`, not mode selection.

Keeping the bias machinery in place at the conservative `SHIFT=16`
preserves the option to tighten the threshold once alpha-bleed lands
and surfaces a different mode-pick signal.

## — probe-anchored sweep, hypothesis re-test

Re-ran the sweep against the synthetic gradient probes
(`dev/bc7_unity_probe/{inputs,unity_out,ours_out}`) plus the parity_bytes
fixture caps at HEAD (`21f784b`). Probe inputs are the four gradients
called out in the parity brief: `grad_h_R_64`, `grad_v_G_64`,
`grad_2d_RG_64`, `srgb_sweep_256x4`. Per-probe baseline (SHIFT=16):

| probe              | nblocks | bit_diff | Unity dominant modes | ours dominant modes |
|--------------------|---------|----------|----------------------|---------------------|
| grad_h_R_64        | 343     | 6106     | m6×302 m1×31         | m6×180 m5×148       |
| grad_v_G_64        | 343     | 7920     | m6×282 m3×44         | m6×180 m5×148       |
| grad_2d_RG_64      | 343     | 21475    | m3×120 m0×98 m1×85   | m5×343              |
| srgb_sweep_256x4   | 129     | 928      | m6×125 m3×3 m5×1     | m6×125 m3×3 m5×1    |
| **total**          | **1158**| **36429**|                      |                     |

`srgb_sweep_256x4` already matches the reference's mode mix bit-for-bit (residual
928 bits comes from endpoint/index drift inside m6, not mode picks).
The other three probes show a single dominant residual: **mode 5 winning
over everything the reference picks** — m6 (for 1D gradients) or 2-subset modes
(for the 2D gradient).

### MODE6_PREF_SHIFT sweep, probe vs parity

| SHIFT | probe bit_diff | bafkreihbgn43gqc3k delta (vs cap) | other fixtures |
|-------|---------------:|-----------------------------------|----------------|
| 18    | 36429          | 0 / 0                             | all at cap     |
| 16    | 36429          | 0 / 0                             | all at cap     |
| 14    | 36429          | +971 / +973 bits (FAIL)           | all at cap     |
| 12    | 36429          | +196806 / +196807 bits (FAIL)     | all at cap     |
| 10    | 36429          | +196770 / +196771 bits (FAIL)     | all at cap     |
| 8     | 36429          | +246797 / +246799 bits (FAIL)     | all at cap     |
| 4     | 36429          | +273074 / +273075 bits (FAIL)     | e23r +213k, bxef +3.7k FAIL |
| 0     | **36563**      | +302031 / +302032 bits (FAIL)     | e23r +218k, bxef +6.9k FAIL |

Two firm conclusions:

1. **The bias multiplier has no influence on probe mode pick in
 [8, 18].** Probe bit_diff is constant at 36429. The mode-5 vs others
 contest is decided by raw error margins many orders of magnitude
 above `err >> SHIFT` for SHIFT ≥ 8, so the multiplicative bias
 cannot move the needle on this residual.
2. **At SHIFT=0 the bias actively hurts probes.** 134 extra residual
 bits appear (mode flips inside `bafkreihbgn43gqc3k`-style blocks
 surface in `grad_h_R_64`/`grad_v_G_64` as well). The tightest
 meaningful bias is already counterproductive for the residual
 originally targeted.

### Diagnosis: the residual is mode-5-over-2-subset, not mode-6-under-pick

The original framing called the dominant texture parity gap "mode 6 picked
where the reference picks a 2-subset mode". The probe data argues the opposite
direction: our encoder over-picks mode **5** (1-subset RGBA with
2-bit indices and rotation) where the reference picks mode 6 (1-subset RGBA
with 4-bit indices) on smooth 1D gradients, and where the reference picks the
2-subset modes (0/1/2/3) on 2D gradients. Mode 5's raw error wins
because:

- 2-bit indices over 16 pixels yield 4 evenly spaced interpolants per
 channel, which is a perfect match for the small per-block range of
 a 4×4 patch of a 64-pixel gradient.
- Rotation lets it spend the alpha-channel slot on the highest-variance
 RGB channel, giving it 8-bit endpoints (vs mode 6's 7-bit RGBA),
 which dominates the SSE-style error on monotone gradients.

`nm6_score(trial_err)` already inflates mode 5's err by `1/(1<<SHIFT)`
before the m6 comparison, but the inflation factor required to flip
the probe blocks is ≥ 1/16 (SHIFT=4), which immediately blows up all
the textured fixtures.

### What would actually move it (not pursued here)

Closing this residual without a multiplicative bias would need one of:

- A **per-block "rotation worth it" rule** for mode 5: only consider
 rotations 1..3 when alpha-channel variance is ≥ some fraction of the
 max RGB channel variance. Unity appears to skip rotations on
 alpha=255 opaque blocks for gradients, ceding the slot to mode 6.
- A **mode-5 disable on low-variance opaque blocks**: detect "alpha
 is constant && per-channel range ≤ K" and bypass mode 5 entirely,
 forcing the contest to mode 1/2/3/6. This is the inverse of the
 brief's "is_low_variance_gradient → penalize mode 6" suggestion —
 the data says we want low-variance gradients to bypass mode **5**.

Both of these are structural changes (not a bias-shift) and need their
own sweep against the full corpus + alpha-bleed-aware regression
fixtures. Leaving the MODE6 epsilon at SHIFT=16 (inert at the probe
scale, no fixture regression) and flagging this as the next direction
for the parity work to take up.
