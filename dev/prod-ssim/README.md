# prod-ssim — categorized prod-comparison harness

A reliability gate that compares abgen output against the **real production
AB-CDN mirror** (`/path/to/asset-bundles`) per entity and
**per category** (scene / wearable / emote, read from `entity.json` `type`).

The point of this harness is that it **does not reduce to a single SSIM
number**. SSIM 0.9999 — or even 1.0 — can hide an encoding, ordering, PathID,
or material-binding error. So every bundle is scored on three independent axes
and the verdict is a multi-bucket cell, not one float.

## The three axes

1. **byte-identity** — is abgen's bundle byte-for-byte equal to prod?
2. **structural** — `examples/classify_pair` decomposes each non-identical
   bundle into a CAT bucket (below). This is the axis that catches object-set,
   PathID and preload-ordering differences that pixels cannot see.
3. **visual SSIM** — `ab-render-harness` decodes every `Texture2D` (Unity-free)
   and scores per-texture SSIM vs prod. Per-bundle score = worst (min) texture.

## Bucket taxonomy

Structural bucket (from `classify_pair` CAT 1–9):

| bucket | CATs | meaning |
|---|---|---|
| `byte-identical` | 1 | bytes equal |
| `ordering/id-only` | 2, 5 | only PathIDs / preload ordering differ |
| `value-noise` | 3, 4, 6 | texel / float noise, no structural change |
| `STRUCTURAL` | 7 | object-set / size / extra-or-missing objects |
| `STRUCTURAL-tex-far` | 8 | structural **and** pixel-far |
| `BUNDLE-only-abgen` / `BUNDLE-only-prod` | — | a whole bundle exists on only one side |

Visual band (from per-texture SSIM, relative to the category floor):
`visual-identical` (≥0.99999) · `visual-ok` (≥floor) · `visual-degraded`
(≥0.85) · `visual-broken` (<0.85, the ~0.71 stub-vs-real signature) ·
`no-texture`.

## How categorization prevents SSIM from masking errors

The crossed cell **`high_ssim_structural_diff`** (`masked` in the scoreboard)
flags any bundle that **passes the SSIM floor yet is structurally different
from prod**. A pure-SSIM gate would pass these silently; here they are counted
and listed. Example from the probe run: a scene bundle with `SSIM = 1.000000`
(pixel-identical) but `CAT7 STRUCTURAL` (`sizemis=1 ids_changed=true`) — a real
PathID + serialized-size difference that SSIM alone calls perfect.

## Per-category SSIM floors

In `prod_ssim.py` (`CATEGORY_FLOOR`). Ratchet **up** only, never down without a
documented reason. The visual floor gates the *visual* axis; the structural
axis has no floor — any structural diff is reported regardless of SSIM.

## Cross-version comparison

The prod mirror is mixed-version (v15…v48). Texture artwork is version-stable,
so cross-version **visual** SSIM is valid; the **structural** axis will
legitimately light up `CAT7` across converter versions (preload ordering,
Texture2D dual-emit, PathID schemes). The harness **records the prod version
per entity** and rolls it into the per-category table so a version-correlated
result is interpretable, not silently averaged away.

## Run

```bash
cd <repo>
python3 dev/prod-ssim/prod_ssim.py \
  --entities-file dev/prod-ssim/probe-entities.txt \
  --out /tmp/agent-06/prod-ssim-report --platform windows
```

Each CID must exist in BOTH `content_rust` and the prod mirror (the harness
skips ones that don't). abgen is built with the prod-style recipe
(`--cdn-layout --real-textures --v38-compat`, `ABGEN_V38_TIMESTAMP` pinned,
`ABGEN_ROOT` pointed at the template dir) inside an FHS shell for the 64-bit
libturbojpeg. Use `--abgen-out DIR` to reuse a prior build and skip stage B.

Outputs `report.json` (structured) and `report.md` (readable scoreboard).
A non-zero `masked` total is a hard finding — visual score would have hidden it.

## Dependencies (all prebuilt in the worktree / harness dir)

`target/release/abgen-corpus`, `target/release/examples/classify_pair`,
`target/release/examples/dump_tex_png`, the `ab-render-harness` script, and
imagemagick (inside an FHS shell).
