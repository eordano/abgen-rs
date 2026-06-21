# Why asset-bundle conversion fails — diagnosis & remediation

> **Status (2026-06-21): all remediations below are implemented.** The 7 panics
> (B1/B2a/B2b) are fixed (guard-and-fallback, byte-identical for valid input);
> the A1 error is now self-diagnosing (mis-pathed vs never-deployed); and a new
> `--magenta-missing` flag (on `abgen`, `regen_all`, `abgen-serve`) ships broken
> content as renderable **magenta placeholder textures with the failure baked in
> as text** (`MISSING: <file>`) instead of failing — verified end-to-end by
> extracting the texture from a rebuilt bundle. The creator-facing report
> generator is `dev/broken_content_report.sh`. All built + tested in an isolated
> target; parity output is unchanged (magenta is off by default).

Data source: full corpus regen at HEAD `392b29a5c`, windows platform
(`abgenrs-392b29a5c-2026-06-21/_failures.windows.tsv`, the complete 1,875-row
failure dump). mac/linux are the same distribution (the failure causes below are
platform-independent — textures are never a failure source).

## TL;DR

**1,875 / 278,938 windows assets failed (0.67%).** The split is lopsided:

- **~99.6% (1,868) is broken/unconvertible SOURCE CONTENT** — publishing bugs in
  the deployed entities. Decentraland's own Unity `asset-bundle-converter`
  rejects these too; abgen is behaving correctly. **Not abgen's fault, not
  abgen-fixable.**
- **~0.37% (7) are abgen CRASHES** — genuine bugs on degenerate-but-valid glTF.
  These are the only engineering action items. Fixes below.

## Real distribution (windows, deterministically reproduced)

| Count | %     | Signature | Bucket |
|------:|------:|---|---|
| 1,700 | 90.7% | `dep "X" -> "path/X" not in entity content` | A1 broken content |
|    78 |  4.2% | `glTF URI "blob:https://…" / "C:\…" has a URI scheme` | A2 broken content |
|   ~90 |  4.8% | corrupt glb: `EOF parsing` / `glb too short` / `magic mismatch` / `invalid utf-8` / `not JSON` | A3 broken content |
|     7 | 0.37% | `panic: …` | **B abgen bug** |

---

## Bucket A — broken source content (1,868 / 99.6%) — NOT abgen's fault

### A1. Missing texture dependency — 1,700 (90.7%)

**Why it's broken:** these are multi-model "kit pack" scenes. Each model is its
own `.glb`/`.gltf` that references a shared texture by **bare filename**
(`SciFiPack_TX.png`, `file1.png`). abgen correctly resolves that to
`models/<thisModel>/SciFiPack_TX.png`, but the deployer's content build only
copied the texture into **one** model's subfolder. So the texture isn't at the
path the glb points to.

abgen's resolver is correct (case / percent-decode / path-join / `../` traversal
all handled + unit-tested, verified against the live content store). Upstream
Unity fails identically — `GltFastFileProvider.cs:146` throws
`AssetNotMappedException` → `GLTFAST_CRITICAL_ERROR`
(`AssetBundleConverter.cs:418`).

**Drill-down (sample of 200):**
- **~85% mis-pathed** — the texture's basename DOES exist elsewhere in the same
  entity, just under a different folder. → **Republishable** (creator content
  bug); also recoverable by abgen with a basename fallback (see remediation).
- **~15% never deployed** — the texture was never uploaded at all. Unrecoverable
  by anyone without the original asset.

**What we can do:**
1. *(content, the real fix)* republish the affected scenes with textures in the
   referenced folders. These are creator/deployer bugs.
2. *(abgen, high-leverage)* **make the error self-diagnosing** — when a dep is
   missing, check whether its basename exists elsewhere in the entity and say so:
   `texture "SciFiPack_TX.png" is deployed at models/PlantSF_03/… but the glb
   references it as models/PlantSF_12/… (mis-pathed kit-pack asset)`. Turns a
   cryptic line into an actionable one. Site: `naming.rs:296`.
3. *(abgen, opinionated/optional)* a **basename-fallback resolver** behind a flag
   would recover ~85% of these (~1,445 assets) by matching the unique basename.
   **Diverges from Unity parity** (Unity does not do this) → opt-in only, never
   for parity corpora.

### A2. Web-export junk URIs — 78 (4.2%)

**Why it's broken:** 78 models from a single creator, exported from the
blackthread.io web glTF editor with leftover browser
`blob:https://blackthread.io/…` texture URLs (plus one `C:\Users\panag\Downloads`
absolute path). These reference data that only existed in that browser session.

**What we can do:** nothing on the abgen side — genuinely unconvertible; Unity
fails them too. Flag to the creator to re-export with embedded/relative textures.

### A3. Corrupt / invalid glb files — ~90 (4.8%)

**Why it's broken:** the file deployed under a `.glb`/`.gltf` key isn't a valid
glb — truncated (`glb too short: N bytes`), wrong magic (an HTML error page or
text deployed as a model), malformed JSON (`EOF while parsing`), invalid UTF-8.

**What we can do:** nothing on the abgen side — content-side data corruption.
abgen already rejects these cleanly.

---

## Bucket B — abgen bugs (7 / 0.37%) — FIX THESE

