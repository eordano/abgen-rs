# Research areas — SUPERSEDED (kept for the paper trail)

> **Status (2026-06-10):** this file described the open parity-research
> areas at an earlier point in the effort. Every area below has since been
> resolved, landed, or reclassified as a proven wall. The living documents
> are [`PARITY_STATUS.md`](PARITY_STATUS.md) (current score + walls) and
> [`../docs/README.md`](../docs/README.md) (index of per-fix write-ups).
> Don't pick work from this list.

Disposition of the original ten areas:

1. **Collection-URN bundle-count gap (547 vs 568)** — closed. Texture2D
   emission was decoupled from material binding (one per loadable glTF
   texture); the collection bundle set was verified 1:1 against the
   `ConvertWearablesCollection` output of Decentraland's
   asset-bundle-converter. See
   `../docs/pipeline/oversight_collection_urn.md`, `../docs/pipeline/urn_bundle_gap_session.md`.
2. **`scene_ignore` / shader bundles** — closed via the vendored DCL/Scene
   shader bundle (`shader/scene_ignore_windows`, `src/shader.rs`);
   `--from-reference` runs now build the full reference bundle set with
   zero "abgen emits nothing" entries.
3. **±1…±4-byte size clusters** — largely closed by the size-parity fix
   wave (DCL_Scene emission rules, collider merge, 16-bit PNG truncation,
   constant-clip split, …). A 2026-06-09 `rawcmp` pass found only ~0.5% of
   non-identical bundles still differ in raw decompressed length; what
   remains maps to the walls in `PARITY_STATUS.md`.
4. **Mesh vertex-stream length** — resolved; the channel-inclusion and
   stride rules are implemented. See
   `../docs/mesh/glb_wearable_mesh_stream_length.md`,
   `../docs/methodology/parallel_wave_findings.md`.
5. **Texture mip-chain / streaming split** — resolved, including the NPOT
   power-of-two tie-break (the converter rounds ties UP). See
   `../docs/textures/textures_streaming.md`, `../docs/textures/standalone_texture_size_session.md`.
6. **Draco round-trip determinism** — decode path validated against a
   targeted blind-spot reference corpus; the residual (merged-mesh weld in
   the native draco plugin the converter invokes, affecting normals) is classified
   irreducible. See `../docs/mesh/draco_decoder.md`.
7. **Texture import flags** — landed. See
   `../docs/textures/texture_import_flags_session.md`.
8. **Mesh `m_LocalAABB`** — landed. See `../docs/mesh/bones_aabb_morph.md`,
   `../docs/mesh/bones_aabb_diagonal_corners.md`.
9. **BC7 mode-6 selector post-pass** — superseded: bc7e-faithful mode
   *selection* became the default, removing the accreted empirical biases.
   The remaining BC7 residual is the within-mode float-order wall
   (`PARITY_STATUS.md`). See `../docs/textures/bc7_mode_selection_faithful.md`.
10. **Normal/tangent quantization (1-ULP family)** — investigated to the
    bottom; what remains is last-bit rounding noise without a content
    predictor. See `../docs/walls/mesh-tangent-1ulp_research.md`,
    `../docs/walls/mesh_normal_tangent_quant_session.md`.

The method notes still hold and are worth keeping:

- **Size first, then bytes.** A bundle that is the wrong size can never be
  byte-identical; size mismatches point at structure and are cheaper to
  find than a one-bit float drift.
- **One cause, many bundles.** Prefer hypotheses that explain a cluster,
  and always corpus-verify — single-example fixes regressed at scale more
  than once.
- **Clean-room.** Black-box reference bytes, glTF content, and genuinely
  permissive open source only (see `PARITY_STATUS.md` § Discipline).
