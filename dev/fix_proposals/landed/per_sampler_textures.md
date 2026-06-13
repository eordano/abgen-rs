# Per-sampler Texture2D duplication

**Status: implemented** — closes 53 Texture2D residuals (113 → 60 across the
corpus). Target bundle `bafybeiczim5cqrv` goes from 53 paired Texture2D diffs
+ 2 unpaired (`only-prod`) to 0 of either kind.

## Symptom

Pre-fix bitwise diff for `bafybeiczim5cqrv`:

- **52 paired Texture2D diffs**, signature `[.m_StreamData.offset]` only —
 every streamed BC7 texture's offset shifted by ~67 MiB vs prod.
- **1 paired diff** on image_4 (4096×4096 RGBA32, pid 670010139831010012):
 `.image data: len=0`/`67108864`, `.m_IsReadable: False`/`True`,
 `.m_StreamData: streamed`/`empty` — we put it in the `.resS`, prod kept it
 inline.
- **2 unpaired Texture2D in prod** (`image_4_sampler0` BC7 + uncompressed) —
 we never emitted them.

The 52 offset shifts and the 1 image_4 diff have the **same root cause**: we
streamed a 64 MiB uncompressed texture that prod keeps inline, so every
subsequent texture's `m_StreamData.offset` was off by 64 MiB.

## Two coupled bugs

### Bug 1 — `Builder::texture` keyed cache on `image_idx` only

```rust
// --- old ---
tex_pid: HashMap<usize, i64> // image_idx -> Texture2D pid
fn texture(&mut self, scene, img_idx: Option<usize>) -> Option<i64> {... }
```

When a glTF image is referenced through more than one sampler, prod emits a
distinct Texture2D per `(source, sampler)` pair — the first encountered
sampler keeps the plain `image_N` name; later samplers append `_samplerM`
where `M` is the glTF sampler index (e.g. `image_4_sampler0`). Both the
uncompressed in-glb variant AND the streamed BC7 variant are duplicated.

The old cache silently coalesced them, so:
- Only one Texture2D was emitted per image, regardless of distinct samplers.
- The "winning" sampler's wrap/filter was baked into every material's slot,
 even materials whose glTF reference asked for a different sampler.
- Two Texture2D objects in prod simply had no counterpart in our build.

`bafybeiczim5cqrv` is the only bundle in the 280-bundle corpus that exercises
this — image_4 is referenced through sampler 0 (one material: `Squaretexture`)
and sampler 1 (40+ ADSBOXLOGO and Brand materials).

### Bug 2 — Multi-sampler uncompressed must stay inline

Just adding the per-sampler dup _regresses_ the bundle: now we stream *two*
67 MiB uncompressed Texture2Ds instead of one, and the offset cascade gets
worse (52 diffs → 101 diffs in the measurement that found this).

Prod keeps both `image_4` and `image_4_sampler0` **inline** (`image data`
non-empty, `m_StreamData` empty, `m_IsReadable=True`) — only the BC7 dups
go in the `.resS`. Walking the prod corpus confirms the rule:

> The in-glb uncompressed Texture2D for an image is kept inline + readable
> iff that image is referenced through more than one distinct glTF sampler.

Across 280 bundles, exactly two inline glb-builder Texture2D objects exist
(both image_4 variants in `bafybeiczim5cqrv`). Larger one-sampler images
(6600×6600 in `bafkreiapldwfo4`, 3840×2160 in `bafybeihvw4h2`) all stream
normally, ruling out any "raw size" cutoff. Multi-sampler is the only
predictor that matches the observed layout.

## Implementation

### Scene model — carry sampler alongside image index

`scene.rs`:

```rust
pub struct TexRef { pub image: usize, pub sampler: Option<usize> }
pub struct Sampler { pub mag_filter, min_filter, wrap_s, wrap_t: Option<i64> }
pub struct Material {
    pub base_color_image:        Option<TexRef>,   // was Option<usize>
    pub emissive_image:          Option<TexRef>,
    pub normal_image:            Option<TexRef>,
    pub metallic_roughness_image: Option<TexRef>,
    pub occlusion_image:         Option<TexRef>,
    ...
}
pub struct Scene {
    ...
    pub samplers: Vec<Sampler>,   // new — full glTF samplers list
    ...
}
```

`gltf.rs` now resolves each `textureInfo` into a full `TexRef`:

```rust
let tex_ref = |tex_info| -> Option<TexRef> {
    let ti = ... ;
    let tex = &textures[ti];
    let image  = tex.get("source")?.as_i64()? as usize;
    let sampler = tex.get("sampler")
        .and_then(|s| s.as_i64())
        .filter(|&i| i >= 0 && (i as usize) < samplers.len())
        .map(|i| i as usize);
    Some(TexRef { image, sampler })
};
```

The per-image fallback (`image_sampler`, `image_wrap`) is preserved for
callers that don't carry a sampler — Texture2D emission prefers the per-
sampler lookup when `TexRef.sampler` is `Some`.

### Builder — per-(image, sampler) cache + first-sampler-wins naming

`builder.rs`:

```rust
tex_pid: HashMap<(usize, Option<usize>), i64>, // was HashMap<usize, _>
tex_name: HashMap<(usize, Option<usize>), String>,
tex_first_sampler: HashMap<usize, Option<usize>>, // new — name policy
```

