# TextAsset (`metadata.json.dependencies`) — close 3, landed

Implementation of the minimum-viable fix sketched in `textasset.md`
("Proposed patch (minimum-viable, TextAsset-only)"). Closes 3/3 `TextAsset`
residuals in the 280-bundle corpus by threading sibling-bundle filenames into
`metadata.json.dependencies`, which were hard-coded to `[]`.

## What landed

### A. `BuildOpts.metadata_dependencies: &[String]` — `src/builder.rs`

New field on the public options struct (default `&[]`). Forward-compatible:
existing callers using `..Default::default` keep the empty literal. Field
docs nail the contract: caller supplies pre-deduped sibling-bundle filenames
in the order they should appear in the emitted JSON. The builder does not
reorder.

### B. `Builder.metadata_dependencies: Vec<String>` — `src/builder.rs`

New field on the private `Builder` struct, populated by `Builder::new`'s
extra parameter, which is fed from `BuildOpts.metadata_dependencies.to_vec`
in `build_bundle` (glb-path only — the standalone-texture path already emits
`[]` and is not touched).

### C. Metadata-emit hand-formatter — `src/builder.rs`

Replaces the hard-coded literal:

```rust
let meta_json =
    r#"{"timestamp":0,"version":"7.0","dependencies":[],"mainAsset":""}"#.to_string();
```

with:

```rust
let deps_json: String = {
    let parts: Vec<String> = self.metadata_dependencies.iter()
        .map(|d| serde_json::to_string(d).expect("serialize metadata dep"))
        .collect();
    format!("[{}]", parts.join(","))
};
let meta_json = format!(
    "{{\"timestamp\":0,\"version\":\"7.0\",\"dependencies\":{deps_json},\"mainAsset\":\"\"}}"
);
```

Field order (`timestamp,version,dependencies,mainAsset`) and compact
separators (`","`/`":"`) mirror Unity's `JsonUtility.ToJson` shape. With
`metadata_dependencies = []` the formatter produces a string byte-identical
to the previous literal — verified empirically (md5 match for any bundle
with no external image URIs; the parity_bytes fixtures unchanged).

### D. `parse_gltf_image_uris` + `metadata_dep_bundles_for_glb` — `src/naming.rs`

Two new public helpers:

1. `parse_gltf_image_uris(data, ext) -> Vec<String>` — external image URIs in
 glTF `images[]` iteration order. Differs from the existing
 `parse_gltf_dep_refs` in two important ways:
 - **images-only**: external `buffers[]` URIs are excluded. Per
     `textasset.md` case 3 (`AutoPad.bin`), prod does NOT cross-bundle into
     external glTF buffers — Unity inlines buffer bytes at parse time and
     never produces a cross-bundle PPtr from them.
 - **iteration order, not sorted**: the metadata dep list is order-preserving
     (deduped by bundle filename downstream).
2. `metadata_dep_bundles_for_glb(glb_bytes, glb_file, content_by_file, target)
 -> Vec<String>` — convenience wrapper that resolves each external image URI
 against the entity's content map and converts the resolved content-hash to
 the canonical sibling-bundle filename (`{hash}_{target}`). Missing
 `content_by_file` entries are silently skipped (matches the converter's
 `AssetBundleManifest.GetAllDependencies` output — only assets that ship as their
 own bundle become deps).

### E. `ab-generate.rs` wiring — `src/bin/ab-generate.rs`

Phase 1 worker computes `metadata_deps` before calling `build_bundle` and
passes them via the new `BuildOpts.metadata_dependencies` field. Uses
`naming::metadata_dep_bundles_for_glb` with the entity's `content_by_file`
that's already in scope. Failure returns empty (the build still proceeds —
just with the historical `[]` dependencies, no worse than before).

### F. `ab-build-local` — full driver, beyond textasset.md proposal F

The proposal said "leave as-is" for `ab-build-local`, because the tool has
no scene context. But the parity measurement (`dev/measure_full_vs_prod.py`)
drives `ab-build-local` — so without flag support, the TextAsset fix would
not show up in the measurement. Added:

- `--metadata-dep NAME` (repeatable) — caller supplies sibling-bundle deps
- `--metadata-deps-file PATH` — same but from a file
- `--source-file PATH` — virtual in-entity file path (drives the
 `.gltf` extension sniff + `_emote.glb` gate when the on-disk path is
 content-addressed and has no extension)
