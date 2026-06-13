# Crunch (CRN) encoder for BC5 normal-map textures — LANDED

**Status :** Vendored BinomialLLC/crunch at
`third_party/crunch/` (public domain), exposed via `crunch_ffi` crate
(C ABI in `cpp/crn_wrapper.cc` + Rust shim in `src/lib.rs`). Dispatch
in `src/builder.rs` now routes BC5-classified images through
`bc5_pure::encode_bc5_normal_crn_mip_chain`, which builds the RGBA
mip chain (post `pack_normal_map` + repack to (R=X, G=Y)) and hands it
to `crn_compress(.. cCRNFmtDXN_XY..)`. CRN magic `Hx` confirmed in
output resS streams at the m_StreamData.offset for all 11 BC5 windows
bundles (UnityPy probe).

**Result:** -7.25 MB across 11 BC5 windows bundles, -46,022 ppm
(514,667 → 468,645). All 10 parity fixtures still pass at-or-below
their per-bundle caps; lib tests 128/128 pass.

Original scaffolding write-up follows for reference.

---

## Original status

Scaffolding in place:

- `src/bc5_pure.rs` — pure-Rust BC5 encoder via `texpresso` crate
 (`texpresso::Format::Bc5.compress`, RangeFit). Handles the DCL
 `pack_normal_map → BC5` swizzle (X→R, Y→G), Unity-layout mip chain with
 bottom-up flip + linear box-halve between mips. 4 unit tests including
 `mip_chain_1024_matches_prod_raw_byte_count` (1,398,128 bytes — matches
 prod's raw BC5 chain size for the 1024² normal textures documented in
 `bc5_normal_trigger.md`).
- `src/texprofile.rs` — `TF_BC5 = 29`, `bc5_normal_profile` (sets
 `m_TextureFormat = 29`, `m_ColorSpace = 0`, `m_LightmapFormat = 3`,
 `m_IsAlphaChannelOptional = false`), `texture_profile_bc5_normal`.
 Sub-block fallback to `TF_RGBA32_UNITY` mirrors `bc7_profile`'s rule.
- `src/materials.rs` — `classify_bc5_normal_images` computes
 `normal_uses ∩ (baseColor ∪ emissive)` per the trigger doc.
- `src/builder.rs` — `bc5_normal_images: HashSet<usize>` populated in
 `build`; texture-emit dispatch wired but parked behind a no-op `let _`
 (see “Why parked”).

## Why parked

Measurement on the 11 BC5-affected windows bundles
(`/tmp/measure_bc5_in_worktree.py` ):

| metric | baseline (BC7-raw fallback) | with raw-BC5 wiring | delta |
|---|---:|---:|---:|
| total ref_bits | 1,376,957,488 | 1,376,957,488 | 0 |
| total diff_bits | 708,674,932 | 723,983,378 | **+15,308,446** |
| ppm | 514,667 | 525,784 | **+11,117** |

Per-bundle disk size grew ~150–200 KB because raw-BC5 block bytes
LZ4-compress differently than raw-BC7 block bytes (the BC5 layout has
much higher run-length redundancy in the 2-byte endpoint pairs, which
oddly defeats LZ4HC match prediction more often than BC7's
mode/partition headers).

The trigger doc's prediction was **≤ 1 ppm net gain** from raw BC5.
Reality is **−11k ppm regression** on the BC5 bundles alone, ~−1.1k ppm
overall windows (since BC5 bundles are ~10 % of corpus by bytes). The
~24 bits / texture metadata win (≈ 384 bits across 16 textures) is
swamped by LZ4 noise. Re-enable dispatch only after the CRN encoder
lands, when the resS bytes will match prod's ~372 KB / 361 KB targets.

## Path to landing — Crunch (CRN) encoder

Prod's 16 BC5 textures are NOT raw BC5 — they are Crunch-compressed
(magic `Hx`, ~3.75× compression ratio vs raw). To land the full
−10.2 MB / −600k ppm fix from `dev/perf/size_delta_deep_dive.md` row 1,
we need to emit CRN-wrapped BC5 streams matching prod's
`m_StreamData.size` byte-for-byte (or close).

### Option A — C++ FFI to BinomialLLC/crunch (preferred)

Vendor `https://github.com/BinomialLLC/crunch` (Apache 2.0) at
`third_party/crunch/` following the `third_party/draco_decoder/`
pattern. Same `cc-build` + `cxx`-style FFI wrapper that draco uses
should work — crunch is C++14, similar build complexity.

Footprint estimate:

- `crnlib/` (~50 source files, ~25k LOC) — only the encoder half is
 needed; decoder can be omitted.
