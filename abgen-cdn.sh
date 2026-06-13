#!/usr/bin/env bash
# abgen-cdn — build a 100%-abgen asset-bundle collection (no editor) and serve it
# to unity-explorer. This is the quick standalone path: one wearables
# collection, built and served from this machine in one command.
#
# Usage:
#   abgen-cdn.sh <collection-urn> [platform] [port]
#       e.g. abgen-cdn.sh urn:decentraland:off-chain:base-avatars windows 5185
#       Resolves the wearables collection via the local lambdas (a local catalyst lambdas service,
#       override with ABGEN_LAMBDAS_URL) and builds every wearable's bundles
#       straight from the catalyst content store (abgen-corpus
#       --collection-urn → flat <hash>_<platform> output). Fully standalone —
#       no editor, no reference corpus.
#
#   abgen-cdn.sh --serve-only <dir> [port]      # serve an already-built abgen dir
#
# Point the client with:
#   --lsd-use-remote-ab --lsd-remote-ab-server http://127.0.0.1:<port>
#
# Scope notes:
#   * The build runs with abgen defaults (byte-faithful to the reference
#     fork). Oversized textures then ship as flat-color stubs and render
#     gray — export ABGEN_REAL_TEXTURES=1 (propagates through dcl-shell)
#     before running if you want real artwork. See README.md "Build modes".
#   * Serving is delegated to abgen-serve.py (same dir), which handles both
#     the flat collection layout this script produces and nested
#     <entity>/<bundle> trees.
#   * For a full CDN (every active entity, per-entity manifests) build with
#     `abgen-corpus --entity-ids ... --cdn-layout --real-textures
#     --v38-compat` instead — that output shape is what an ab-cdn server
#     (umbrella's production-ish AB-CDN service) serves; abgen-serve.py
#     remains the zero-infrastructure alternative. See README.md
#     ("Workflows") for both paths.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DCLSHELL="/home/dcl/linux-rigging/dcl-shell"
CONTENT_ROOT="/home/dcl/umbrella/data/content_server/contents"
LAMBDAS="${ABGEN_LAMBDAS_URL:-http://localhost:5141/lambdas}"

if [ "${1:-}" = "--serve-only" ]; then
  exec python3 "$HERE/abgen-serve.py" "$2" --port "${3:-5185}"
fi

URN="${1:?usage: abgen-cdn.sh <collection-urn> [platform] [port]   (or --serve-only <dir> [port])}"
PLATFORM="${2:-windows}"
PORT="${3:-5185}"
OUT="$HERE/cdn-serve/$(echo "$URN" | tr ':/' '__')-$PLATFORM"

echo ">> building abgen-rs" >&2
"$DCLSHELL" -c "cd '$HERE' && cargo build --release --bins" >&2

echo ">> generating collection '$URN' ($PLATFORM) via lambdas $LAMBDAS -> $OUT" >&2
mkdir -p "$OUT"
"$DCLSHELL" -c "
  cd '$HERE'
  export ABGEN_CONTENT_ROOT='$CONTENT_ROOT'
  ./target/release/abgen-corpus --collection-urn '$URN' '$OUT' --platform '$PLATFORM' --lambdas-url '$LAMBDAS' -j 80
" >&2

echo ">> serving $OUT on :$PORT" >&2
exec python3 "$HERE/abgen-serve.py" "$OUT" --port "$PORT"
