# Parity status

## What "byte-parity" means here

A bundle is *byte-identical* when abgen-rs, given the same catalyst entity
and target platform, produces a file whose bytes equal the reference bundle
built by Decentraland's `asset-bundle-converter` exactly. No fuzzy matching,
no "semantically equivalent" — equal bytes or not.

The reference is **not** the production CDN. It is the output of the
[`abc-deterministic-guids`](https://github.com/decentraland/abc-deterministic-guids)
fork of `asset-bundle-converter`, run in headless batchmode against a fixed
entity corpus. The fork exists because the stock converter is itself
non-deterministic: Unity's `AssetDatabase.AddObjectToAsset` assigns
sub-asset fileIDs from a session PRNG, so two runs of the *official*
converter on the same input differ. The fork rewrites sub-asset ids
deterministically, giving a stable oracle to diff against. Corpus entity
selection lives in `tests/corpora/` (the `validation300` list is the
standard scoreboard); on this machine the generated references sit in
`../unity-reference-ab/ad0564d-val300-{windows,mac}/`. Regeneration notes:
`tests/corpora/README.md`.

Parity is scored with **defaults only** — no `--real-textures`, no
`--v38-compat`, no `ABGEN_FAST_SERVE`. Those flags deliberately diverge
from the reference (see the main README's "Build modes").

## How to reproduce the score

Run inside `dcl-shell` (or with `TURBOJPEG_LIB` set — a wrong/missing
libturbojpeg silently changes JPEG decode and costs ~300 byte-identical
bundles; see the README gotcha):

```bash
export ABGEN_CONTENT_ROOT=/home/dcl/umbrella/data/content_server/contents
./target/release/abgen-corpus --from-reference \
    ../unity-reference-ab/ad0564d-val300-windows /tmp/abgen-val300 \
    --platform windows -j 80
./target/release/abgen-verify /tmp/abgen-val300 \
    ../unity-reference-ab/ad0564d-val300-windows
```

The byte-identical count is the metric. It is deterministic: the same
binary on the same corpus always yields the same number.

## Current score

Don't quote a number here — re-derive it. Run the commands above against
the val300 corpus; the byte-identical count is whatever that prints. A
parity number written into this file goes stale the next time a wall falls,
which is exactly the trap the rest of the docs avoid.

**Windows and mac are now in lock-step.** Not only is the count equal —
the *byte-identical set is identical across platforms*: every byte-identical
bundle is byte-identical on **both** windows and mac, and there is **zero**
bundle that is byte-identical on one platform but not the other. There is no
longer any "windows↔mac gap". (Verify with two `abgen-verify` runs, one per
platform, and intersect the byte-identical lists.)

The old "preload coin-flip" framing — a large win↔mac flip that cost
whichever platform you didn't tune against a chunk of bundles — is **dead**. It
was an artifact of the pre-cab-merge era (the old mac baseline of 2,543
predated the entire wall-fall wave). Once the preload-table ordering rule
fell (`d2b216e`: each container entry's preload run is the dependency set
sorted by `(CAB-name-lowercase, signed pathID)` ascending), mac preload
order became fully determined, exactly like windows. See the re-verdict in
§1 below.

The only kind where the mac reference still structurally diverges from
windows is **glb-emote** (17 of 21 bundles). This is a *stale-reference*
artifact, not an abgen defect: the mac reference (`ad0564d-val300-mac`) was
never regenerated after the deterministic sub-asset doc-order fork fix
(`974f971`), so its emote controllers still carry the old nondeterministic
YAML-doc ranks. The windows reference (`974f971-val300-windows`) *was*
regenerated. Exclude glb-emote from any mac verdict until the mac reference
is regenerated. Excluding glb-emote, mac is byte-for-byte structurally
identical to windows on every non-identical bundle (same raw lengths, same
size-delta sign).

## How to read the non-identical remainder

Most non-identical bundles are **value noise, not structure**: the object
set, field layout, and decompressed length all match; only byte values
inside same-sized fields differ (BC7 texels, preload pointer order), and
the LZ4 recompression of those different bytes then lands at a slightly
different on-disk size. Never judge a size delta from the compressed file —
decompress both sides first:

```bash
cargo build --release --example rawcmp
./target/release/examples/rawcmp /tmp/abgen-val300 \
    ../unity-reference-ab/ad0564d-val300-windows
```

`rawcmp` splits pairs into *structural* (different raw length) vs
*value-noise* (equal raw length, different bytes). A measurement on
2026-06-09 found 99.5% of non-identical windows bundles were value-only;
re-run `rawcmp` for the current split.

## The proven walls

These are the divergences we have root-caused and concluded cannot be
closed from the content alone. Don't re-drill them without new evidence.

### 1. AssetBundle `m_PreloadTable` ordering — SOLVED (was a wall, no longer)

**This is no longer a wall.** It was, until `d2b216e` derived the rule:
each container entry's preload run is its dependency set sorted by
`(CAB-name-lowercase, signed pathID)` ascending — internal objects sort
under the bundle's own `cab-<md5>` name, externals under their dependency's
CAB name. Validated 1467/1467 runs across 600 bundles
(`docs/bundle-format/preload_cab_merge_order.md`). The old "build-time instance-id /
session-state coin-flip" framing was wrong: the ExternalsPosition
heuristics were merely sampling fixed points of this name-dependent rule.

**Cross-platform re-verdict (2026-06-11):** the *same input does* order
differently between the windows and mac builds — but that is now fully
*explained and reproduced*, not a coin-flip. The CAB md5 is computed over a
platform-tagged string, so a dependency's `cab-<md5>` name differs between
windows and mac; the `(CAB-name-lowercase, …)` sort key therefore yields a
legitimately different lexicographic order per platform. abgen reproduces
the mac ordering exactly: on a glb-scene with two externally-keyed preload
entries, abgen's mac output preload table is byte-identical to the mac
reference — *including* the entry order that differs from the windows
reference. Probe it yourself:

```bash
./target/release/examples/preload_probe <abgen-mac-out>/<ent>/<bn>_mac
./target/release/examples/preload_probe <mac-ref>/<ent>/<bn>_mac    # identical
```

The remaining on-disk size differences on preload-bearing bundles are pure
BC7 texel value-noise recompressing to a slightly different length — present
identically on both platforms.

### 2. AnimatorController sub-asset index (emotes)

Emote bundles carry Mecanim AnimationClips as sub-assets of an
AnimatorController. The clip's PathID derives from a formula we recovered
(`prefab_packed(asset_guid("{hash}/animatorController"), md5("{hash}/animatorController/{idx}"))`)
— but `idx` is the clip's rank in the controller's serialized sub-asset
order, and the converter orders those by ascending session-PRNG
`localIdentifierInFile`. Sixteen structurally identical single-clip emotes
in the corpus land their clip on six different indices; no content formula
fits. The fork's determinism patch fixes the fileID *values* given an
order, but not the *order* — its own probe shows re-runs disagree on every
multi-sub-asset controller. Closing this needs an **upstream converter
change** (sort sub-assets deterministically before the rewrite), not an
abgen change. Probe tools: `examples/clipidxprobe`, `examples/dump_clips`;
write-up: `docs/animation/emote_animclip_pathid.md`.

### 3. BC7 within-mode float residual

BC7 *mode selection* now matches the upstream `bc7e` encoder's plain
comparison (this is the default path). What remains is endpoint noise
within a chosen mode: abgen's solver and Intel's ISPC build of the same
algorithm reduce floats in different orders, so a fraction of blocks land
on neighboring endpoint values — same algorithm, different last-bit
rounding. This is the dominant value-noise source on texture-bearing
bundles. Chasing it means reproducing ISPC's exact lane order and FMA
contraction, with steeply diminishing returns.

### 4. The fork's headless-batchmode texture stubs

This wall is in the *reference*, not in abgen. Headless batchmode Unity
collapses certain oversized textures to a flat mean-color block, and the
reference corpus bakes that artifact in — decoded reference textures
verify as flat (stddev ≈ 0). abgen reproduces the stub by default because
that's what byte-parity demands; `--real-textures` encodes the real image
instead and deliberately gives up parity on those bundles. The deeper
point: the parity oracle certifies "matches the fork in batchmode", which
is not the same thing as "matches production v38". Production-shape checks
go through `--v38-compat` + `examples/dump_census` against the production
mirror, and visually through ab-render-harness — not through this
scoreboard. Write-up: `docs/textures/standalone_texture_validation_regression.md`.

### Smaller proven residue

- **skin+animation PathID relabel** — glbs with both a skin and an
  animation get bone GameObject/Transform PathIDs from a non-deterministic
  `AddObjectToAsset` path; objects byte-identical, ids differ, references
  cascade (`docs/walls/skeleton_bone_pathid_relabel.md`).
- **Mecanim native residue** — the converter's native muscle-clip builder
  (`Internal_BuildClipMuscleConstant`) leaves sub-ULP coefficient
  differences; opaque to black-box probing (`docs/animation/m_muscle_clip_impl.md`).
- **Transform rotation signed-zero** — a few `-0.0` lanes on
  orientation-root nodes, a SIMD artifact with no content predictor
  (`docs/mesh/transform_signed_zero.md`).

## Discipline

Clean-room only. Allowed: black-box observation of reference bytes,
genuinely permissive open source (bc7e/ISPC under MIT, GLM, glTFast,
draco), and public math/specs. **Forbidden:** decompiling Unity binaries,
reading UnityCsReference, and reading Unity Companion License sources —
including the Scriptable Build Pipeline, `Unity.Mathematics`, and other
`com.unity.*` packages. No per-CID lookup tables; every rule must be
corpus-verified, not fitted to one example.
