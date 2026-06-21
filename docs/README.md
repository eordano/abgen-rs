# docs/ — the catalog of derived converter behaviors

One markdown note per parity investigation: **what diverged, why it
matters, and what the rule is**. Together these notes are the project's
core knowledge — the behaviors of Decentraland's asset-bundle-converter
and the AssetBundle file format it emits, recovered black-box from
reference bytes alone (no decompilation, no Unity source) and
reimplemented clean-room for file-format interoperability. Those
reference bytes are our own output — AssetBundles the converter built from
Decentraland's own assets — which we are free to observe. Read the main
[`README`](../README.md) first
for the pipeline and the parity methodology; read
[`methodology/gaps.md`](methodology/gaps.md) for what is *not* yet
derived; come here for the detail behind a specific rule.

The notes are grouped by pipeline stage. Each subdirectory below is one
stage; within a stage the most load-bearing rules come first, with the
negative findings and triage sessions after them.

Naming conventions carry through every folder: `*_session.md` = a triage
session over a cluster of diffs; `*_probe.md` = a black-box experiment
against reference bytes; `oversight_*.md` = an audit of something the
corpus or process was missing; platform suffixes (`*_windows`, `*_mac`) =
platform-specific deltas. Many notes record **negative findings** —
hypotheses tested and refuted. Those are kept deliberately: knowing what a
divergence is *not* prevents re-drilling dead ends.

## [`textures/`](textures/) — texture import, encoding, and resize

How a source image becomes a Unity `Texture2D`: the pre-encode pixel
passes, the BC7/DXT/Crunch encoders, the resize and decode filters, the
mip chain, and the streaming gate.

Pixel passes and import flags:

- [`alpha_bleed_jump_flood.md`](textures/alpha_bleed_jump_flood.md) — the `alphaIsTransparency` preprocessing is a jump-flood **nearest-seed fill**, not an averaging dilation; the recovered algorithm.
- [`alpha_bleed_standalone.md`](textures/alpha_bleed_standalone.md) — the alpha-bleed pass exists at all: transparent pixels get their RGB rewritten before BC7 encoding.
- [`alpha_bleed_v2.md`](textures/alpha_bleed_v2.md) — the bleed's RGB mean rounds half-to-even (banker's rounding).
- [`texture_resize_filter.md`](textures/texture_resize_filter.md) — the converter's NPOT/oversize resize filter: a separable cubic in the raw byte domain, derived byte-exact.
- [`texture_jpeg_decoder.md`](textures/texture_jpeg_decoder.md) — the converter's standalone JPEG decode path (islow IDCT + box chroma upsampling + JFIF matrix), read out of the reference.
- [`jpeg_upsampling_split.md`](textures/jpeg_upsampling_split.md) — the box upsampling above belongs to the converter's pre-resize pass only; JPEGs Unity imports directly decode with fancy upsampling.
- [`production_decode_audit.md`](textures/production_decode_audit.md) — corpus-wide decode comparison against the production CDN with source-image refereeing; fork-vs-production divergence catalog (dual-bound normal typing, EXIF orientation, palette PNGs).
- [`png_color_management.md`](textures/png_color_management.md) — which PNG ancillary chunks the converter's decode path honours (`gAMA` applied, `iCCP`/`sRGB`/`cHRM` ignored).
- [`texture_import_flags_session.md`](textures/texture_import_flags_session.md) — the Texture2D import-flag header (readability, streaming flags, color space, filter/wrap/aniso) verified correct (negative finding).
- [`standalone_quality_knob_leak.md`](textures/standalone_quality_knob_leak.md) — model-referenced standalone textures inherit the quality-100 (Slow) BC7 profile from the model importer; pure standalone textures stay Basic.
- [`glb_scene_16bit_png_truncation.md`](textures/glb_scene_16bit_png_truncation.md) — 16-bit PNG downconvert truncates the high byte, never rounds.
- [`per_sampler_textures.md`](textures/per_sampler_textures.md) — one Texture2D per (image, sampler) pair, not per image.

Streaming and the encode targets:

