# AssetBundle opt-in emit-and-verify (`BuildOpts.expect_hash`)

Follow-up to `dev/fix_proposals/assetbundle_shader_slot_rule_v2.md` (negative
result — no content-derivable rule predicts the FIRST/LAST shader-slot
minority on either platform).

The prior doc's recommendation: expose an **opt-in** `expect_hash` field that
lets parity-replay pipelines (which already have prod bundles on hand) build
once with the per-target majority slot, hash-check against the prod bundle's
SHA-256, and on miss rebuild once with the opposite slot. Zero overhead on
forward builds (no hash → identical to today). This is what landed.

This commit *also* finishes the wiring promised by `bc5c9b0`: the
`ExternalsPosition::for_target` constructor was added in that commit but its
call site (`fill_assetbundle`) kept using the default `Last`-everywhere
variant. The two now agree.

## Changes

### `src/sbp_order.rs`
Untouched — `ExternalsPosition::for_target("windows" | "mac") => First, _ =>
Last` was already correct. The `_with` variants were already there from
`bc5c9b0`; we just start using them at the call site.

### `src/builder.rs`

- `BuildOpts.expect_hash: Option<&'a str>` — hex-encoded SHA-256 of the
 prod bundle. `None` (Default) preserves the historical single-build path.
- Inner refactor: the glb-path build moved to `build_glb_with_position`,
 which takes an `Option<ExternalsPosition>` override. `build_bundle` calls
 it once with `None` (= per-target default via `ExternalsPosition::for_target`).
- When `expect_hash = Some(h)`:
 1. Build with per-target default.
 2. SHA-256 the output (`hashes::sha256_hex`); if matches → return.
 3. Otherwise rebuild with the *opposite* slot.
 4. If second matches → return second.
 5. Else (neither matches) → return the **first** build with a
      `warning:` line on stderr. (See "Deviation from the prior sketch" below for
      why first, not second.)
- `Builder::new` gains an `externals_position: Option<ExternalsPosition>`
 parameter; the second-attempt path passes `Some(opposite)`, every other
 caller passes `None`.
- `fill_assetbundle` now calls `build_preload_and_container_with(entries,
 self.externals_position.unwrap_or_else(|| for_target(self.target)))` —
 previously it always passed `Last` via the legacy wrapper, even on
 `windows`/`mac`. This is the latent fix completing `bc5c9b0`.

The `StandaloneTextureBuilder` path is unchanged: standalone-texture bundles
have at most 2 internal entries (texture + metadata) and no externals, so
the position is irrelevant.

### `src/bin/ab-build-local.rs`

Two new CLI flags (mutually exclusive):

- `--expect-hash HEX` — pass the SHA-256 directly.
- `--expect-hash-file PATH` — read from a file (first whitespace-delimited
 token); convenient when the hash is already on disk from a measurement
 pipeline.

`expect_hash` is trimmed before comparison; the compare is
case-insensitive.

### `dev/measure_bits_expect_hash.py`

New driver (sibling of `dev/measure_bits_assetbundle_{windows,mac}.py`).
For each prod bundle:

1. Computes `hashlib.sha256(prod_bytes).hexdigest`.
2. Runs `ab-build-local` once *without* `--expect-hash` (baseline sample).
3. Runs again *with* `--expect-hash <prod_sha256>` (expect-hash sample).
4. Diffs the AssetBundle typetree against prod for both, accumulates
 bits-diff and AB-typetree byte-id counts, and reports the delta.

Reports per-platform subprocess wall-time to confirm the overhead claim.

Env knobs: `ABGEN_PLATFORM` (default `windows`), `ABGEN_VAL_ROOT`,
`ABGEN_AB_BIN`, `ABGEN_LIMIT=<N>` (for smoke tests).

### `tests/parity_bytes.rs`

New `expect_hash_dispatch_is_correct` integration test:

1. Builds a glb fixture, captures its SHA-256.
2. Builds with `expect_hash = Some(<that sha>)` — must return identical
 bytes (idempotent).
3. Builds with `expect_hash = Some("0" * 64)` (impossible match) — both
 slots miss, fallback to the per-target-default build; verifies
 deterministic across two calls.

Plus `MAX_BITS_DIFFERENT: 773_674 → 773_032` (a -642-bit improvement is the
net effect of wiring `for_target` at the call site for the 10 fixture
bundles).

## Empirical results — 22-entity test set (windows + mac, URP v10)

Measured against `workdir/pathid_rt_v10_<platform>/`, full
corpus (windows: 2,174 bundles, mac: 280 bundles).

### Windows (2,174 bundles, 2,158 paired)

| metric             | baseline | --expect-hash | delta |
|---|---:|---:|---:|
| AssetBundle byte-id | 877 | **884** | +7 |
| AB bits differ      | 4,790,820 | **4,789,998** | -822 |
| AB ppm-bits         | 85,606.6 | **85,591.9** | **-14.7** |
| build wall-time     | 321.0 s  | 427.8 s   | **+33.3%** |
| per-bundle wall     | 0.149 s/b | 0.198 s/b | +0.049 s/b |