- `--content-map JSON` + `--content-dir DIR` — enable the on-disk byte
 resolver. Required for `.gltf` files with external buffer URIs (e.g.
 `AutoPad.gltf` → `AutoPad.bin`); the buffer bytes are needed to read mesh
 accessors. Uses `LocalContentStore` (already in `src/local_store.rs`).

Without any of these flags, behavior is byte-identical to the previous
`ab-build-local` for every input that worked before — verified by hashing
output for the corpus's 280 bundles (md5 stable for all non-`.gltf` glbs).

### G. `dev/measure_full_vs_prod.py` updated

The script now:

1. Resolves the parent entity (parent dir name = entity id) and reads its
 content manifest from the local content store (`load_entity_content`).
2. Derives per-glb metadata deps via `metadata_deps_for_glb` (Python mirror
 of the new Rust helper) and passes them through `--metadata-dep`.
3. For `.gltf` cases, passes `--source-file` + `--content-map` + `--content-dir`
 so the gltf-with-external-buffer case (`bafkreihxu6pmg5u…`) can actually
 build via the glb path rather than mis-dispatching to standalone-texture.
4. Honors `$ABGEN_AB_BIN` and prefers a sibling `target/release/ab-build-local`
 over the main checkout's binary — so running from a worktree picks up the
 worktree's binary without env-var gymnastics.

## Measurement (worktree binary; baseline = same binary + script from HEAD)

Before — `dev/measure_full_vs_prod.py` from HEAD, worktree binary:

```
paired & compared : 280
paired-object byte-exact : 14684/14997 (97.91%)
residuals (313 total):
 Texture2D 113
 Mesh 100
 AssetBundle 74
 MeshFilter 11
 GameObject 8
 Material 4
 TextAsset 3 ← target
```

After — this commit:

```
paired & compared : 280
paired-object byte-exact : 14696/15008 (97.92%)
residuals (312 total):
 Texture2D 113
 Mesh 101 ← +1 (newly comparable in.gltf bundle)
 AssetBundle 74
 MeshFilter 11
 GameObject 8
 Material 5 ← +1 (newly comparable in.gltf bundle)
 TextAsset 0 ← 3 → 0 ✔
```

The two new residuals (Mesh +1, Material +1) are NOT regressions — they
come from the previously-mis-dispatched `.gltf` cases now building through
the proper glb path, which exposes 11 newly-comparable objects per bundle.
9 of those 11 match prod; 2 are new open problems orthogonal to TextAsset.

Bits-different headline (paired-objects level, this metric does not include
the unpaired-prod-objects):

* TextAsset bits-different across corpus: **3 m_Script JSON strings
 differing (~134 bytes off prod)** → **0**.
* Overall paired-object exactness: **97.91% → 97.92%** (+1 paired object
 exact net; +11 paired objects total).

## No-regression verification

1. `cargo test --release --test parity_bytes` — passes; KNOWN_RESIDUALS = 9
 unchanged; 12/21 bundles byte-identical (same as before).
2. md5 of every non-`.gltf` corpus bundle (`bafkrei…_linux` /
 `bafybei…_linux`) under `target/release/ab-build-local` is unchanged
 between the pre-patch and post-patch binary when invoked with no new
 flags. This covers the 277 already-identical-with-`[]`-deps bundles.
3. The default of `BuildOpts.metadata_dependencies = &[]` is preserved by
 all callers that use `..Default::default` (verified: `wearables.rs`,
 `lods.rs`, `regen.rs`, `bin/ab-glb.rs`, `bin/ab-build-local.rs`,
 `tests/parity_bytes.rs`).
4. `StandaloneTextureBuilder` is deliberately untouched — standalone
 textures are leaves with no sibling deps, and the proposal's
 `dependencies = []` literal there was already correct.

## Open problems exposed (next-steps, NOT closed by this commit)

The `.gltf` cases (`bafkreihxu6pmg5u…` / `bafybeidmu6ix6uz…`) now build
through the glb path and expose 2 new diff signatures:

- **Mesh +1** in one of the gltf bundles — a mesh accessor mismatch that was
 invisible while the gltf was mis-dispatching to standalone-texture. Next
 step: forensic per-field diff via `dev/forensic_Mesh.py`.
- **Material +1** in the same family — likely the same root cause as the
 Texture2D-streaming / Material-PPtr family that `textasset.md`'s "wider
 scan" calls out as needing the full `external_texture` mechanism.

Neither is addressed here — the mission was "drive TextAsset to zero", and
those two are pre-existing latent issues the textasset path now surfaces.
