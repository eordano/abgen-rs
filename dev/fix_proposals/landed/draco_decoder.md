# KHR_draco_mesh_compression — decoder survey, May-25

`src/gltf.rs:396` early-returns on any glb whose `extensionsRequired`
includes `KHR_draco_mesh_compression`. 27 corpus bundles trip this gate
and surface as "unsupported glTF extension". A draco decoder would let
those bundles build; whether they reach **bit-exact** parity vs prod is a
separate, larger question (see "Parity risk" below).

## Crates surveyed (crates.io, )

| Crate | Version | What | Verdict |
|---|---|---|---|
| `draco-oxide` | 0.1.0-alpha.5 | Pure-Rust rewrite by Re:Earth, MIT/Apache | **Decoder unimplemented.** README explicitly: "Mesh Decoder ❌ — Planned for the beta milestone." Encoder + glb transcoder only. |
| `draco_decoder` | 0.0.26 | C++ FFI via `cxx-build`, bundles google/draco third_party (Apache 2.0), MIT/Apache | **Closest option.** Requires `cmake` + C++17 toolchain. Public Rust API drops `unique_id` (would have to rely on sort-by-unique-id ordering convention baked into `cpp/decoder_api.cc`). `decode_mesh_with_config_sync` exists. WIP (0.0.x), single maintainer, no published reverse-deps on crates.io. |
| `draco-rs` | 0.1.3 | C++ bindings via `autocxx` + `cmake` build | Heavier than `draco_decoder` (autocxx + cmake-rs both). Same Google C++ source. |
| `spatial_codec_draco` | 0.2.5 | Point-cloud wrapper around upstream draco | Point clouds only — no mesh decode path. |
| `tmf` | 0.2.1 | Unrelated mesh codec | Not draco. |

No pure-Rust draco mesh decoder exists today.

## Why C++ FFI is risky here (not just risky-in-general)

1. **Build-environment regression.** `Cargo.toml` today is 11 pure-Rust
 deps + zero native build deps. `README.md:98` is the single line
 "cargo build --release". Adding `draco_decoder` introduces `cmake` +
 `g++` requirements. the dev `shell.nix` does not include cmake
 (verified: `which cmake` fails inside `nix-shell shell.nix`). The
 The FHS shell has cmake, but abgen-rs has never required FHS. The
 only existing C-touching path in the crate is the optional
 libjpeg-turbo runtime probe (`dlopen`, not a build dep).
2. **API impedance mismatch.** Google's draco gives each compressed
 attribute a `unique_id` (set when the glb was encoded, referenced
 from the glTF `KHR_draco_mesh_compression.attributes` map). The
 `draco_decoder` crate's `convert_config` (`src/ffi.rs:43-56`) drops
 `unique_id` — the public `MeshAttribute` has only `(dim, data_type,
 offset, length)`. The C++ side does `std::sort(attrs.by_unique_id)`
 before laying them out, so callers must reconstruct the mapping by
 sorting the glTF extension's `attributes` map by value and zipping
 index-by-index. Workable but fragile; an upstream change to the
 ordering convention would silently corrupt outputs. A small patch to
 `draco_decoder` to expose `unique_id` would be the right fix and
 should be upstreamed before any non-experimental adoption.
3. **Cold-build cost.** `cmake --build` of the full draco library plus
 the cxx-bridge compile is ~30 s of extra cold-build time. Currently
 `cargo build --release` is ~25 s; this roughly doubles it.

## Parity risk — why this might land bundles that score *worse* than the current "unbuildable" state

The current state of those 27 bundles is **not "crash"** — `src/gltf.rs:407`
returns a clean error and the parallel-build harness categorises this as
a known coverage gap. The full-corpus ppm-bits headline (442,608 ppm)
**excludes** these unbuildable bundles. Adding a decoder that produces
not-quite-prod-equal bundles would:

- **Convert 27 gap-bundles into ppm contributors.** If each averages
 even a few thousand bits-different, that adds visible noise to the
 headline without paying down any known residual.
- **Cross a Unity-side opaque boundary we have not probed.** Unity's
 draco path is `Draco for Unity` (a Unity-team package, not the same
 binary as google/draco). The decode itself is deterministic
 (quantisation parameters are baked into the compressed stream), so the
 decoded float arrays *should* match — but the path from
 decoded-floats → Unity Mesh m_VertexData / m_IndexBuffer goes through
 a different importer than the non-draco glTF path. Specifically:
 vertex deduplication, vertex reorder for ACMR optimisation, and
 whether Unity recomputes vs trusts incoming normals/tangents are all
 ambiguous on the draco branch and have not been black-box probed.
