#!/usr/bin/env bash
# dev/check_determinism.sh — abgen self-determinism gate.
#
# Builds a small, PINNED entity set three times in fork-reference mode:
#   build a:  -j 8   (parallel)
#   build b:  -j 8   (parallel, re-run)        — catches run-to-run nondeterminism
#   build c:  -j 1   (serial)                  — catches parallelism-dependent output
# then sha256-compares every output bundle across all three. Exits nonzero if ANY
# file differs (content, or a missing/extra path). This locks in abgen's OWN
# determinism — independent of any Unity reference — so a future change that
# introduces a HashMap-ordered emit, a wall-clock field, or a racy rayon reduce
# fails loudly instead of silently corrupting reproducibility.
#
# MUST be run inside an FHS env so the 64-bit libturbojpeg is used:
#   <fhs-shell> -c 'bash dev/check_determinism.sh'
# Running it bare silently degrades JPEG decode (the documented 2652->2353
# footgun) — the gate would still be self-consistent, but on a wrong code path.
#
# Overridable via env: ABGEN_BIN, ABGEN_CONTENT_ROOT, ABGEN_REF_SCENE,
# ABGEN_REF_WE, ABGEN_JOBS (default 8).
set -euo pipefail

# --- config ---------------------------------------------------------------
# Default to the MAIN tree's prebuilt binary (script-only feature; no build needed).
BIN=${ABGEN_BIN:-./target/release/abgen-corpus}
export ABGEN_CONTENT_ROOT=${ABGEN_CONTENT_ROOT:-/path/to/content/contents}
# Reference trees the pinned entity dirs are sourced from. from-reference does
# NOT follow symlinks, so we copy (deref) the dirs into one mini-ref.
REF_SCENE=${ABGEN_REF_SCENE:-/path/to/abc-abgenrs-799967c3-2026-06-20/val300-windows}
REF_WE=${ABGEN_REF_WE:-/path/to/abc-abgenrs-799967c3-2026-06-20/val600-we-windows}
JOBS=${ABGEN_JOBS:-8}

# Pinned entity IDs — NOT `ls | head`, so the gate's corpus can never silently
# drift when a reference dir changes. Mix: 3 small textured, 1 BC7-heavy
# multi-bundle scene (rayon/allocator race surface), 1 emote (multi-sub-asset
# AnimationClip controller path). If any ID is dropped from its reference the
# `cp` below fails loudly.
SCENE_IDS=(
  bafkreia25oumh7a7b32qz2ni653z4anwyrz4bnnc6lbko52gregnngoj3y
  bafkreia2cd5vjzv2ahz5lzmbav6rh6s3ru2r6ib6n7wnov65grntj72m7u   # 178-file, BC7-heavy
  bafkreia2sds5vrdlnab6vrdpqqq2ua5hrl62zh7quwih6orq3vnk5j55w4
  bafkreia44servarwqevuzt62d56myf5aj5d3fs3qj2eey7eozbphvcdw7q
)
EMOTE_IDS=(
  bafkreiaafmcdhsof5qyhvxjn7n22asckm2xuuy3ksg5wwfpjpizmft6ona   # emote
)

# --- preflight ------------------------------------------------------------
[ -x "$BIN" ] || { echo "FATAL: binary not found/executable: $BIN" >&2; exit 3; }
[ -d "$ABGEN_CONTENT_ROOT" ] || { echo "FATAL: content root not found: $ABGEN_CONTENT_ROOT" >&2; exit 3; }

WORK=$(mktemp -d /tmp/abgen-determinism.XXXXXX)
trap 'rm -rf "$WORK"' EXIT

# --- assemble pinned mini-reference ---------------------------------------
mkdir -p "$WORK/ref"
for id in "${SCENE_IDS[@]}"; do
  [ -d "$REF_SCENE/$id" ] || { echo "FATAL: pinned scene entity missing from $REF_SCENE: $id" >&2; exit 3; }
  cp -rL "$REF_SCENE/$id" "$WORK/ref/"
done
for id in "${EMOTE_IDS[@]}"; do
  [ -d "$REF_WE/$id" ] || { echo "FATAL: pinned emote entity missing from $REF_WE: $id" >&2; exit 3; }
  cp -rL "$REF_WE/$id" "$WORK/ref/"
done

# --- build + verify helpers ----------------------------------------------
build() {  # $1=outdir  $2=jobs
  local out=$1 j=$2
  if ! "$BIN" --from-reference "$WORK/ref" "$out" --platform windows -j "$j" \
        >"$WORK/log.$j.out" 2>&1; then
    echo "FATAL: build failed (-j $j):" >&2; cat "$WORK/log.$j.out" >&2; exit 2
  fi
  grep -q 'errs=0' "$WORK/log.$j.out" \
    || { echo "FATAL: nonzero errs (-j $j) — refusing to pass a degraded build" >&2; \
         tail -3 "$WORK/log.$j.out" >&2; exit 2; }
}

# sorted "sha256  ./relpath" manifest of every output file
sums() { ( cd "$1" && find . -type f -exec sha256sum {} + | sort -k2 ); }

# --- run ------------------------------------------------------------------
echo "abgen determinism gate"
echo "  binary : $BIN"
echo "  content: $ABGEN_CONTENT_ROOT"
echo "  corpus : ${#SCENE_IDS[@]} scene/texture + ${#EMOTE_IDS[@]} emote (pinned)"
echo "  builds : a=-j$JOBS  b=-j$JOBS  c=-j1"

build "$WORK/a" "$JOBS"
build "$WORK/b" "$JOBS"
build "$WORK/c" 1

N=$(sums "$WORK/a" | wc -l)
echo "  bundles: $N per build"

fail=0
if diff <(sums "$WORK/a") <(sums "$WORK/b") >"$WORK/diff.ab"; then
  echo "  PASS  re-build (-j$JOBS vs -j$JOBS): identical"
else
  echo "  FAIL  NONDETERMINISTIC across re-build (-j$JOBS):" >&2
  sed -n '1,40p' "$WORK/diff.ab" >&2
  fail=1
fi
if diff <(sums "$WORK/a") <(sums "$WORK/c") >"$WORK/diff.ac"; then
  echo "  PASS  parallelism (-j$JOBS vs -j1): identical"
else
  echo "  FAIL  NONDETERMINISTIC across thread count (-j$JOBS vs -j1):" >&2
  sed -n '1,40p' "$WORK/diff.ac" >&2
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "OK: determinism check passed ($N bundles byte-identical across 3 builds, 2 axes)"
else
  echo "DETERMINISM CHECK FAILED" >&2
fi
exit "$fail"
