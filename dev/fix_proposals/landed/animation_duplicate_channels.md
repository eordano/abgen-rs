# Legacy AnimationClip — dedup duplicate (node, path) channels

> **Status: landed.** Verified net improvement, no regression, parity gate green.

## Area context (negative finding on the headline target + one real fix)

The session brief targeted `glb-animated` Mecanim curve parity (129 bundles,
59 byte-id, 422257 ppm). Investigation showed the `glb-animated` residual is
**not** animation-curve work:

A typetree-level diff of every AnimationClip (class 74) in the windows
test-set found **978 clip pairs, only 1 with any content diff**. The legacy
`Animation`+`AnimationClip` content is otherwise bit-exact (the closure claimed
in `animationclip_content.md` holds corpus-wide).

Decompressed-byte attribution of the 70 non-identical `glb-animated` bundles:

| residual | diff bytes (decompressed) | character |
|---|---:|---|
| resS (texture stream) | 224,454,076 (39 bundles) | BC7 wall |
| SF (serialized file)  | 14,486,550 (31 bundles)  | mostly Mesh |

Name-paired leaf-value diffs by class on `glb-animated`:

| class | leaf diffs |
|---|---:|
| Mesh (43) | 1,328,214 |
| AnimationClip (74) | 8,927 |
| AssetBundle (142, preload order) | 5,682 |
| Transform (4) | 1,168 |
| everything else | < 200 each |

So the `glb-animated` ppm is dominated by **Mesh vertex/normal encoding**
(Area 4/10) and **BC7 textures** (the texel wall), with the AssetBundle
preload-table ordering wall a distant third. AnimationClip is a rounding-error
contributor — and almost all of its 8,927 came from the single bundle below.

## The one real animation bug

`bafkreiexf46ljdv…/bafkreigm5wbcn3…` (`LightsShip_Action`, a glb-wearable
bundle) had the only divergent AnimationClip. Its source glTF contains
**duplicate channels**: node 0's `translation`/`rotation`/`scale` each appear
**5×** (27 channels total). Our legacy emitter
(`animation.rs::build_animation_clips_from_gltf`) looped over every channel and
pushed one curve per channel, so it emitted 9 rotation curves (5 redundant
`LightsShip` root entries interleaved) where Unity emits 5.

Unity's glTF importer keeps a single curve per `(node, target.path)` — the
first occurrence; later duplicate channels are dropped. The Mecanim path
(`animation_mecanim.rs::gather_clip_curves`) already did this dedup; the legacy
path did not.

```
ours (before): m_RotationCurves count=9  (LightsShip×5 keys=1 + 4 bones)
ref:           m_RotationCurves count=5  (LightsShip×1 + 4 bones)
```

## Fix

`src/animation.rs::build_animation_clips_from_gltf`: before processing each
channel, skip it if `(node, target.path)` was already seen in this animation
(a `HashSet` guard). One occurrence kept, in channel order — matching Unity and
the existing Mecanim dedup.

## Corpus scope

A scan of all 978 emitted clips found exactly **1 bundle** with duplicate
same-path curves (12 redundant curves = 4 extra × 3 attrs). It is a rare input
shape, but the fix is unambiguously correct (Unity never emits the duplicates)
and only changes output when duplicate channels are present, so it cannot
regress the dedup-free majority.

## Measured impact (windows test-set, 4243 bundles)

| | diff-bits | ppm |
|---|---:|---:|
| glb-wearable before | 864,366,483 | 321,732.7 |
| glb-wearable after  | 863,806,042 | 321,524.1 |
| **glb-wearable Δ**  | **−560,441** | **−208.6** |
| TOTAL before | 4,013,834,953 | 384,481.3 |
| TOTAL after  | 4,013,274,512 | 384,427.6 |
| **TOTAL Δ**  | **−560,441** | **−53.7** |

`glb-animated` row is unchanged (the affected bundle is classified
glb-wearable). No other kind regressed.

## Test bars

- `cargo test --release --test parity_bytes`: 2 passed (gate green).
- `cargo test --release --lib`: 129 passed; the only 2 failures are the
  pre-existing `bc7_pure::tests::bit_exact_*` (missing probe vectors) which fail
  identically on the baseline checkout — unrelated to this change.

## Why the headline Mecanim target was not pursued further

- Mesh and BC7 own the `glb-animated` residual; both are separate, large,
  already-scoped areas (`RESEARCH_AREAS.md` #4/#10 and the BC7 wall).
- The Mecanim `a/b` coefficient residual (`constant_curve_split.md`) and the
  streamed/constant split remain blocked on Unity's internal classifier +
  `m_MuscleClipSize` derivation — no new clean-room lead surfaced this session.
