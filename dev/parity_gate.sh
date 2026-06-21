#!/usr/bin/env bash
# Per-category parity regression gate for abgen.
#
# Runs `abgen-verify --json` on an output dir vs a reference dir, then compares
# every per-kind byte-identical count against the checked-in floors in
# dev/parity_floors.json. Exits non-zero (3) if ANY category (or the total)
# dropped below its floor.
#
# Usage:
#   dev/parity_gate.sh <set-name> [out-dir] [ref-dir]
#   dev/parity_gate.sh <set-name> [out-dir] [ref-dir] --bless
#
#   <set-name>  a key under "sets" in dev/parity_floors.json
#               (val300-windows | val600-scenes-windows | val600-we-windows)
#   [out-dir]   abgen output dir to verify    (default: the set's output_dir)
#   [ref-dir]   Unity reference dir           (default: the set's reference_dir)
#   --bless     re-write floors for this set from the fresh verify report
#               (use after a legitimate parity improvement; commit the result)
#
# Env:
#   ABGEN_VERIFY  path to the abgen-verify binary
#                 (default: <repo>/target/release/abgen-verify, then the main
#                  tree's target/release/abgen-verify as a fallback)
#
# NOTE: this gate does NOT build corpora. It verifies whatever output dir you
# point it at against the reference. The floors were blessed against the
# deterministic fork-parity recipe (--platform windows, no V38 / real-texture
# flags); verifying output built with other flags will not match the floors.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
FLOORS="$HERE/parity_floors.json"
COMPARE="$HERE/parity_compare.py"

if [[ $# -lt 1 ]]; then
  sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
fi

SETNAME="$1"; shift
BLESS=""
OUT=""
REF=""
for arg in "$@"; do
  case "$arg" in
    --bless) BLESS="--bless" ;;
    *) if [[ -z "$OUT" ]]; then OUT="$arg"; elif [[ -z "$REF" ]]; then REF="$arg"; fi ;;
  esac
done

# Resolve the abgen-verify binary.
if [[ -n "${ABGEN_VERIFY:-}" ]]; then
  VERIFY_BIN="$ABGEN_VERIFY"
elif [[ -x "$REPO/target/release/abgen-verify" ]]; then
  VERIFY_BIN="$REPO/target/release/abgen-verify"
else
  echo "error: abgen-verify not found; set ABGEN_VERIFY=<path>" >&2
  exit 2
fi

# Pull defaults for out/ref from the floors file for this set.
read_set_field() {
  python3 - "$FLOORS" "$SETNAME" "$1" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1]))
s = doc.get("sets", {}).get(sys.argv[2])
if s is None:
    sys.exit(0)
print(s.get(sys.argv[3], ""))
PY
}

[[ -z "$OUT" ]] && OUT="$(read_set_field output_dir)"
[[ -z "$REF" ]] && REF="$(read_set_field reference_dir)"

if [[ -z "$OUT" || -z "$REF" ]]; then
  echo "error: could not resolve out/ref for set '$SETNAME' (not in $FLOORS?)." >&2
  echo "       pass them explicitly: dev/parity_gate.sh $SETNAME <out> <ref>" >&2
  exit 2
fi
if [[ ! -d "$OUT" ]]; then echo "error: out dir missing: $OUT" >&2; exit 2; fi
if [[ ! -d "$REF" ]]; then echo "error: ref dir missing: $REF" >&2; exit 2; fi

JSON="$(mktemp /tmp/parity-gate-verify.XXXXXX.json)"
trap 'rm -f "$JSON"' EXIT

echo ">> abgen-verify $OUT $REF --json"
"$VERIFY_BIN" "$OUT" "$REF" --json "$JSON" >/dev/null

python3 "$COMPARE" --verify "$JSON" --floors "$FLOORS" --set "$SETNAME" $BLESS
