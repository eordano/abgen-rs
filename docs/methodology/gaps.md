# Known gaps and open walls

This is the honest list of what abgen-rs does **not** yet reproduce. Each
entry says what diverges, what has been established about it, and what
would unblock it. Everything here blocks *byte-identity* only — the
affected bundles are structurally correct and render correctly; the
remaining differences are identity values, last-bit arithmetic, or
permutations.

There is deliberately no score in this file. Parity is re-derived, not
quoted: build the reference corpus with `abgen-corpus --from-reference`
and diff it with `abgen-verify` (see the main README, "Measuring
parity"). The 9-way `classify_pair` taxonomy attributes whatever is not
byte-identical to the walls below.

Two recurring unblock paths are referenced throughout:

- **The probe-entity technique.** Substitute a synthetic image or model
  for a real entity's content file, serve the entity to the reference
  converter over HTTP, and read the converter's behavior out of the
  resulting bundle directly. A probe image that imports as *uncompressed
  RGBA32* makes the resize, decode, and mip pixels byte-readable with no
  encoder noise on top. This is how the alpha-bleed fill, the mip chain,
  and several BC7 rules were recovered.
- **An upstream determinism fix.** Some values are not functions of the
  content at all — they come from editor session state on the machine
  that runs the converter. No reimplementation can derive them; the
  reference converter (or its deterministic fork) has to be changed to
  pin them. The fork already pins sub-asset fileID *values*; it does not
  yet pin their *order*.

## Texture walls

Three texture walls that this list used to carry as open — the NPOT resize
filter, the converter's JPEG decode path, and the deep-mip tail — have since
been **closed** (resize filter and JPEG decode derived byte-exact from
uncompressed-RGBA32 probes; the deep-mip diffs turned out to be sub-block
tiling, not a downsampling rule). They are kept here only as a pointer to
the pages that solved them: `../textures/texture_resize_filter.md`,
`../textures/texture_jpeg_decoder.md`, `../textures/bc7_subblock_padding.md`,
`../textures/png_color_management.md`, and the decomposition in
`../textures/standalone_texture_remaining_walls.md`. What remains in the
texture path is the BC7 within-mode encoder tail (below) plus two rare
decode gaps.

### BC7 within-mode encoder tail

Mode selection now matches the upstream bc7e encoder's comparison, and
the pure-Rust port is bit-exact to bc7e at preset. What remains is
endpoint noise within a chosen mode: abgen's solver and the ISPC build of
the same algorithm reduce floats in different orders, so a fraction of
blocks land on neighboring endpoint values — same algorithm, different
last-bit rounding — plus rare genuine tiebreaks where two encodings
decode identically. This is the proven-irreducible float-order wall;
the closing analysis is `../walls/bc7_float_order_taxonomy.md`. Chasing it
means reproducing ISPC's exact lane order and FMA contraction, with
steeply diminishing returns.

### Crunch RDO bytes

BC5 normal maps ship Crunch-compressed (CRN). The vendored crnlib is
calibrated to the converter's quality level and matches the container and
dimensions, but crnlib's rate-distortion search is not byte-stable across
builds, so exact CRN payload bytes stay out of reach
(`../textures/crunch_rdo_calibration.md`).

### PSD decode

PSD-sourced standalone textures have a known decode gap on some files
(`../textures/standalone_texture_midsize_delta.md`); rare in practice.

## Identity walls (session-state, need an upstream fix)

These are proven **not** to be functions of the content. The objects on
both sides are byte-identical; only identity values or orderings differ,
and they cascade through every referencing field.

### Sub-asset fileID order / emote clip index

The converter's `AssetDatabase.AddObjectToAsset` path assigns sub-asset
`localIdentifierInFile` values from a session PRNG — two runs of the
*stock* converter differ from themselves
(`../walls/addobjecttoasset_pathid_probe.md`). The deterministic fork rewrites the
values but keeps the session ordering, so:

- **Emote AnimationClip PathID** — the PathID formula is fully recovered
  (`prefab_packed` over the controller GUID and a clip index), but the
  index is the clip's rank in session-PRNG order. Structurally identical
  emotes land on different indices; no content formula fits
  (`../animation/emote_animclip_pathid.md`).
- **Skeleton bone PathID relabel** — glbs with both a skin and an
  animation relabel bone GameObject/Transform PathIDs through the same
  path (`../walls/skeleton_bone_pathid_relabel.md`).

**Unblock:** an upstream change to the converter fork — sort sub-assets
deterministically before the fileID rewrite. This is a small,
self-contained determinism PR; nothing in abgen-rs can substitute for it.

### Shader-slot position

The position of the URP shader external within each material run is set
by per-target editor state. An exhaustive content-feature search found no
derivable rule (`../walls/assetbundle_shader_slot_rule_v2.md`). The per-target
majority default gets most bundles right; `--expect-hash` closes the rest
when the target hash is known (emit-and-verify). Note the cab-merge
preload rule (`../bundle-format/preload_cab_merge_order.md`) has since subsumed most of
what was attributed to slot position; the residual is small.

## Emote content walls

Beyond the clip-index identity wall, emote bundles have two content-level
opens:

### Mecanim dense-clip content

The converter's native muscle-clip builder (`Internal_BuildClipMuscleConstant`)
produces streamed-clip coefficients with sub-ULP differences from any
reimplementation tried; the arithmetic is opaque to black-box probing
(`../animation/m_muscle_clip_impl.md`, `../animation/glb_emote_clip_size.md`). The clip layout and
length match; the coefficient values carry a residue that also perturbs
LZ4 compressibility.

Related: the **constant-curve split** — which baked curves the converter
collapses into `m_ConstantClip` — is not exactly classified yet; the
naive "all samples bit-equal" rule over-extracts
(`../animation/constant_curve_split.md`, `../animation/constant_curve_classifier_probe.md`).

### AnimatorController `m_TOS` order

The `m_TOS` (hash → name) table content matches entry-for-entry, and the
hash is plain CRC32. The serialization order is **content-deterministic**
— the same name set always produces the same order across entities and
conversion runs, so it is derivable in principle — but it is the
iteration order of an internal hash container in the converter's runtime
whose function has not been identified. A long hypothesis list is ruled out in
(`../animation/emote_animator_tos_order.md`). Byte-cosmetic: closing it alone would
not flip bundles while the clip-index wall stands.

**Unblock:** more hypothesis testing against the (deterministic) observed
orders, or the same upstream determinism PR if it normalizes container
iteration.

## Structural odds and ends

- **Duplicate sibling node names** — recycle-name PathID collisions are
  resolved for the common cases, but some pathological glTFs (deep
  unnamed chains, per-glyph meshes) remain partly irreducible
  (`../bundle-format/gltf_pathid_collision.md`).
- **Transform signed-zero lanes** — a few `-0.0` rotation lanes on
  orientation-root nodes are a SIMD artifact with no content predictor
  (`../mesh/transform_signed_zero.md`).
- **The reference's own artifacts** — the headless-batchmode mean-color
  texture stubs are a property of the *reference corpus*, not a gap in
  abgen (`../textures/standalone_texture_validation_regression.md`). The parity
  oracle certifies "matches the fork in batchmode"; production-shape
  checks go through `--v38-compat` + `dump_census` instead.

## Validation gaps (coverage, not correctness)

- **Collection-URN mode is under-validated** — there is no collection-mode
  reference corpus yet; `--collection-mode` models the known divergences
  but has not been byte-scored (`../pipeline/oversight_collection_urn.md`).
- **Rare glTF features are under-sampled** by the production-distribution
  corpora; a deliberately over-sampling blind-spot entity list exists in
  `tests/corpora/` awaiting a reference run
  (`oversight_corpus_coverage.md`).