These crash (`panic:`) on degenerate-but-valid glTF. They're caught by the regen
`catch_unwind` guard (so they're recorded as failures, not lost), but they should
be clean errors or recovered.

### B1. Empty UV set → out-of-bounds — 5 assets

```
thread 'main' panicked at src/mesh_layout.rs:220:62:
index out of bounds: the len is 0 but the index is 0
  abgen::gltf::vertex_buffer
  abgen::builder::Builder::mesh_tree → add_mesh → attach_primitive → build_node → build
```

`mesh_layout.rs:220` indexes `attrs.uv_sets[ci - CH_TEXCOORD0][v]`. A primitive
whose channel layout declares a TEXCOORD set but whose UV accessor has **0
elements** (while positions has vertices) panics on `[v]`. The neighbouring
`attrs.normals/tangents/colors/weights/joints.unwrap()[v]` accesses
(`mesh_layout.rs:207-227`) have the same latent fault for any attribute whose
length < vertex count.

Entities: `QmeJeGBtqzRSeCsxdqbkQK2JBag1iHwtTyJVehj9mpPoWA::models/Rat_01.gltf`,
`QmW6C6stmsB3aqZzmJp93mjFw6eAxsGS2PJUvn9rXT7SEd::models/Rat_01.gltf`,
`QmZ7t6riQCDV3nLaL29L64G8jdw8WJxAeKybGwyrgcoEA2::{models/Standing_lamp_01.gltf,models/shark.gltf}`,
`QmWk2gGXT6swU3xDRDRqf9EEeXiZC3z6VYVGXVfDWgENbV::…/Piano_01.gltf`.

**Fix:** in `gltf::vertex_buffer` / the layout builder, validate that every
declared channel's data length == vertex count; if a declared attribute is short
or empty, either drop that channel (Unity-like — preferred for recovery) or
return a clean `Err` (conservative). Use `.get(v)` with a default rather than
`[v]` indexing.

### B2. bufferView-less accessor → panic — 2 assets

```
thread 'main' panicked at src/animation.rs:110:49:
accessor bufferView
  abgen::animation::glb::read_accessor_with_buffers
  abgen::animation::build_animation_clips_from_gltf → Builder::build
```

`animation.rs:110` does `acc["bufferView"].as_u64().expect("accessor bufferView")`.
Per the glTF spec, **`accessor.bufferView` is optional** — when omitted the
accessor is zero-filled (used by sparse accessors / placeholders). An animation
sampler pointing at such an accessor is valid glTF, and abgen crashes on it.
(The `panic: bufferViews` failure on `entity123990.gltf` is the same brittle
parser.)

Entities: `bafkreih333…::…/spot_lights_1-v2.glb`,
`QmRFKeawRHZkRRGBtsrfMMxiykvp8EJgKYKhtJskzuVmTL::unity_assets/entity123990.gltf`.

**Fix:** handle a missing `bufferView` by returning `count` zero-filled elements
(spec behaviour). More broadly, the entire `animation::glb` accessor reader
(`animation.rs:~95-130` and siblings) uses `.expect()`/`assert!` — convert it to
return `Result` so malformed animations become clean recorded skips instead of
crashes. (`gltf.rs`'s main accessor reader is already `Result`-based; the
animation path has its own brittle copy.)

### Related hardening (no failures observed, but latent)

- `plan_asset` is **not** wrapped in the `catch_unwind` guard (`regen.rs:402`,
  vs the guard at `regen.rs:298`). A panic there would kill a rayon worker and
  silently undercount failures. Wrap it the same way.
- `finalize_pathids` uses unchecked `old2new[…]` / `.unwrap()`
  (`builder.rs:3013-3025`) — make fallible to pinpoint a bad entity instead of
  crashing.

---

## Recommended actions (prioritized)

1. **Fix B1 + B2** (small, well-scoped). Recovers/cleans the 7 crashing assets
   and hardens against future degenerate inputs. Output for the 278k working
   assets is unchanged (they never hit these paths).
2. **Make A1 self-diagnosing** (`naming.rs:296`): detect basename-elsewhere and
   say so. Highest triage leverage — turns 90% of failures into actionable
   creator feedback.
3. **Emit a creator-facing broken-content report** — `entity_id → plain reason`
   (mis-pathed / never-deployed / web-export-junk / corrupt-glb), generated from
   `_failures.*.tsv`. Lets whoever deployed the content fix it. (A `data:` /
   distribution-side artifact, not an abgen change.)
4. *(optional, opinionated)* basename-fallback resolver behind a flag — recovers
   ~1,445 mis-pathed assets but breaks Unity parity, so opt-in only.

## Reproduce a crash (for fixing)

```bash
cd ab-generator/abgen-rs
STORE=/home/dcl/umbrella/data/content_rust/contents
# content map: SELECT json_agg(json_build_object('file',cf.key,'hash',cf.content_hash))
#   FROM deployments d JOIN content_files cf ON cf.deployment=d.id WHERE d.entity_id='<ENT>';
RUST_BACKTRACE=1 ./target/release/abgen <glb-path-in-store> name <ENT> /tmp/out.bundle \
  --source-file <virtual/path.gltf> --content-dir "$STORE" --content-map /tmp/cm.json
```
(The regen guard swallows backtraces via `catch_unwind`; the standalone `abgen`
binary does not, so use it to get the stack.)
