#!/usr/bin/env bash
# dev/check_goldens.sh — verify (or re-bless) abgen's golden hash set.
#
# Pins abgen's OWN deterministic output (SHA256 per emitted bundle) for a small,
# curated set of entities — one representative per hard-won "wall" fix-family —
# so those fixes can never silently regress, even when the external val-300
# reference corpus is unavailable. This is a self-consistency / anti-silent-
# regression gate, NOT a Unity-parity assertion (see dev/goldens.json _purpose).
#
# Usage:
#   dev/check_goldens.sh            # regenerate + compare to dev/goldens.json; exit 1 on mismatch
#   dev/check_goldens.sh --update   # re-bless: build TWICE, assert reproducible, rewrite goldens.json
#   dev/check_goldens.sh --help
#
# Behaviour:
#   - SKIPS (exit 0, with a clear notice) when the local content store is absent,
#     so fresh checkouts / CI without the store stay green.
#   - Per-entity SKIP (does not fail) if an entity's input is missing from the store.
#   - Honours the libturbojpeg footgun: run inside an FHS env, or set TURBOJPEG_LIB,
#     or textured bundles silently change bytes (JPEG decode falls back to `image`).
#
# Recipe is fork-reference mode (NO --v38-compat / --real-textures) — the
# reproducible one. Do not change the recipe without re-blessing.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GOLDENS="$SCRIPT_DIR/goldens.json"

# Binary: prefer an explicit ABGEN_CORPUS_BIN, else this repo's release build.
BIN="${ABGEN_CORPUS_BIN:-}"
if [[ -z "$BIN" ]]; then
  for cand in \
    "$REPO_ROOT/target/release/abgen-corpus"; do
    if [[ -x "$cand" ]]; then BIN="$cand"; break; fi
  done
fi

CONTENT_ROOT="${ABGEN_CONTENT_ROOT:-/path/to/content/contents}"
PLATFORM="windows"   # goldens are pinned for windows (mac tracks windows bundle-for-bundle)
JOBS="${ABGEN_GOLDEN_JOBS:-8}"

MODE="check"
case "${1:-}" in
  --update) MODE="update" ;;
  --help|-h)
    sed -n '2,33p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit 0 ;;
  "" ) ;;
  * ) echo "unknown arg: $1 (try --help)" >&2; exit 2 ;;
esac

# ---- SKIP gates (keep fresh checkouts green) ----------------------------------
if [[ ! -f "$GOLDENS" ]]; then
  echo "SKIP check_goldens: $GOLDENS not found" >&2
  exit 0
fi
if [[ -z "$BIN" || ! -x "$BIN" ]]; then
  echo "SKIP check_goldens: abgen-corpus binary not found (set ABGEN_CORPUS_BIN, or build target/release/abgen-corpus)" >&2
  exit 0
fi
if [[ ! -d "$CONTENT_ROOT" ]]; then
  echo "SKIP check_goldens: content store absent at ABGEN_CONTENT_ROOT=$CONTENT_ROOT" >&2
  echo "  (this is expected on a fresh checkout / CI without the local store)" >&2
  exit 0
fi

# libturbojpeg footgun: warn if neither an FHS env nor TURBOJPEG_LIB looks set.
if [[ -z "${TURBOJPEG_LIB:-}" && -z "${IN_NIX_SHELL:-}" ]]; then
  echo "NOTE: TURBOJPEG_LIB not set and not in a nix shell — if textured bundles" >&2
  echo "  mismatch, re-run inside an FHS env or set TURBOJPEG_LIB (see goldens.json)." >&2
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/abgen-goldens.XXXXXX")"
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

# Entity CID list (only entities present in the local store; absent ones are
# skipped, not failed). Emitted to $WORK/cids.txt; skipped list to stderr.
PRESENT_CIDS="$WORK/cids.txt"
python3 - "$GOLDENS" "$CONTENT_ROOT" "$PRESENT_CIDS" <<'PY'
import json, sys, os, hashlib
goldens, root, out = sys.argv[1], sys.argv[2], sys.argv[3]
doc = json.load(open(goldens))
present, skipped = [], []
for e in doc["entities"]:
    cid = e["entity_id"]
    shard = hashlib.sha1(cid.encode()).hexdigest()[:4]
    if os.path.exists(os.path.join(root, shard, cid)):
        present.append(cid)
    else:
        skipped.append(cid)
with open(out, "w") as f:
    f.write("\n".join(present) + ("\n" if present else ""))
for c in skipped:
    print(f"SKIP {c}: not in local store", file=sys.stderr)
print(f"{len(present)} present / {len(skipped)} skipped", file=sys.stderr)
PY