- [`textures_streaming.md`](textures/textures_streaming.md) — the inline-vs-`.resS` streaming gate is `m_IsReadable`, not payload size.
- [`texture2d_residual_v3.md`](textures/texture2d_residual_v3.md) — model-referenced standalone BC7 textures stream into `.resS` regardless of size.
- [`texture2d_followup.md`](textures/texture2d_followup.md) — the upscale-to-power-of-two resize filter for standalone textures.
- [`texture2d_windows.md`](textures/texture2d_windows.md) — the BC7 preset for standalone textures (Basic vs Slow) per code path.
- [`bc5_normal_trigger.md`](textures/bc5_normal_trigger.md) — when Unity selects BC5 + normal-map flag: derivable from the glTF binding alone.
- [`spec_color_dxt5_stub.md`](textures/spec_color_dxt5_stub.md) — textures bound only through `KHR_materials_specular` keep their default-import DXT5 stub plus a linear in-glb copy.

The BC7 encoder (mode selection and tiebreaks):

- [`bc7_mode_selection_faithful.md`](textures/bc7_mode_selection_faithful.md) — Unity runs bc7e at its `slow` profile; mode selection is bc7e's plain error comparison.
- [`bc7_mode1_rule.md`](textures/bc7_mode1_rule.md) — the high-variance gate that reserves mode 1 (vs mode 3) for gradient blocks.
- [`bc7_mode3_mode7_rules.md`](textures/bc7_mode3_mode7_rules.md) — partition shortlisting by variance reduction; coupled pbit/rounding decisions; alpha-driven mode-7 partition scoring.
- [`bc7_mode6_rule.md`](textures/bc7_mode6_rule.md) — mode-6 tiebreak behaviours: endpoint ties, selector snapping, the mode-5 boundary.
- [`bc7_mode6_epsilon.md`](textures/bc7_mode6_epsilon.md) — the mode-6 preference epsilon is inert; re-diagnosis of the real residual.
- [`bc7_m1_p0_selector_lower_tiebreak.md`](textures/bc7_m1_p0_selector_lower_tiebreak.md) — on equal palette entries, the reference keeps the **lower** selector.
- [`bc7_partition_probe.md`](textures/bc7_partition_probe.md) — pixel-feature discriminators for the mode-1 vs mode-6 contest.
- [`bc7_subblock_padding.md`](textures/bc7_subblock_padding.md) — mips smaller than one compressed block (2x2, 1x2, 2x4) fill their block by tiling the mip image, not edge replication.
- [`bc7_tiebreak_v2.md`](textures/bc7_tiebreak_v2.md) — the long-blamed "encoder tiebreak" residual was actually the alpha-bleed input pixels; the pure-Rust port is bit-exact to bc7e at preset.
- [`bc7_texel_walkdown_session.md`](textures/bc7_texel_walkdown_session.md) — the mode-6 walk-down heuristic is *not* a divergence source (negative finding).

Crunch (BC5 normal maps):

- [`crunch_encoder.md`](textures/crunch_encoder.md) — prod's BC5 normal maps are Crunch-compressed (CRN container); vendoring crnlib.
- [`crunch_rdo_calibration.md`](textures/crunch_rdo_calibration.md) — calibrating crnlib's RDO quality level against prod payloads, and why exact bytes stay out of reach.

Triage and size-noise (mostly negative):

- [`standalone_texture_remaining_walls.md`](textures/standalone_texture_remaining_walls.md) — the decomposition of the standalone-texture residual into resize / JPEG / deep-mip / color-management (all closed) plus the BC7 encoder tail.
- [`tex_close_60.md`](textures/tex_close_60.md) — the standalone Texture2D triage that established the streaming gate, the sub-block fallback, and the encoder wall.
- [`mip_chain_arithmetic_verdict.md`](textures/mip_chain_arithmetic_verdict.md) — the mip downsampling arithmetic is correct; the deep-mip diffs are the encoder, not the filter.
- [`glb_scene_large_outliers.md`](textures/glb_scene_large_outliers.md) — for certain oversized glb textures the reference ships a degenerate placeholder BC7 block, not a real encode.
- [`standalone_texture_validation_regression.md`](textures/standalone_texture_validation_regression.md) — the headless-batchmode mean-color stub fires only when Unity's pre-resize step actually ran; origin of `--real-textures`.
- [`load_image_dimension_cap.md`](textures/load_image_dimension_cap.md) — Unity's `LoadImage` gate is a max-dimension cap (8192), not a 32-megapixel pixel count; large-but-loadable glb images were being dropped.
- [`standalone_texture_size_session.md`](textures/standalone_texture_size_session.md) / [`standalone_texture_midsize_delta.md`](textures/standalone_texture_midsize_delta.md) / [`standalone_texture_legacy_mid_deltas.md`](textures/standalone_texture_legacy_mid_deltas.md) — standalone-texture structural size verified correct; deltas are encoder noise (plus a PSD decode gap).
- [`texture_size_delta_lz4_noise.md`](textures/texture_size_delta_lz4_noise.md) — tiny on-disk deltas are LZ4 noise over BC7 texel diffs, not structure (negative finding).

