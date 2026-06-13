# Clean-room charter

abgen-rs is an independent, clean-room reimplementation of Decentraland's
own [`asset-bundle-converter`](https://github.com/decentraland/asset-bundle-converter).
This document records how it is built and the rules every contributor
follows, so the project's provenance is clear and auditable.

## What we are reimplementing, and why

Decentraland's clients load AssetBundles — the UnityFS container format —
produced by Decentraland's own asset-bundle-converter, which drives a
headless editor once per entity. abgen-rs reimplements that converter
from scratch in Rust. The goal is file-format interoperability: emitting
the same AssetBundle format Decentraland's clients already consume,
directly from the glTF and image source on the catalyst content network,
without the cost and operational burden of running the converter service.

## What we observe, and why we may

The only artifacts abgen-rs studies are **our own output**: AssetBundles
produced by Decentraland's converter from Decentraland's own assets, and
the bundles Decentraland already serves publicly from its production
asset-bundle CDN. Both are things we are entitled to observe:

- The bundles are the converted form of Decentraland's own glTF, PNG, and
  JPG assets. Under the upstream editor's own Terms of Service, the
  results generated through use of the editor — its "Project" output —
  belong to the party that created them. Matching the byte format of our
  own Project output is studying our own files, not inspecting the tool.
- The production-CDN bundles are served publicly to every client that
  loads them.

The AssetBundle/UnityFS container is a functional file format, openly
parsed by every client that loads it and by independent open-source
tooling (for example AssetStudio, UnityPy, AssetRipper). It is a format
we interoperate with, not a secret we extracted.

## Method

Every rule in abgen-rs is recovered the same way:

1. **Observe** the bytes of reference output — our own Project output, or
   public production-CDN bundles.
2. **Hypothesize** the algorithm that would produce them.
3. **Reimplement independently** in Rust, from that hypothesis and from
   public specifications.
4. **Verify** by rebuilding a large, varied corpus and diffing for
   byte-parity. A rule is accepted only when it holds across the corpus —
   never fitted to a single example.

No per-CID lookup tables, and no hard-coded constants taken from any
third-party source. Trained or statistical artifacts — such as the BC7
mode-prediction tree — are derived solely from our own observed reference
output, never from any third-party internals.

## Prohibited — never, by anyone, in this project

- Decompiling, disassembling, or otherwise reverse-engineering any
  third-party binary.
- Consulting `UnityCsReference` or any other published reference source of
  the upstream editor.
- Using Unity Companion License material — the Scriptable Build Pipeline,
  `Unity.Mathematics`, or other `com.unity.*` packages (source-available
  but restricted).
- Consulting leaked, confidential, or NDA-covered material of any third
  party.
- Copying proprietary source code of any kind.

## Allowed sources

- Black-box observation of our own Project output and of publicly-served
  production-CDN bundles.
- Genuinely permissive open source (MIT / BSD / Apache / public-domain) —
  for example bc7e, GLM, glTFast, google/draco, BinomialLLC/crunch, lz4,
  and xxHash. Every vendored or ported component is listed with its
  license and upstream in [`NOTICES.md`](NOTICES.md).
- Public standards and specifications — BC7/BPTC, the PNG and JPEG/JFIF
  formats, glTF, and the published RFCs for the hash functions used.

## Provenance separation

The one step that runs the upstream converter — and therefore the
upstream editor — is generating the reference corpora. That step is
performed by an individual contributor under that contributor's own
commercial editor license, independently of the Decentraland Foundation.
Only the resulting output artifacts inform abgen-rs; no editor internals
cross into the implementation.

## For contributors

If you contribute to abgen-rs, you confirm that your contribution follows
this charter: it is your own independent work or permissively-licensed
material, recovered only from the allowed sources above, with none of the
prohibited sources consulted. If you are unsure whether a source is
allowed, ask before using it.

## Scope

This document records the project's methodology and discipline; it is not
legal advice. The project's license terms are in [`LICENSE`](LICENSE);
third-party attributions are in [`NOTICES.md`](NOTICES.md); the catalog of
recovered rules is in [`docs/`](docs/README.md).