if [[ ! -s "$PRESENT_CIDS" ]]; then
  echo "SKIP check_goldens: none of the pinned entities are in the local store" >&2
  exit 0
fi

build() {
  local out="$1" jobs="$2"
  ABGEN_CONTENT_ROOT="$CONTENT_ROOT" "$BIN" \
    --entity-ids "$PRESENT_CIDS" "$out" --platform "$PLATFORM" -j "$jobs" \
    >/dev/null 2>"$WORK/build.log" || {
      echo "ERROR: abgen-corpus failed:" >&2; tail -20 "$WORK/build.log" >&2; exit 3; }
}

GEN_A="$WORK/genA"
echo "Building ${PLATFORM} bundles for $(wc -l < "$PRESENT_CIDS") entities (-j $JOBS)..." >&2
build "$GEN_A" "$JOBS"

if [[ "$MODE" == "update" ]]; then
  # Reproducibility guard: build a SECOND time (different parallelism) and assert
  # byte-identical before writing new goldens — keeps the goldens honest.
  GEN_B="$WORK/genB"
  echo "Re-building (-j 1) to verify reproducibility before re-blessing..." >&2
  build "$GEN_B" 1
  if ! diff <(cd "$GEN_A" && find . -type f -print0 | sort -z | xargs -0 sha256sum) \
            <(cd "$GEN_B" && find . -type f -print0 | sort -z | xargs -0 sha256sum) >/dev/null; then
    echo "ABORT --update: nondeterminism detected (genA != genB) — refusing to write flaky goldens." >&2
    exit 4
  fi
  echo "Reproducible (genA == genB). Re-blessing $GOLDENS ..." >&2
  python3 - "$GOLDENS" "$GEN_A" <<'PY'
import json, sys, os, hashlib
goldens, gen = sys.argv[1], sys.argv[2]
doc = json.load(open(goldens))
def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()
total = 0
for e in doc["entities"]:
    d = os.path.join(gen, e["entity_id"])
    if not os.path.isdir(d):
        continue  # entity was skipped (absent from store); keep its old hashes
    bundles = {}
    for name in sorted(os.listdir(d)):
        p = os.path.join(d, name)
        if os.path.isfile(p):
            bundles[name] = sha256(p)
    if bundles:
        e["bundles"] = bundles
    total += len(bundles)
doc["_bundle_count"] = sum(len(e["bundles"]) for e in doc["entities"])
json.dump(doc, open(goldens, "w"), indent=2)
open(goldens, "a").write("\n")
print(f"re-blessed {total} bundles across rebuilt entities", file=sys.stderr)
PY
  echo "DONE --update. Review with: git diff dev/goldens.json" >&2
  exit 0
fi

# ---- check mode: compare emitted hashes to goldens ---------------------------
python3 - "$GOLDENS" "$GEN_A" "$PRESENT_CIDS" <<'PY'
import json, sys, os, hashlib
goldens, gen, cids_path = sys.argv[1], sys.argv[2], sys.argv[3]
doc = json.load(open(goldens))
present = set(l.strip() for l in open(cids_path) if l.strip())
def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()
mismatches, missing, checked = [], [], 0
for e in doc["entities"]:
    cid = e["entity_id"]
    if cid not in present:
        continue  # already reported SKIP above
    d = os.path.join(gen, cid)
    for name, want in e["bundles"].items():
        p = os.path.join(d, name)
        if not os.path.isfile(p):
            missing.append((cid, name))
            continue
        got = sha256(p)
        checked += 1
        if got != want:
            mismatches.append((cid, name, want, got, e.get("fix_families", [])))
print(f"checked {checked} bundle hashes across {len(present)} entities", file=sys.stderr)
if missing:
    print(f"ERROR: {len(missing)} pinned bundle(s) were not emitted (build shape changed):", file=sys.stderr)
    for cid, name in missing[:20]:
        print(f"  MISSING-OUTPUT {cid}/{name}", file=sys.stderr)
if mismatches:
    print(f"GOLDEN MISMATCH: {len(mismatches)} bundle(s) changed bytes — a fix changed output.", file=sys.stderr)
    print("  If intended, re-bless: dev/check_goldens.sh --update && git diff dev/goldens.json", file=sys.stderr)
    for cid, name, want, got, fams in mismatches[:20]:
        print(f"  {cid}/{name}", file=sys.stderr)
        print(f"    fix_families: {fams}", file=sys.stderr)
        print(f"    want {want}", file=sys.stderr)
        print(f"    got  {got}", file=sys.stderr)
if mismatches or missing:
    sys.exit(1)
print("GOLDENS OK: all pinned hashes match.", file=sys.stderr)
PY
