# dev/ — internal working notes

Machine-specific session logs, parity-status snapshots, and fix-proposal
history. Not part of the guide: paths, scores, and setup details in here
are tied to the development environment and go stale. The curated,
machine-independent documentation is the main `README.md` and `docs/`.

## check_determinism.sh — self-determinism gate

`check_determinism.sh` builds a small pinned entity set (4 scene/textured
incl. a 178-file BC7-heavy scene + 1 emote) three times — `-j8`, `-j8`, `-j1`
— and asserts every output bundle is byte-identical (sha256) across all three.
This catches abgen's *own* nondeterminism (rayon races, HashMap iteration
order, allocator first-touch, embedded timestamps), independent of any Unity
reference. Run it after a build, the same way you run the parity scoreboard.

Must run inside the FHS env so the 64-bit libturbojpeg is used (running bare
silently degrades JPEG — the 2652→2353 footgun):

```sh
<fhs-shell> -c 'bash dev/check_determinism.sh'
```

Exits 0 on identity, nonzero (and prints the diff) if any bundle differs or a
file is missing/extra. Entity IDs are pinned in the script so its corpus can't
silently drift; a dropped reference entity fails loudly. Knobs via env:
`ABGEN_BIN`, `ABGEN_CONTENT_ROOT`, `ABGEN_REF_SCENE`, `ABGEN_REF_WE`,
`ABGEN_JOBS`. Baseline: 189 bundles identical across both axes (HEAD 8e6333d).