- **The recent edaf9b9 AABB fix** added the
 `position_min_decl`/`position_max_decl` plumbing precisely because
 draco accessors lack `min`/`max` in the JSON — implying someone
 previously expected to land a decoder and wanted the AABB path ready.
 The plumbing is in place but the parity question is still open.

A useful preflight is missing: rebuild *one* draco bundle with the
prod-side draco decoder + the rest of abgen-rs's pipeline, diff against
prod, and only then decide whether to land a draco decoder at all.

## Recommendation

**Do not land a decoder yet.** Sequence:

1. **Wait for `draco-oxide` decoder** (their roadmap commits to beta).
 That removes the cmake/C++ regression entirely. Re-evaluate
 quarterly. If it ships and exposes `unique_id` per attribute, the
 integration is ~80 lines in `src/gltf.rs` (synthesise bufferViews +
 accessors per decoded attribute, recompute POSITION min/max into the
 existing `position_min_decl`/`position_max_decl` slots).
2. **In parallel: Unity black-box probe.** Pick one of the 27 bundles,
 manually decode the draco buffer using the `draco_transcoder` CLI
 (google/draco's reference encoder/decoder), substitute the decoded
 accessors back into a non-draco copy of the glb, run the resulting
 plain-glb through the existing abgen-rs pipeline, and diff against
 prod. If bits-diff is small, draco integration is worth the C++ FFI
 cost as an interim measure. If bits-diff is large, integration is
 premature regardless of which crate ships first.
3. **If `draco_decoder` is the path forward** (because
 `draco-oxide`'s beta slips and the Unity probe shows acceptable
 parity), upstream a patch first to add `unique_id` to the public
 `MeshAttribute`, then take the dependency.

The 27 bundles remain a known coverage gap; this proposal does not
unblock them, but documents why the cheap-looking path (just add a
crate) is not actually cheap, and what the right preflight is.

## Update — May-25, decoder landed

Vendored `draco_decoder` 0.0.26 into
`ab-generator/abgen-rs/third_party/draco_decoder/` and integrated it
via `src/draco.rs`. Two upstream-bug patches:

1. **`build.rs` lib64 detection** — upstream hardcodes the static lib
 search path to `third_party/draco/build/install/lib`, but CMake
 `GNUInstallDirs` on x86_64 Linux installs to `lib64`. Probe both;
 also make the path absolute (rustc's linker runs from a different
 CWD than build.rs so the original relative path failed to resolve
 regardless).
2. **`utils.rs` / `ffi.rs` `unique_id` exposure** — the cxx-bridge
 `MeshAttribute` already carries `unique_id`, but the public Rust
 `MeshAttribute` dropped it in `convert_config`. Patched both
 `add_attribute` and the public struct to thread `unique_id`
 through. Removes the survey's sort-by-uid ordering-convention
 fragility.

`src/draco.rs::materialize` walks every primitive with a
`KHR_draco_mesh_compression` extension, decodes the buffer, appends
fresh indices + per-attribute byte streams to `buffers[0]`, synthesizes
the matching `bufferViews` + `accessors`, rewrites
`primitive.attributes` + `primitive.indices`, and strips the extension.
Runs in-place inside `load_gltf_inputs`, so the existing parser sees
plain glTF and `position_min_decl`/`position_max_decl` fall back to the
stream-scan path (Unity recomputes AABB from the decoded float stream
for draco meshes — accessors lack JSON min/max).

### Coverage delta

- **All 27 previously-unbuildable bundles now build.** 0 build errors
 on both `pathid_rt_v10_windows` (2174 bundles) and `validation_2`
 (3088 bundles).
- Per-bundle bits-diff for the 27 averages ~493k ppm (range:
 ~3000 ppm to >500k ppm; raw sizes mostly within ±1% of prod).
- **Headline ppm regression (predicted by parity-risk section above):**
 windows full corpus 448,343 → 457,783 ppm (+9,440 ppm), val2 similar.
 The 27 bundles were excluded from the previous headline; now they're
 in-corpus but each contributes a near-worst-case per-bundle diff because
 the converter's draco import path does vertex reorder / dedup / channel layout that
 abgen-rs does not yet model on the draco branch. The bundles are
 built artifacts, not crashes — the gap moved from "unbuildable" to
 "buildable but high-ppm".

### Next steps for the 27

The high per-bundle ppm is the Unity-side draco-importer divergence
the survey flagged. A useful follow-up is a side-by-side diff of one
bundle's decoded vertex stream against Unity's `Draco for Unity`
output — same input, two decoders — to confirm whether the residual is
decoder-side (draco quantisation match between google/draco and Unity's
fork) or post-decode (Unity's vertex dedup / reorder / strip-degenerate
passes). Until that's resolved the 27 bundles ship as known
high-residual cases; the win is that they now produce output and can be
shader-validated end-to-end.