## [`materials/`](materials/) — material binding and external textures

How a glTF material maps to a Unity URP material, including the texture
extensions and how a material points at a texture in a sibling bundle.

- [`material_spec_gloss.md`](materials/material_spec_gloss.md) — `KHR_materials_pbrSpecularGlossiness` drives base color and specular; the metal-rough block is ignored.
- [`khr_materials_specular.md`](materials/khr_materials_specular.md) — routing `specularColorTexture` into the `_SpecColorMap` slot (and its DXT1 encode).
- [`material_close_3.md`](materials/material_close_3.md) — `KHR_texture_transform` tiling/offset and cross-bundle pointer residuals closed.
- [`cross_bundle_externals.md`](materials/cross_bundle_externals.md) — how a material references a texture living in a sibling bundle (externals entry + precomputed PathID).
- [`material_windows.md`](materials/material_windows.md) — cross-bundle texture pointers for plain-`.glb` materials need the source-file path and entity content map.
- [`glb_source_file_enables_external_textures.md`](materials/glb_source_file_enables_external_textures.md) — sibling-texture resolution only activates when a bundle has a `source_file`; without it materials emit null pointers.

## [`mesh/`](mesh/) — geometry, vertex data, and transforms

Vertex streams, recomputed attributes, blend shapes, bounds, draco
decode, and the glTF→Unity transform basis.

Recomputed attributes:

- [`mesh_recalculate_normals.md`](mesh/mesh_recalculate_normals.md) — clean-room `Mesh.RecalculateNormals()` for primitives that ship no NORMAL accessor.
- [`recalculate_normals_f32.md`](mesh/recalculate_normals_f32.md) — the recompute accumulates in **single precision** and has no unit-axis fallback for zero accumulators.
- [`tangent_degenerate_gate_projection.md`](mesh/tangent_degenerate_gate_projection.md) — `RecalculateTangents()`: the degenerate test applies to the projected tangent, and handedness is branch-dependent on the degenerate path.
- [`cylinder_002_uvless_tangent.md`](mesh/cylinder_002_uvless_tangent.md) — UV-less primitives with a normal-mapped material still get a fabricated TANGENT channel.

Blend shapes and bounds:

- [`blendshape_l2_compaction.md`](mesh/blendshape_l2_compaction.md) — morph-delta compaction thresholds on **Euclidean (L2)** delta magnitude.
- [`morph_final_closure.md`](mesh/morph_final_closure.md) — which vertices a morph target keeps and how tangent slots are filled in `m_Shapes`.
- [`bones_aabb_morph.md`](mesh/bones_aabb_morph.md) — per-bone AABBs are recomputed with morph deltas folded in.
- [`bones_aabb_diagonal_corners.md`](mesh/bones_aabb_diagonal_corners.md) — the AABB projection takes all corner combinations of base±delta, not just two points.

Vertex layout and primitive emission:

- [`primitive_emit_decisions.md`](mesh/primitive_emit_decisions.md) — the unified per-primitive decision tree: which primitives produce MeshFilter/Renderer/collider component sets.
- [`multiprim_subm_merge.md`](mesh/multiprim_subm_merge.md) — shared-vertex-stream multi-primitive nodes fold into one multi-submesh Mesh on the parent.
- [`glb_animated_size_session.md`](mesh/glb_animated_size_session.md) — the same merge applies to collider meshes.
- [`meshfilter_close_11.md`](mesh/meshfilter_close_11.md) — shared meshes deduplicate on (mesh, primitive, usage, skin) and every referencing node points at one PathID.
- [`mesh_close_21.md`](mesh/mesh_close_21.md) — mesh-usage flags (orphan-skin zero flag), keep-vertices, and near-zero morph-delta survival rules.
- [`glb_wearable_mesh_stream_length.md`](mesh/glb_wearable_mesh_stream_length.md) — channel inclusion and interleaved stride verified byte-exact (negative finding; the size deltas were compression noise).

