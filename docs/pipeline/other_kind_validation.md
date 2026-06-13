# Validation of the "other" kind finding on the larger corpus

**Why it matters:** the conclusion that the `other` bundle kind is entirely
legacy CIDv0 standalone textures (see `other_kind_drill.md`) was first drawn on
a small test corpus. Before relying on it for attribution and prioritization, it
had to be confirmed that the finding scales and holds no surprises on a much
larger validation corpus.

**How it works:** across the validation corpus every `other` bundle again has the
single class signature of one Texture2D plus one AssetBundle and nothing else —
no second signature appears. The same structural distribution holds: the same
split between the two object orderings, the same fraction of `.resS`-streamed
bundles, and the same net profile where abgen-rs is overall byte-smaller than the
reference. The bit-level divergence is concentrated in the `.resS`-streamed
cohort (a minority of bundles carrying the majority of the diff, all of it
compression-envelope drift) with the remainder in inline per-block BC7 mismatch.
Byte-identical bundles all sit in the inline, no-`.resS`, Texture2D-first cohort
that the BC7 long-tail work is actively closing.

This is a confirmation, not a new mechanism. The recommended action remains the
classifier relabel to `standalone-texture-legacy`, which recovers no bytes and
only clarifies attribution; the real recovery is owned by the existing LZ4HC and
BC7 probes. The two cohorts (legacy CIDv0 and modern bafkrei standalone textures)
are kept as separate kinds rather than merged, because their byte/bit profiles
differ and merging would hide that signal.
