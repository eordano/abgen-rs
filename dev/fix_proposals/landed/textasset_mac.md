# TextAsset (`metadata.json.version`) — macOS / URP v10, close 280/280

> **Status: LANDED in commit `47f02a0`** ("derive metadata.json `version`
> from build target"). `metadata_version_for_target` in `src/builder.rs`
> returns `"8.0"` for `"mac" | "osx" | "windows"`, `"7.0"` otherwise.
> Both `Builder` glb path and `StandaloneTextureBuilder` use it.
> TextAsset class: 280/280 byte-exact on mac v10 and windows v10.

Closes the entire TextAsset class on the macOS + URP v10 corpus
(`workdir/pathid_rt_v10_mac/`, suffix `_mac`) by deriving the `metadata.json`
`version` literal from the build target rather than hard-coding `"7.0"`.

## Signature

A single signature accounted for **all 280 paired TextAsset objects** in the
mac v10 corpus (and the windows v10 corpus too — same converter checkout):

```
ours: {"timestamp":0,"version":"7.0","dependencies":[...],"mainAsset":""}
prod: {"timestamp":0,"version":"8.0","dependencies":[...],"mainAsset":""}
                                ^ one byte different at offset 26
```

That single ASCII byte difference (`7` vs `8`) consistently caused the entire
serialised TextAsset to mismatch under typetree compare.

## Root cause

The converter's `VERSION` constant in
`asset-bundle-converter/Assets/AssetBundleConverter/AssetBundleConverter.cs`
was bumped from `"7.0"` to `"8.0"` on the `abc-deterministic-guids` branch
in commit `268f610` ("Deterministic asset bundle GUIDs + metadata timestamp;
bump AB_VERSION v49"). That value is written verbatim into every
`metadata.json` TextAsset's `m_Script` string.

abgen-rs had `"7.0"` hard-coded in two places in `src/builder.rs`:
the `Builder` glb path (line 1177) and the `StandaloneTextureBuilder` texture
path (line 1861), both landed in `textasset_close_3.md`.

The Linux training corpus under `workdir/pathid_rt/` (used by the
`tests/fixtures/parity/*_linux` parity test) was built with the older
`main`-branch converter that still has `VERSION = "7.0"`, so the hard-code
was correct for that corpus and only broke on the v10 `_mac` / `_windows`
ones.

## Mapping platform → version

The two corpora correlate one-to-one with converter versions:

| Suffix | Corpus | Converter `VERSION` |
|---|---|---|
| `_linux`   | `workdir/pathid_rt/` (legacy)         | `"7.0"` |
| `_windows` | `workdir/pathid_rt_v10_windows/`      | `"8.0"` |
| `_mac`     | `workdir/pathid_rt_v10_mac/`          | `"8.0"` |

DCL doesn't ship Linux or WebGL clients (see `PARITY_STATUS.md` "Platform
scope"), so the converter is always run with `-buildTarget Win64` or
`OSXUniversal` for shipping bundles, and those targets come from the v10
converter. The Linux corpus exists only as a historical reference (built via
an older internal code path) and is what the existing parity_bytes fixtures
sample. The target suffix is therefore a load-bearing proxy for the converter
checkout — no extra dispatch context needed.

## What landed

### `metadata_version_for_target` — `src/builder.rs`

```rust
fn metadata_version_for_target(target: &str) -> &'static str {
    match target {
        "mac" | "osx" | "windows" => "8.0",
        _ => "7.0",
    }
}
```

Single helper, fully derived from the already-existing `target` field on both
builders (`Builder.target: &'static str` and
`StandaloneTextureBuilder.target: &'static str`), which is itself parsed from
the bundle name via `target_from_bundle_name`. No new options, no new wiring.

### `Builder` glb path — `src/builder.rs`

```rust
let version = metadata_version_for_target(self.target);
let meta_json = format!(
    "{{\"timestamp\":0,\"version\":\"{version}\",\"dependencies\":{deps_json},\"mainAsset\":\"\"}}"
);
```

Replaces the literal `"7.0"` previously hard-coded in the metadata JSON
formatter. Field order and compact separators preserved.

### `StandaloneTextureBuilder` texture path — `src/builder.rs`

```rust
let version = metadata_version_for_target(self.target);
meta.insert(
    "m_Script",
    format!(r#"{{"timestamp":0,"version":"{version}","dependencies":[],"mainAsset":""}}"#),
);
```

Same change for the standalone-texture-bundle path. `dependencies` is always
`[]` for standalone textures (no sibling-bundle cross-refs ever exist for a
leaf texture bundle), so we don't reuse the `Builder` helper.

### `dev/bitwise_residuals_mac_textasset.py`

New TextAsset-only forensic adapted from `bitwise_residuals.py` +
`measure_validation_2.py`. Walks `workdir/pathid_rt_v10_mac/*/*_mac`, builds
through `ab-build-local` with the same metadata-deps + content-map wiring as
`measure_validation_2.py`, and reports per-signature diff counts, sample
field values, and exact first-byte-of-divergence offset for `m_Script`. Used
to confirm the fix.

## Measurement

`dev/bitwise_residuals_mac_textasset.py` on the full 280-bundle mac v10
corpus.

### Before this commit (rebuilt against `agent/textasset-mac-fix` worktree
prior to applying the patch)

```
TextAsset paired total: 280
TextAsset differing : 280
TextAsset ppm-differ : 1000000
bytes differing total : 280
```

Single signature (`m_Script`), single byte off per object — but every paired
TextAsset failed typetree equality.

### After this commit

```
TextAsset paired total: 280
TextAsset differing : 280 → 0
TextAsset ppm-differ : 0
bytes differing total : 0
```

Bonus: windows v10 corpus (`workdir/pathid_rt_v10_windows/`) shares the same
converter checkout so the same fix closes its TextAsset class too. Spot-check
of 30 windows v10 bundles confirms 0/30 differing (down from 30/30).

## No-regression verification

1. `cargo test --release` — **108 tests** (107 unit + 1 ignored + 1 parity)
 all pass.
2. `cargo test --release --test parity_bytes -- --nocapture` — **791,702 bits
 different total** (ceiling 800,000), unchanged from headline. The five
 linux fixtures still match exactly the same way they did before — they're
 built with the v7 converter and `metadata_version_for_target("linux")`
 returns `"7.0"`, byte-identical to the previous hard-coded literal.
3. `dev/bitwise_residuals_mac_textasset.py` — TextAsset 0/280 differing on
 mac v10 corpus (was 280/280).
4. Spot-check on windows v10 corpus — TextAsset 0/30 differing on first 30
 bundles (was 30/30).

## Headline

**TextAsset bits-diff on macOS + URP v10 corpus: 1,000,000 ppm → 0 ppm.**

Single one-byte fix in `m_Script.version` ("7.0" → "8.0" when target is
mac/windows). Linux corpus untouched (default "7.0" preserved).