Draco decode:

- [`draco_decoder.md`](mesh/draco_decoder.md) — decoding `KHR_draco_mesh_compression` through vendored google/draco.
- [`draco_141_normals.md`](mesh/draco_141_normals.md) — production decodes normals through a draco **1.4.1 integer-abs truncation bug**; reproducing it exactly closes the normal and tangent lanes.
- [`draco_vertex_color_unorm16.md`](mesh/draco_vertex_color_unorm16.md) — draco-decoded COLOR_0 stores as `UNorm16 dim=4`, not Float32; an 8-byte-per-vertex structural difference.

Transforms:

- [`transform_signed_zero.md`](mesh/transform_signed_zero.md) — the glTF→Unity basis flip must not turn `0.0` into `-0.0` on translation.x.
- [`transform_rotation_simd.md`](mesh/transform_rotation_simd.md) — quaternion normalization is per-lane SIMD-shaped; matching it fixes 1-ULP y/w lane flips.
- [`glb_scene_cosmetic_bitflips.md`](mesh/glb_scene_cosmetic_bitflips.md) — the signed-zero family as seen from the glb-scene cohort.

## [`animation/`](animation/) — legacy clips, Mecanim, and emotes

Baked AnimationClips, the Mecanim path (Animator + AnimatorController)
used by emotes, and the `m_TOS` table. The `tos_data/` folder holds the
ground-truth probe data and the hash solver for the open `m_TOS` ordering
problem.

- [`animationclip_content.md`](animation/animationclip_content.md) — keyframe weights, node-path strings, and blendshape attribute naming conventions in baked clips.
- [`animation_duplicate_channels.md`](animation/animation_duplicate_channels.md) — one curve per `(node, target path)`; duplicate glTF channels collapse.
- [`multi_scene_legacy_animation.md`](animation/multi_scene_legacy_animation.md) — glbs with multiple glTF scenes get one legacy `Animation` component per scene, each carrying the full clip list.
- [`rare.md`](animation/rare.md) — morph-target weights animate through AnimationClip float curves and SkinnedMeshRenderer blend-shape weights.
- [`wearable_animation_method_none.md`](animation/wearable_animation_method_none.md) — legacy `type:"wearable"` emotes convert with `AnimationMethod.None`: skeleton-only bundles, no clip, collapsed root.
- [`emote_constant_classification.md`](animation/emote_constant_classification.md) — the streamed-vs-constant split for emote muscle-clip curves, black-box recovered.
- [`constant_curve_split.md`](animation/constant_curve_split.md) — Unity splits baked humanoid curves into streamed and constant groups; the naive bit-equality classifier over-extracts (open).
- [`constant_curve_classifier_probe.md`](animation/constant_curve_classifier_probe.md) — probing the classifier from production bundles alone (negative finding, with durable facts about the split).
- [`m_muscle_clip_impl.md`](animation/m_muscle_clip_impl.md) — a diff attributed to muscle-clip data was actually a missing specular texture; attribution caution for `shift_cascade` tags.
- [`glb_emote_clip_size.md`](animation/glb_emote_clip_size.md) — emote size deltas are muscle-clip coefficient noise changing LZ4 compressibility, not clip-length differences.
- [`m_tos_hash_research.md`](animation/m_tos_hash_research.md) — AnimatorController `m_TOS`: the key hash is plain CRC32 of the name; the serialization order is a separate (open) problem.
- [`emote_animclip_pathid.md`](animation/emote_animclip_pathid.md) — the emote clip PathID formula is recovered, but its sub-asset index is session-PRNG-ordered (wall; needs an upstream converter fix).
- [`emote_animator_tos_order.md`](animation/emote_animator_tos_order.md) — `m_TOS` serialization order is content-deterministic but the internal hash-container ordering is unidentified (wall; full ruled-out list; probe data in `tos_data/`).

## [`bundle-format/`](bundle-format/) — serialization, ordering, and identity

The bundle envelope, the deterministic write orders, the PathID/CAB
naming rules, and the TextAsset/typetree details. Getting an order wrong
permutes bytes without changing content.