### Mac (280 bundles, 280 paired)

| metric             | baseline | --expect-hash | delta |
|---|---:|---:|---:|
| AssetBundle byte-id | 206 | **209** | +3 |
| AB bits differ      | 45,133 | **44,741** | -392 |
| AB ppm-bits         | 4,920.3 | **4,877.5** | **-42.7** |
| build wall-time     | 55.7 s  | 106.5 s  | **+91.4%** |
| per-bundle wall     | 0.199 s/b | 0.380 s/b | +0.181 s/b |

### Why the AB-class delta is small (vs. the prior doc's "close to zero" hope)

The prior doc's residual estimate (4,517 ppm windows / 6,701 ppm mac) was
**shader-slot-only**, measured on a 280-bundle / 1-entity corpus where
~32-33% of bundles wanted the minority slot and *no other class had
residual on the rest of the bundle*. The 22-entity corpus is dominated by
**other-class residual**: most AB-class mismatches don't come from the
shader slot, they come from `m_Container` preload sizes / `m_Dependencies`
ordering / Texture2D ress sizes that affect AB structure indirectly. The
shader slot is < 1% of the total AB-class bits-diff on the larger corpus.

The 7 (windows) + 3 (mac) byte-id wins are exactly the bundles where the
shader slot was the **last remaining** AB-class residual.

### Why mac's wall-time overhead is higher than windows'

Mac's baseline is 4,920 ppm — already very close to the floor. Most of
mac's bundles fail the *full-bundle* SHA-256 check (because of other-class
residual we haven't fully closed), so almost all of them trigger the
rebuild path. The 91% overhead matches "most bundles rebuilt".

Windows' baseline is 85,606 ppm. The bigger residual means most bundles
fail the SHA-256 on the *first* slot — but also fail on the *second* —
which triggers our "fallback to first" path that doesn't actually re-emit
the bytes-after-second-build. Net per-bundle wall: only 33% over.

(Both are well within the prior proposal's "up to 2× wall on minority bundles"
upper bound, and well above its "1.05× total" optimistic estimate. The
1.05× would only apply once *all other* class residuals are closed.)

## Deviation from the prior sketch

The prior sketch suggests returning the **second** build on
total miss ("returns whichever") — but empirically this *destroys* the
AB-class byte-id metric, because the AssetBundle typetree of bundles where
the slot was correct (but other classes had residual) is flipped to the
wrong slot. We checked: returning second cost mac's AB-byte-id 206 → 140
(-66, **-32%**) and added +2,770 ppm-bits before any wins.

Returning **first** on total miss preserves the per-target-majority
behaviour for the bundles where the slot wasn't the actual residual cause,
*at no cost to the bundles where the slot WAS the cause* (those byte-match
on second, get returned from step 4). The decision tree is:

```
first matches? → return first (zero rebuild cost)
       no
second matches? → return second (one rebuild, closes a slot-only residual)
       no
neither matches → return first (one rebuild, kept on warning — fallback
                                   to the documented per-target default)
```

The warning text is preserved so downstream tooling can spot total-miss
events and route them to per-class residual investigation.

## Files touched

- `src/sbp_order.rs` — no API change (already had the `_with` plumbing).
- `src/builder.rs` — `BuildOpts.expect_hash`, `Builder.externals_position`,
 `build_glb_with_position`, `hash_matches`, `fill_assetbundle` switched to
 `_with` + `for_target` default.
- `src/bin/ab-build-local.rs` — `--expect-hash` + `--expect-hash-file`.
- `tests/parity_bytes.rs` — `expect_hash_dispatch_is_correct`,
 `MAX_BITS_DIFFERENT` lowered by 642 bits.
- `dev/measure_bits_expect_hash.py` — new before/after measurement driver.
- `dev/fix_proposals/landed/assetbundle_expect_hash.md` — this file.

## Test bars

- `cargo test --release --lib`: 115 passed.
- `cargo test --release --test parity_bytes -- --nocapture`: 2 passed
 (`rust_vs_unity_byte_parity` at 773,032 bits-diff vs ceiling 773,032;
 `expect_hash_dispatch_is_correct`).
- Windows 2,174-bundle corpus AB ppm-bits: 85,606.6 → 85,591.9 (-14.7).
- Mac 280-bundle corpus AB ppm-bits: 4,920.3 → 4,877.5 (-42.7).

## Out of scope / next steps

- **Forward builds** (no `expect_hash`) — by design, no benefit. The
 per-target default is what runs (unchanged behaviour for callers that
 don't opt in).
- **Larger-class residuals** (Texture2D, Mesh, MeshFilter) on the
 22-entity windows corpus are the new dominant terms (~85k ppm-bits each
 vs ~5k for the AB slot). Future work to land those will let the
 `expect-hash` close to actually *zero* shader-slot residual become
 visible at the top-level metric.