The first sampler seen for each image wins the unsuffixed `image_N` name;
later samplers for the same image get `image_N_samplerM` where `M` is the
glTF sampler index (not a sequential counter). When `TexRef.sampler` is
`None`, the texture falls back to the historical "image_N" naming —
covering the single-sampler path verbatim.

```rust
let key = (idx, tex.sampler);
if let Some(&pid) = self.tex_pid.get(&key) { return Some(pid); }
let first_sampler = *self.tex_first_sampler.entry(idx).or_insert(tex.sampler);
let name = if tex.sampler == first_sampler {
    format!("image_{idx}")
} else {
    match tex.sampler {
        Some(s) => format!("image_{idx}_sampler{s}"),
        None    => format!("image_{idx}"),
    }
};
```

Filter/wrap lookup also goes per-sampler now — `scene.samplers[tex.sampler]`
if `Some`, otherwise the historical per-image fallback. This is the part
that actually fixes the silent wrap-mode leak.

### Inline-uncompressed for multi-sampler images

A new `force_inline_tex: HashSet<i64>` tracks temp pids whose in-glb
uncompressed Texture2D must stay inline. Populated up front from the
multi-sampler set:

```rust
// at build startup:
for m in &scene.materials {
    for (_, accessor) in MATERIAL_TEXTURE_SLOTS.iter() {
        if let Some(tr) = accessor(m) {
            self.image_distinct_samplers
                .entry(tr.image).or_default().insert(tr.sampler);
        }
    }
}

// inside texture:
let n_distinct = self.image_distinct_samplers.get(&idx).map(|s| s.len).unwrap_or(1);
let multi_sampler_uncompressed = !unc_p.compressed && n_distinct > 1;
let mut inglb_tree = self.texture_tree_with_wrap(...);
if multi_sampler_uncompressed {
    inglb_tree.insert("m_IsReadable", true);
}
let inglb = self.add("Texture2D", inglb_tree,...);
if multi_sampler_uncompressed { self.force_inline_tex.insert(inglb); }
```

`finalize_pathids` remaps the set from temp→final pids, then `commit`
threads it through `commit_objects`:

```rust
fn commit_objects(..., inline_pids: &HashSet<i64>,...) {
    let pred = |t: &TextureBlob| inline_pids.contains(&t.path_id);
    let predicate = if inline_pids.is_empty() { None } else { Some(&pred as _) };
    let built = Some(ress::build_ress(blobs, &cab, predicate));
    ...
    for (pid, sd) in &b.stream_data {
        if let Some((tn, tree)) = objects_final.get_mut(pid) {
            if tn == "Texture2D" {
                tree.insert("m_StreamData", ...);
                if !sd.path.is_empty() {              // <-- new guard
                    tree.insert("image data", Bytes(vec![]));
                }
            }
        }
    }
}
```

The new guard `!sd.path.is_empty` is essential: `build_ress` returns
`StreamData::empty` for inline-predicate blobs, and the old unconditional
`image data = []` rewrite was zeroing them anyway. Only actually-streamed
entries (non-empty `.resS` path) get their inline data wiped.

`ress::build_ress` already supported an inline predicate via existing API
(used by the standalone-texture path), so no `ress.rs` change was needed.

## Measurement (corpus-wide)

```
                                  before    after    delta
paired-object byte-exact 14684 14740 +56
                                 (97.91%)  (98.27%)
Texture2D residuals 113 60 −53
 bafybeiczim5cqrv 53 0 −53 (+2 unpaired closed)
 (60 standalone bundles) 60 60 0 (pre-existing BC7 byte diffs)
Material residuals 4 3 −1 (mat 69 binding fixed)
Mesh / AssetBundle / others unchanged
```

Target bundle `bafybeiczim5cqrv`: Texture2D went from 53 paired diffs + 2
`only-prod` to **0 of either**. Mat 69 (`Squaretexture`) now correctly binds
its `_BaseMap`/`_EmissionMap` to the `image_4_sampler0` BC7 pid
(2462010952954616097) instead of inheriting mat 6's `image_4` (sampler 1)
binding — the 4th Material residual the bitwise report flagged in this
bundle.

The 60 remaining Texture2D residuals are unrelated single-image BC7 byte
diffs in standalone-texture bundles (the `bafkrei…` cohort, one per bundle).
Those are encoder-level pixel differences, not bundle-layout, and are
untouched by this change.

`cargo test --release` passes (104 unit + 1 parity_bytes).

## Files touched

| file | change |
|---|---|
| `src/scene.rs` | new `TexRef`/`Sampler` types; `Material.*_image` typed `Option<TexRef>`; `Scene.samplers: Vec<Sampler>` added |
| `src/gltf.rs` | `tex_image` → `tex_ref` returns the full pair; parses `samplers` into the `Sampler` struct; preserves per-image fallback maps |
| `src/materials.rs` | `MATERIAL_TEXTURE_SLOTS` accessor type → `Option<TexRef>`; `classify_texture_colorspaces` unpacks `.image` |
| `src/builder.rs` | per-`(image, sampler)` cache; first-sampler-wins naming; `image_distinct_samplers` precompute in `build()`; `force_inline_tex` + remap in `finalize_pathids`; `commit_objects` takes `inline_pids: &HashSet<i64>` and guards the `image data` zero on `!path.is_empty()` |

Files this fix did NOT need to touch: `tangents.rs`, `sbp_order.rs`,
`bc7_pure.rs`, `skeleton.rs`, `mesh_layout.rs`, primitive-emit in
`builder.rs`, and `ress.rs` itself (its `inline_predicate` API already
supported the need).