Envelope and serialization:

- [`size_delta_windows_mac.md`](bundle-format/size_delta_windows_mac.md) — objects align to 16 bytes (not 8) and the UnityFS flags tag LZ4HC + inline block-info; the envelope rules.
- [`typetree_common_string_zero.md`](bundle-format/typetree_common_string_zero.md) — common-string interning must accept table offset zero (`AABB` is the first entry).
- [`multi_scene_emission.md`](bundle-format/multi_scene_emission.md) — every glTF scene is emitted, not just the default one.

Preload and externals ordering:

- [`preload_cab_merge_order.md`](bundle-format/preload_cab_merge_order.md) — **the cab-merge rule**: each container entry's preload run is its dependency set sorted by `(CAB name, signed PathID)`, internal objects sorting under the bundle's own CAB name. The single rule for all run types.
- [`externals_first_use_order.md`](bundle-format/externals_first_use_order.md) — externals-table slots are numbered by **first PPtr use in serialization order**, not build order.

PathIDs, naming, and identity:

- [`empty_scene_name_wrap.md`](bundle-format/empty_scene_name_wrap.md) — unnamed glTF scenes wrap under a literal `"Scene"` layer, which feeds every descendant's recycle path.
- [`meshfilter_windows.md`](bundle-format/meshfilter_windows.md) — scene-name and `.gltf` wrap rules for recycle-path prefixes.
- [`gltf_pathid_collision.md`](bundle-format/gltf_pathid_collision.md) — duplicate sibling node names and the recycle-name disambiguation that resolves (most of) them.
- [`gltf_container_key.md`](bundle-format/gltf_container_key.md) — the AssetBundle container key uses the real source extension (`.glb` vs `.gltf`), which also moves its sort position.
- [`lfid_wrap_new_game_object.md`](bundle-format/lfid_wrap_new_game_object.md) — the glb importer's hierarchy PathIDs are deterministic; the animated-wrap recycle segment is "New Game Object".
- [`anim_subasset_pathid_validation.md`](bundle-format/anim_subasset_pathid_validation.md) — the recycle-namespace AnimationClip PathID derivation verified at corpus scale.
- [`sf_other_v3.md`](bundle-format/sf_other_v3.md) — legacy CIDv0 CAB hashes are computed from the **lowercased** bundle filename.

TextAsset (the metadata sidecar) and its sort:

- [`sf_other_v2.md`](bundle-format/sf_other_v2.md) — the `metadata` TextAsset is a v3+ (CIDv1) feature; legacy scene bundles must not carry it.
- [`textasset_close_3.md`](bundle-format/textasset_close_3.md) — `metadata.json.dependencies` lists sibling bundles that ship as their own bundle.
- [`textasset_cidv1_sort.md`](bundle-format/textasset_cidv1_sort.md) — `metadata.json.dependencies` uses Unity's **natural sort** (digit runs compare numerically), not byte-lexicographic order.
- [`textasset_mac.md`](bundle-format/textasset_mac.md) — the metadata `version` literal tracks the converter checkout, one ASCII byte that flips the whole TextAsset.

The shader-slot choice and platform targets:

- [`assetbundle_expect_hash.md`](bundle-format/assetbundle_expect_hash.md) — opt-in emit-and-verify (`--expect-hash`) closes the one nondeterministic shader-slot choice when the target hash is known.
- [`assetbundle_windows.md`](bundle-format/assetbundle_windows.md) — target-aware shader-slot defaults and `.gltf` container keys on windows.
- [`assetbundle_mac.md`](bundle-format/assetbundle_mac.md) — the mac default for the shader-slot position.

Compression-noise verdicts (negative findings):

- [`size_delta_v2.md`](bundle-format/size_delta_v2.md) — LZ4HC recompression proven byte-exact, so residual size deltas are input-byte deltas, not compressor config.
- [`sf_pad_lz4_noise.md`](bundle-format/sf_pad_lz4_noise.md) — why a correct change can make a bit-level parity metric look worse (metric composition).

## [`pipeline/`](pipeline/) — entity routing and bundle-set selection

What the converter emits for a given entity, the collection-URN mode, the
legacy "other" bundle kind, and the per-platform class audit.