- One FFI entry point: `crn_compress(uncompressed_blocks: &[u8], width,
 height, levels, format=DXN_XY) -> Vec<u8>`. CRN supports DXN (BC5)
 natively (`cCRNFmtDXN_XY`).
- Build via `build.rs` mirroring `draco_decoder/build.rs`. Use
 `nix-shell shell.nix` to get the required cmake / clang stack.

Risks: crunch's CMakeLists is older and assumes msvc-or-gcc; needs a
small Nix patch for clang + recent libstdc++. Same kind of patch we
applied to draco.

### Option B — port the RDO loop to Rust (~3000 LOC)

`BinomialLLC/crunch-cpp` (MIT, the more modern fork) has a cleaner
encoder. The hot path is the rate-distortion optimization loop:
backward-greedy block-pair refinement that re-runs `compress_block`
many times with perturbed endpoints. Doable but Q ≥ 2-week effort for
parity with crunch's quality. Defer until Option A is proven blocked.

### Option C — pre-built `libcrn` system dep

Not in nixpkgs 2026-05. Would have to package; same complexity
as Option A. No advantage.

## How to re-enable dispatch after CRN lands

1. Replace the `let _ = self.bc5_normal_images.contains(&idx);` no-op
 in `src/builder.rs::texture` with the three-way `is_bc5_normal /
 is_dxt1 / else` dispatch (commented in-place; trivial revert from
 git history of this scaffolding commit).
2. Add a Crunch wrap pass on the encoded mip chain:
   ```rust
 } else if prof.texture_format == texprofile::TF_BC5 {
       let (sw, sh) = src.dimensions();
       let packed = pack_normal_map(src.as_raw());
       let (raw_bc5, _mips) = crate::bc5_pure::encode_bc5_mip_chain(
           &packed, sw, sh, Some(prof.mip_count), true);
       // NEW: wrap raw BC5 blocks in CRN container.
       crate::crunch::crn_compress_bc5(&raw_bc5, sw, sh, prof.mip_count)
 }
   ```
3. Verify `m_StreamData.size` lands within ±5% of prod's Crunched size
 for each of the 16 textures (per-bundle list in `bc5_normal_trigger.md`).
4. CRN magic bytes `Hx` must appear at the start of the streamed
 payload (probe with `/tmp/probe_resS_bytes.py`).

## Per-format Crunch policy audit

Prod Crunches *some* formats and not others. Closed by
the full-corpus census (`dev/perf/texture_format_census.md`,
`dev/scout_texture_format_census.py` — 11 366 Texture2D across windows +
val2 = 5262 bundles). Final answer:

- **BC5/DXT5Crunched** (`fmt=29`, normal maps): **Crunched** (21/21
 corpus instances — 16 windows + 5 val2). `Hx` magic. Unity enum byte
 29 is `DXT5Crunched`, not `BC5` (BC5 raw is enum 27); prod's
 TextureImporter ships these via the Crunch container in either
 reading.
- **DXT1** (`fmt=10`, KHR_specular color): **raw** (8/8 corpus
 instances — 7 windows + 1 val2). No magic — direct BC1 endpoint
 pairs.
- **BC7** (`fmt=25`, default): **all raw** (6 711/6 711 corpus
 instances). No `BC7Crunched` (would be `fmt=64` if it existed — but
 Unity enum 64 is actually `ETC_RGB4Crunched`, an Android format with
 zero presence in windows corpus). The "Compressed (High Quality)"
 heuristic in the previous note was wrong; prod's BC7 emission stays
 raw across the entire corpus and ours matches.
- **DXT1Crunched** (`fmt=28`): **0 instances**.

Policy when CRN encoder lands:

| Unity format | Default policy | Trigger |
|---|---|---|
| 29 (DXT5Crunched) | CRN-wrap | normal ∩ (baseColor ∪ emissive) |
| 25 (BC7) | raw | everything else |
| 10 (DXT1) | raw | `_SpecColorMap` only (landed at `88a827d`) |

## References

- `dev/fix_proposals/bc5_normal_trigger.md` — trigger criterion
 (16/16 confirms, 0 false positives) + raw-BC5 ROI analysis.
- `dev/perf/size_delta_deep_dive.md` row 1 — +10.2 MB / 13 bundles
 attributable to BC5+Crunch.
- `dev/fix_proposals/khr_materials_specular.md` (`88a827d`) — DXT1
 landing precedent; DXT1 stays raw because prod ships raw DXT1.
- `third_party/draco_decoder/` — the canonical pattern for vendoring
 a C++ encoder under `third_party/` with a `build.rs` + cxx FFI.
