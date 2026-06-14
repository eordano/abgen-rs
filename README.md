# abgen-rs -- an independent reimplementation of Decentraland's asset-bundle-converter

abgen-rs is a pure-Rust reimplementation of Decentraland's
[`asset-bundle-converter`](https://github.com/decentraland/asset-bundle-converter).
The job is file-format interoperability: emitting the AssetBundle
(UnityFS archive) container format that Decentraland clients load at
runtime -- for scenes, wearables, emotes, and Worlds -- directly from the
glb/gltf models and png/jpg images stored on the catalyst content
network. A single Rust binary, deterministic output, and -- because it
reimplements the converter from scratch rather than driving an editor --
no editor-in-a-container to operate: it runs anywhere.

## Why this exists

Decentraland clients don't load raw glTF; they load AssetBundles produced
by Decentraland's own asset-bundle-converter, a service that runs a
headless editor once per entity. Operating that service is expensive,
slow to scale, hard to reproduce, and impossible to audit byte-for-byte --
and it means the preview pipeline and the production pipeline can
disagree. abgen-rs reimplements that converter end to end, removing the
cost and operational burden of running it, so anyone can build
production-equivalent bundles from a content snapshot.

The project has **two goals that pull in different directions**, and the
flag system exists to serve both:

1. **Validation (the parity oracle).** The only way to *prove* a
   clean-room reimplementation correct is to make it produce
   byte-identical output to the real converter on a large, varied corpus.
   Every default in abgen-rs is therefore tuned to match the reference
   converter exactly -- including its bugs and quirks.
2. **Serving (production-equivalent output).** Some of those quirks are
   things you would never want to ship (see `--real-textures` below). A
   second set of opt-in flags diverges from the reference on purpose to
   produce bundles that are correct for a real client.

## How it works

The pipeline mirrors what the converter's import-and-build does, stage by
stage:

```
catalyst entity --> glb/gltf + images --> Unity-equivalent import --> serialized bundles
                   (content snapshot)    meshes, textures,           SerializedFile +
                                         materials, animation,       typetrees, wrapped
                                         hierarchy, PathIDs          in LZ4 UnityFS
```

Concretely: parse glTF -> normalize into a scene model -> emit Unity
objects (Meshes, Texture2Ds, Materials, AnimationClips/Animators,
GameObject hierarchy) -> assign deterministic PathIDs -> serialize a
typetree-encoded SerializedFile -> wrap it in an LZ4-compressed UnityFS
archive. The build is parallelized across cores and scales linearly with
entity count.

What makes this hard is not the file format -- it is that **every
behavior the converter applies during import had to be recovered
black-box**, to the last bit: the BC7/DXT encoding profiles and their
tiebreaks, the
alpha-transparency dilation pass (a jump-flood nearest-seed fill), mesh
normal/tangent recomputation in exact `f32` arithmetic, draco 1.4.1
decode quirks, the AssetBundle preload-table ordering rule, externals
numbering by first PPtr use, Unity's natural sort for dependency lists,
Mecanim controller serialization, and dozens more. Each recovered rule is
written up in [`docs/`](docs/README.md) -- that catalog is the project's
core knowledge.

**The byte-parity philosophy:** a reimplementation that is "close" is
unfalsifiable -- every diff is ambiguous between "our bug" and "harmless
difference". A reimplementation that is *byte-identical* on a large
corpus is proven. So the default build mode chases equality with a fixed
reference, and every deliberate improvement over the reference lives
behind a flag.

## Quickstart

### Build

```bash
cargo build --release
```

A standard stable Rust toolchain. The vendored `draco_decoder` invokes
CMake and `crunch` compiles C++ via the `cc` crate, so you also need a
C++17-capable toolchain and `cmake` on `PATH`:

| Platform | Install |
|---|---|
| **Linux** | `cmake`, a C/C++ toolchain, `pkg-config` (e.g. `build-essential cmake pkg-config`). |
| **macOS** | `xcode-select --install`, then `brew install cmake`. The `draco_decoder` build script pins a recent `-mmacosx-version-min`; older SDKs may need that relaxed. |
| **Windows** | VS Build Tools with the C++ workload (MSVC + Windows SDK + CMake). Use the `*-pc-windows-msvc` toolchain, not GNU/MinGW. Caveat: `crunch`'s build script emits a `pthread` link directive that MSVC will look for as `pthread.lib`; provide a pthreads port or patch the directive to be Unix-only. |

**Runtime dependency -- libjpeg-turbo.** abgen loads `libturbojpeg` at
runtime (via `dlopen`) for byte-exact JPEG decode. Without it, decode
falls back to a pure-Rust path that produces *different pixels*: output
stays visually correct, but every JPEG-sourced texture silently loses
byte-parity with the reference. Install your platform's libjpeg-turbo
package, or set `TURBOJPEG_LIB=/path/to/libturbojpeg.so` explicitly.
Serving builds are fine either way; parity runs are not.

### Configure

| Env var | Purpose |
|---|---|
| `ABGEN_CONTENT_ROOT` | Root of your local content snapshot. CIDs resolve to `<root>/<sha1(cid)[:4]>/<cid>` -- the sharded on-disk layout a catalyst content server keeps. Any catalyst's content mirror works; `abgen-world` writes this layout too. |
| `ABGEN_ROOT` | Where the Unity typetree **template bundles** live (`<ABGEN_ROOT>/template/all-types.<platform>.bundle` etc.). These small reference bundles supply the typetree layouts abgen serializes against. Defaults to the parent directory of the crate. |
| `TURBOJPEG_LIB` | Explicit path to `libturbojpeg`. See the runtime-dependency note above. |

Build-mode env vars (`ABGEN_REAL_TEXTURES`, `ABGEN_V38_COMPAT`,
`ABGEN_FAST_SERVE`, `ABGEN_COLLECTION_MODE`) are described under Build
modes; the CLI flags set them for you.

### Run

```bash
export ABGEN_CONTENT_ROOT=$CONTENT_ROOT       # your content snapshot
./target/release/abgen-corpus --entity-ids entity-ids.txt <output-dir> \
    --platform windows --cdn-layout --real-textures --v38-compat
```

That is the serving recipe: every entity in the list, built into the
CDN-ready layout described below.

## The binaries

| Binary | Purpose |
|---|---|
| `abgen` | Build **one bundle** from one source file: `abgen <glb-path> <bundle-name> <root-hash> <out-path>`. Flags control sibling-URI resolution, entity-type routing, and metadata dependencies. |
| `abgen-corpus` | **Parallel batch build** -- the main entry point. Four input modes: an explicit JSON manifest, `--from-reference <ref-dir>` (rebuild whatever a reference corpus contains), `--entity-ids <file>` (a list of entity CIDs, one per line, `#` comments allowed), and `--collection-urn <urn>` (resolve a wearables collection via a catalyst lambdas endpoint). |
| `abgen-verify` | **Diff a built tree against a reference tree**: walks `<reference>/<entity>/<bundle>`, compares bytes, classifies each bundle by kind, prints per-kind byte-identical counts and a size-delta histogram. `--json` writes a machine-readable report. |
| `abgen-world` | **Fetch a Decentraland World** (`<name>.dcl.eth`) -- scene entity plus all content files -- into a local store-layout cache, ready for `abgen-corpus --entity-ids`. |

Run `<binary> --help` for the full flag listing; the help text is the
authoritative flag reference.

### Output layouts (`abgen-corpus`)

- **default** -- `<out>/<entity_id>/<bundle_name>`: the reference-corpus
  shape, used for parity work.
- **`--flat`** -- `<out>/<bundle_name>`: content-addressed
  `<hash>_<platform>` files, deduplicated across entities.
- **`--cdn-layout`** -- the serving shape (see CDN layout below).

## Build modes

**The default output is byte-faithful to the reference converter, quirks
included.** The reference corpus is produced by the
[`abc-deterministic-guids`](https://github.com/decentraland/asset-bundle-converter/tree/abc-deterministic-guids)
fork of the converter running in headless batchmode, and abgen-rs
reproduces what that setup actually emits -- not what production serves.
Two consequences you must know about:

- **Texture stubs.** Headless batchmode Unity collapses certain oversized
  textures to a flat mean-color block. The reference corpus bakes that
  artifact in, so default abgen output reproduces it: the material binds
  a flat-color stub and the bundle renders **flat gray in a real
  client**. Correct for parity scoring, wrong for serving.
- **Duplicate texture copies.** The fork ships a full-resolution
  *uncompressed* in-glb Texture2D alongside the streamed compressed copy;
  production v38 strips it. Default abgen reproduces the duplicate.

Why keep such defaults? Because the parity corpus is the only oracle that
proves the reimplementation right. If defaults drifted toward "what we'd
prefer", every byte-diff would be ambiguous: our bug, or our improvement?
The defaults pin the oracle; the flags below opt into divergence.

| Flag / env | What it changes | When to use |
|---|---|---|
| `--real-textures` / `ABGEN_REAL_TEXTURES` | Encode real downscaled BC7 for oversized textures instead of the fork-faithful mean-color stub (covers both standalone images and glb-embedded bound textures). | **Required for serving.** Diverges from fork byte-parity. |
| `--v38-compat` / `ABGEN_V38_COMPAT` | Production-v38 structural equivalence: glTFast-style primitive clustering (per-glTF-mesh, accessor-keyed), always-emit `metadata.json` TextAsset, unconditional `DCL_Scene` default Material per glb, exactly one Texture2D per image (drops the fork's unbound uncompressed copy). `ABGEN_V38_TIMESTAMP` pins the metadata build timestamp for reproducible output. | Match what the production asset-bundle CDN actually serves. |
| `ABGEN_FAST_SERVE` | Reduced-effort LZ4-HC compression. Decompressed bytes are identical; on-disk files are slightly larger and build with less CPU. | Serving builds where latency matters more than file size. Never for parity runs. |
| `--collection-mode` / `ABGEN_COLLECTION_MODE` | Match the converter's `ConvertWearablesCollection` entry point, which emits slightly different bytes than per-entity conversion (e.g. it always emits `DCL_Scene.mat`). Implied by `--collection-urn`. | Comparing against a collection-mode reference. |

The serving recipe is `--cdn-layout --real-textures --v38-compat`
(optionally + `ABGEN_FAST_SERVE`); the parity recipe is **no flags at
all**.

## CDN layout

`--cdn-layout` writes the on-disk shape a Decentraland asset-bundle CDN
serves, matching the production CDN's two-step manifest->binary fetch:

```
<out>/<entity_id>/<platform>/<hash>_<platform>     bundle binaries
<out>/<entity_id>/<platform>.manifest.json         per-entity manifest
```

Each manifest carries `version` / `files` / `exitCode` /
`contentServerUrl` / `date`; `files` lists the entity's bundle names.
Shared binaries are hardlinked across entities, so a full-network tree
stays compact. Tune with `--ab-version v<int>` (clients parse the integer
after the `v`) and `--content-server-url <url>`. Bundle filenames match
production CDN names, so abgen output drops into an existing serving path
directly. `--cdn-layout` requires `--entity-ids`.

For a quick zero-infrastructure serve, `abgen-cdn.sh` builds a wearables
collection and serves it in one command, and `abgen-serve.py` is the
underlying HTTP server -- it exposes the production URL shape
(`GET /<vN>/<entity>/<hash>_<platform>`,
`GET /manifest/<entity>_<platform>.json`, `POST /entities/versions`) over
any abgen output tree, so a client's remote-asset-bundle option can point
straight at it.

## Workflows

### Build an arbitrary entity set

The input is a file of entity CIDs, one per line. Any source works: a
catalyst's snapshot files, its `/content` APIs, or an operator's database
-- the active-pointer set (every scene/wearable/emote a client can reach)
is the natural "whole network" list.

```bash
./target/release/abgen-corpus --entity-ids ids.txt <out-dir> \
    --platform windows --cdn-layout --real-textures --v38-compat
```

### Wearables collections

```bash
./target/release/abgen-corpus --collection-urn \
    urn:decentraland:off-chain:base-avatars <out-dir> \
    --platform windows --lambdas-url <catalyst>/lambdas
```

Resolves the collection through any catalyst's lambdas endpoint, then
builds every glb and image in it as a flat content-addressed tree.
Implies `--collection-mode --flat`. Fully standalone: no Unity, no
reference corpus.

### Worlds

Worlds live on the worlds-content-server, not the catalyst network, so
their entities and files are absent from a catalyst content snapshot.
`abgen-world` resolves world names via `/world/<name>/about`, downloads
the scene entity plus every content file into a local cache laid out like
a content store, and writes the entity ids for the normal pipeline:

```bash
./target/release/abgen-world myworld.dcl.eth --store <store-dir>
./target/release/abgen-corpus --entity-ids <store-dir>/entity-ids.txt \
    <out-dir> --content-dir <store-dir> \
    --cdn-layout --real-textures --v38-compat
```

Several worlds can be fetched in one call; re-runs only download what is
missing. `--worlds-url` overrides the server (defaults to the public
worlds-content-server).

### Single bundle

```bash
export ABGEN_CONTENT_ROOT=$CONTENT_ROOT
./target/release/abgen <foo.glb> <bundle-name> <root-cid> <out.bundle>
```

One bundle from one source file. `--source-file`, `--entity-type`,
`--content-map`, `--content-dir` control how sibling URIs and emote
routing are resolved -- `abgen --help` documents each.

## Measuring parity

A **reference corpus** is the output of Decentraland's converter on a list
of catalyst entities, laid out `<corpus-dir>/<entity_id>/<bundle_name>`.
Generate it with the
[`abc-deterministic-guids`](https://github.com/decentraland/asset-bundle-converter/tree/abc-deterministic-guids)
fork (the stock converter is non-deterministic -- Unity assigns sub-asset
fileIDs from a session PRNG, so re-runs of the official converter differ
from themselves). Entity selection lists and regeneration notes live in
[`tests/corpora/`](tests/corpora/).

Rebuild the same corpus with abgen and diff it:

```bash
export ABGEN_CONTENT_ROOT=$CONTENT_ROOT
./target/release/abgen-corpus --from-reference <reference-dir> <out-dir> \
    --platform windows
./target/release/abgen-verify <out-dir> <reference-dir>
```

`--from-reference` reads the reference layout and rebuilds each entity
from the content snapshot; `abgen-verify` prints, per bundle kind, how
many are byte-identical and how the rest split smaller/larger. **The
byte-identical count is the metric that matters**, and it is
deterministic: the same binary on the same corpus always yields the same
number. There is no frozen score in these docs -- run the two commands to
get the current one.

### The render-equivalence taxonomy

Byte-identity is the strongest signal, but the question that actually
matters is whether the client -- a Unity binary -- renders a bundle the
way the user is supposed to see it. A bundle can differ byte-for-byte and
still render pixel-identically, or match the source image and still render
*wrong* if its sampler state is off. So `examples/render_assess <ours>
<ref>` classifies every pair on a second axis, orthogonal to bytes,
organized around what reaches the screen. Each texture bundle is judged by
the worst of three independent things: its **sampler state** (format,
color space, wrap/filter, mips -- how the GPU samples the payload), its
**decoded pixels** (alpha-weighted, judged by the *mean* error across the
image rather than the single worst texel), and its **binding/structure**
(does it load, and does the material's pointer resolve to the texture).

The tiers, from "the user cannot see a difference" to "broken on screen":

| Tier | What it means for the client |
|---|---|
| G1 byte-identical | identical bytes -> identical render, by construction |
| G2 decode-identical | bytes differ (encoder endpoints, preload/LZ4 ordering), but every sampled pixel decodes the same |
| G3 imperceptible | sampler state matches, mean pixel error under ~half a level -- the encoder float-order residual |
| G3b marginal | same residual, ~0.5-2 levels mean -- visible only flipping the two images at full zoom, not in-world |
| G4 non-texture noise | mesh/animation bundle, no texture to sample |
| G5 sampler-state | format / color space / wrap / filter / mips differ -- samples differently even when pixels match |
| G6 visible | alpha-weighted mean past ~2 levels, or real alpha divergence -- a difference a person could notice |
| G7 structural / binding | object/class set differs, a pointer fails to resolve, or dimensions mismatch -- renders a default. Most severe. |
| G8 undecodable | payload the client cannot decode -- renders broken |

Two things this lens makes clear that the byte taxonomy buried:

- **Nothing renders broken.** The structural and undecodable tiers
  (G7, G8) are empty -- every bundle loads, every texture binds, every
  payload decodes in a format the client handles. The entire residual
  above G2 is sub-perceptual or scene-concentrated pixel noise, never
  structure.
- **The residual is one phenomenon: the BC7 float-order wall.** bc7e --
  the open-source Intel/GameTechDev encoder the converter invokes (see
  [`NOTICES.md`](NOTICES.md)) -- and abgen's independent clean-room port
  of it make different-but-valid block decisions. G3/G3b is that wall
  averaged thin; the little that reaches G6 is the same wall accumulating
  on dense, high-frequency PBR art. The decoded pixels are equidistant
  from the source -- two valid quantizations, not a bug -- and the
  divergence is irreducible without matching that encoder's exact
  floating-point op-order.

**Quirk-faithful is not a defect.** A texture can land in G5 or G6
against an older reference and still be correct, because it reproduces
what Decentraland's converter does on import. A texture bound as both a
color and a normal map ships swizzled in linear color space (the
converter's importer type is sticky and never downgrades); ignored EXIF
orientation behaves the same way. For those tiers the question is never
"does this match the source image" but "does this match what the
converter produces from the bytes" -- and matching the converter is the
correct outcome.

As with the byte score, there is no frozen histogram in these docs -- run
`render_assess` over a reference corpus to regenerate the current tier
counts. The full writeup -- the three-axis rationale, and the one
verification step this lens cannot cover (an actual GPU frame; G6 is
exactly the short list worth a rendered-frame check) -- is in
[`docs/methodology/render_equivalence_taxonomy.md`](docs/methodology/render_equivalence_taxonomy.md).

For byte-structure attribution specifically, `examples/classify_pair.rs`
splits pairs into a nine-way *byte* taxonomy (byte-identical; same /
smaller / larger length with id-only or value-noise differences;
structural; build error). Two cautions when reading those results:

- **Never judge a size delta from the compressed file.** Equal raw bytes
  can land at different on-disk sizes (and vice versa) because LZ4
  recompresses different-but-same-length content differently. Decompress
  first -- `examples/rawcmp` splits pairs into structural (different raw
  length) vs value-noise (equal raw length, different bytes).
- **The oracle certifies "matches the fork in batchmode"**, which is not
  the same as "matches production". Production-shape checks go through
  `--v38-compat` plus `examples/dump_census` against a production mirror,
  and visually through texture decode (`examples/dump_tex_png`).

## Inspection and probe tools

Small single-purpose tools live in `examples/`
(`cargo build --release --example <name>`):

| Example | Purpose |
|---|---|
| `dump_census` | JSON structural census of a bundle -- class counts, per-Mesh and per-AnimationClip stats. PathID-free, so it compares across converters. |
| `dump_tex` | List every Texture2D: dimensions, mips, format, stream size. |
| `dump_tex_png` | Decode every Texture2D to PNG (BC7/BC5/BC1/BC3/RGBA32/...) for Unity-free visual inspection. |
| `dump_mat` / `dump_mesh` / `dump_clips` | List material bindings / extract a Mesh / list AnimationClips. |
| `dump_ab` | Dump the AssetBundle object's `m_PreloadTable` / `m_Container` / `m_Dependencies`. |
| `dump_externals` / `dump_decomp` | SerializedFile externals table / raw CAB block decompression. |
| `rawcmp` | Raw-decompressed length comparison across two trees -- the first tool to reach for on a size delta. |
| `objalign` | Align two bundles' objects by PathID; show class/size/name diffs. |
| `render_assess` | Render-equivalence classifier: byte-identity, structural match, per-texture sampler-state field diffs, and alpha-weighted decoded-pixel stats. Drives the G1-G8 tier histogram. |
| `classify_pair` / `diff_classify` | The 9-way byte taxonomy / per-class divergence attribution. |
| `preload_probe` / `clipidxprobe` / `bc7probe` ... | Black-box probes built during specific investigations; see the matching note in `docs/`. |

## Going deeper

- **[`docs/README.md`](docs/README.md)** -- the index of every recovered
  converter behavior and AssetBundle-format rule: encoding rules, ordering
  rules, geometry rules,
  animation, methodology. One note per recovered algorithm, each saying
  what diverged, why it matters, and what the rule is.
- **[`docs/methodology/gaps.md`](docs/methodology/gaps.md)** -- the honest
  list of what is *not* yet derived: the open walls, what is known about
  each, and what would unblock them.
- **`dev/`** -- internal working notes (session logs, fix-proposal
  history). Not part of the guide.

## Architecture

```
src/
|-- builder.rs           the orchestrator: object emission, PathID finalization,
|                        CAB naming, .resS streaming, preload/container ordering,
|                        and the --real-textures / --v38-compat gates
|-- gltf.rs              GLB/glTF parser -> normalized scene::Scene
|-- scene.rs             normalized scene model
|-- mesh_layout.rs       vertex-buffer layout (glTFast-exact)
|-- normals.rs           Mesh.RecalculateNormals reimplementation
|-- tangents.rs          tangent generation
|-- skeleton.rs          skin root-joint / skeleton resolution
|-- materials.rs         DCL/Scene PBR materials + texture trees
|-- animation.rs         legacy AnimationClip + Animation
|-- animation_mecanim.rs Mecanim clip / Animator / AnimatorController (emotes)
|-- alpha_bleed.rs       Unity's alpha-transparency dilation (jump-flood fill)
|-- bc7_pure.rs          BC7 encoder (scalar + AVX2)
|-- bc7_mode_tree.rs     trained mode-prediction tree used by the BC7 encoder
|-- bc5_pure.rs          BC5 (normal-map path)
|-- dxt1_pure.rs         DXT1 PCA encoder
|-- draco.rs             KHR_draco_mesh_compression via vendored google/draco
|-- png.rs               zlib + PNG -> RGBA8
|-- resize.rs            texture resize to power-of-two targets
|-- texprofile.rs        platform texture caps + import decisions
|-- ress.rs              .resS texture-streaming payload + StreamData
|-- lz4.rs               LZ4 + LZ4-HC, byte-exact vs liblz4; ABGEN_FAST_SERVE
|-- pathids.rs           MD4/MD5/XXH64/SpookyHashV2 + prefab_packed PathIDs
|-- cabname.rs           SpookyHash-based CAB-<hex> archive naming
|-- sbp_order.rs         preload + container ordering rules
|-- naming.rs            per-glb deps digest + canonical bundle filenames
|-- value.rs             shared typetree value model (Value / Map)
|-- unity/               SerializedFile v22 + typetree read/write + UnityFS
|-- catalyst.rs          entity resolve + content fetch
|-- local_store.rs       on-disk content store accessor
|-- manifest.rs          CDN on-disk layout + per-entity manifests
|-- wearables.rs         collection-URN resolution + batch build
|-- lods.rs              scene LOD bundle build
|-- shader.rs            vendored DCL/Scene shader bundle reference
`-- ffi.rs               dlopen plumbing (libturbojpeg)

third_party/
|-- draco_decoder/       vendored google/draco (Apache-2.0) + C++ FFI
`-- crunch/              vendored BinomialLLC/crunch (Apache-2.0) + C ABI
```

`shader/scene_ignore_windows` is the vendored DCL/Scene shader bundle
every material references. Typetree templates load from
`<ABGEN_ROOT>/template/` (see Configure).

## Gotchas

- **Missing libturbojpeg silently degrades parity.** See the runtime
  dependency note under Quickstart -- serving output stays visually fine,
  the parity score does not.
- **Content-store layout is sharded.** `ABGEN_CONTENT_ROOT` must point at
  a store laid out `<root>/<sha1(cid)[:4]>/<cid>` (the catalyst
  content-server's on-disk shape), not a flat directory of CIDs.
- **Default output is not servable as-is** where oversized textures are
  involved (flat-gray stub materials -- see Build modes). Use
  `--real-textures` for anything a client will render.

## Discipline

Every parity rule is clean-room: observe reference bundle bytes ->
hypothesize the algorithm -> reimplement independently. **No Unity binary
decompilation, no UnityCsReference, and no Unity Companion License
sources** (Scriptable Build Pipeline, `Unity.Mathematics`, other
`com.unity.*` -- source-available but restricted). Allowed: black-box
observation, genuinely permissive open source (MIT/BSD/Apache -- bc7e,
GLM, glTFast, draco), and public math/specs. No per-CID lookup tables;
every rule must be corpus-verified, not fitted to one example.

**Why this is sound.** The bundles abgen-rs studies are *our own output*:
AssetBundles produced by Decentraland's converter from Decentraland's own
glb/gltf and png/jpg assets. Matching the byte format of output we
generated from our own source is reimplementing the converter against its
own results -- not inspecting any third-party tool. We never decompile,
disassemble, or read the source of anything we did not write; every rule
is recovered by black-box observation of output we are entitled to
observe, and reimplemented independently. The AssetBundle/UnityFS
container is a functional file format, openly parsed by every client that
loads it (and by independent tools such as AssetStudio and UnityPy) -- a
format we interoperate with, not a secret we extracted.

See [`CLEANROOM.md`](CLEANROOM.md) for the full clean-room charter: the
method, the allowed and prohibited sources, and the provenance basis.

## Tests

Validation is corpus-scale: `abgen-corpus --from-reference` +
`abgen-verify` against a reference corpus (see Measuring parity).
`cargo test` covers the smaller in-tree logic tests.

## License

AGPL-3.0-or-later. See [`LICENSE`](LICENSE) and [`NOTICES.md`](NOTICES.md).