- [`other_kind_drill.md`](pipeline/other_kind_drill.md) / [`other_kind_validation.md`](pipeline/other_kind_validation.md) — the catch-all "other" bundle kind is legacy CIDv0 standalone textures; classifier attribution confirmed at scale.
- [`oversight_collection_urn.md`](pipeline/oversight_collection_urn.md) — what differs between per-entity and collection-URN conversion, and what a collection-mode reference would validate.
- [`urn_bundle_gap_session.md`](pipeline/urn_bundle_gap_session.md) — the collection bundle-count gap is content drift, not a missing emit rule (confirmed negative).
- [`mac_class_audit.md`](pipeline/mac_class_audit.md) — per-class mac-vs-windows audit: no mac-only emission defect (negative finding).

## [`walls/`](walls/) — proven-irreducible closures

These pages exist to say **do not re-drill this**. Each one root-causes a
residual to native float arithmetic or Unity editor session state and
shows, with evidence, that no content-derivable rule can close it. The
falsification matrices and negative-result evidence live here on purpose —
they are the project's institutional memory.

- [`bc7_float_order_taxonomy.md`](walls/bc7_float_order_taxonomy.md) — the BC7 within-mode residual is irreducible: it is the ISPC-compiled bc7e's per-candidate float-summation order, not our SIMD lane order or any tie rule.
- [`mesh_float_residue_families.md`](walls/mesh_float_residue_families.md) — the surviving mesh f32 diffs fall into exactly two lanes (skin BlendWeight, or tangent), depending only on whether the mesh is skinned — native arithmetic ordering.
- [`tangent_cat7_residue.md`](walls/tangent_cat7_residue.md) — the CAT7 matched-id residue is one family: `RecalculateTangents` arithmetic in the tangent lane.
- [`mesh-tangent-1ulp_research.md`](walls/mesh-tangent-1ulp_research.md) — the 1-ULP tangent rounding family is irreducible boundary noise.
- [`mesh_normal_tangent_quant_session.md`](walls/mesh_normal_tangent_quant_session.md) — the remaining normal/tangent bit residual traces to native arithmetic ordering.
- [`assetbundle_shader_slot_rule_v2.md`](walls/assetbundle_shader_slot_rule_v2.md) — exhaustive search for a content-derivable shader-slot rule; none exists.
- [`addobjecttoasset_pathid_probe.md`](walls/addobjecttoasset_pathid_probe.md) — proof that stock Unity sub-asset fileIDs come from a session PRNG (the reason the deterministic fork exists).
- [`skeleton_bone_pathid_relabel.md`](walls/skeleton_bone_pathid_relabel.md) — skin+animation bundles relabel bone PathIDs through the same nondeterministic path.

## [`methodology/`](methodology/) — how the rules were found

The probe technique, the oracles, the corpus design, and the triage
discipline of separating structure from noise. **[`gaps.md`](methodology/gaps.md)
is the consolidated open-walls list** — what is missing, what is known
about each gap, and what would unblock it; it is re-derived, never scored.

Probing and measurement:

- [`unity_probe_normals_and_ac_clip.md`](methodology/unity_probe_normals_and_ac_clip.md) — driving a live asset-bundle-converter instance against controlled inputs to read importer behavior out directly (the probe-entity technique).
- [`parallel_wave_findings.md`](methodology/parallel_wave_findings.md) — consolidated diagnostic conclusions from a parallel triage wave (cascade effects, ULP families, measurement artifacts).

Corpus design and audits:

- [`oversight_corpus_coverage.md`](methodology/oversight_corpus_coverage.md) — what the validation corpus actually exercises, and the features it under-samples.
- [`render_equivalence_taxonomy.md`](methodology/render_equivalence_taxonomy.md) — classifying bundles by what the client renders (sampler state vs decoded pixels vs binding), orthogonal to the byte taxonomy; the `render_assess` tool and its tiers.

Triage sessions (cluster-level, mostly negative):

- [`glb_scene_size_session.md`](methodology/glb_scene_size_session.md) / [`glb_scene_size_class_val300.md`](methodology/glb_scene_size_class_val300.md) — the glb-scene small size-class hides no structural defect.
- [`glb_wearable_big_size_session.md`](methodology/glb_wearable_big_size_session.md) — wearable size outliers are BC7 texel + LZ4 noise.
