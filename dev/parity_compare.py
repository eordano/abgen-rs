#!/usr/bin/env python3
"""Per-category parity regression comparator for abgen.

Compares the `per_kind[*].byte_identical` counts from an `abgen-verify --json`
report against the checked-in floors in dev/parity_floors.json, for one named
set (e.g. val300-windows).

Anti-masking property: every category is floored independently, so a regression
in one kind (e.g. glb-emote dropping) is caught even if another kind gains
enough to keep the TOTAL flat. The total is floored too, as a backstop.

Exit codes:
  0  all categories (and total) at or above floor  -> PASS
  3  at least one category (or total) below floor  -> REGRESSION
  2  usage / data error (missing set, unreadable json, etc.)

The 10 stable kind_of labels emitted by abgen-verify:
  bundle-empty, standalone-texture, standalone-texture-legacy, glb-emote,
  glb-wearable, glb-animated, glb-scene-empty, glb-scene-collider, glb-scene,
  other.
(glb-with-morph appears in the abgen-verify usage string but is never emitted
by kind_of -- a dead label; it is intentionally not floored.)

Usage:
  parity_compare.py --verify <verify.json> --floors <floors.json> --set <name>
  parity_compare.py --verify <verify.json> --floors <floors.json> --set <name> --bless
"""
import argparse
import json
import sys


def load_json(path):
    try:
        with open(path) as f:
            return json.load(f)
    except FileNotFoundError:
        print(f"error: file not found: {path}", file=sys.stderr)
        sys.exit(2)
    except json.JSONDecodeError as e:
        print(f"error: invalid JSON in {path}: {e}", file=sys.stderr)
        sys.exit(2)


def per_kind_byteid(verify):
    """Map label -> byte_identical from an abgen-verify --json document.

    Labels absent from the report (a kind that produced zero bundles) default
    to 0 so the comparator treats them as 'no byte-identical bundles', never
    as 'pass by omission'.
    """
    out = {}
    for label, stats in verify.get("per_kind", {}).items():
        out[label] = stats.get("byte_identical", 0)
    return out


def main():
    ap = argparse.ArgumentParser(description="abgen per-category parity gate")
    ap.add_argument("--verify", required=True, help="abgen-verify --json output")
    ap.add_argument("--floors", required=True, help="dev/parity_floors.json")
    ap.add_argument("--set", required=True, dest="setname",
                    help="named set in floors (e.g. val300-windows)")
    ap.add_argument("--bless", action="store_true",
                    help="rewrite floors for this set from the verify report")
    args = ap.parse_args()

    floors_doc = load_json(args.floors)
    verify = load_json(args.verify)

    sets = floors_doc.get("sets", {})
    if args.setname not in sets:
        print(f"error: set '{args.setname}' not in {args.floors}. "
              f"known sets: {', '.join(sorted(sets)) or '(none)'}",
              file=sys.stderr)
        sys.exit(2)

    setcfg = sets[args.setname]
    tolerance = floors_doc.get("tolerance", 0)
    got = per_kind_byteid(verify)
    got_total = verify.get("total", {}).get("byte_identical", 0)

    # --- bless mode: overwrite floors for this set from the report ---
    if args.bless:
        setcfg["floors"] = dict(sorted(got.items()))
        setcfg["total_floor"] = got_total
        if "bundles" in verify.get("total", {}):
            setcfg["reference_bundles"] = verify["total"]["bundles"]
        with open(args.floors, "w") as f:
            json.dump(floors_doc, f, indent=2)
            f.write("\n")
        print(f"blessed set '{args.setname}': total_floor={got_total}")
        for label in sorted(got):
            print(f"  {label:<28} {got[label]}")
        sys.exit(0)

    floors = setcfg.get("floors", {})
    total_floor = setcfg.get("total_floor", 0)

    # Union of floored labels and observed labels: a label that appears in the
    # report but is not floored is checked against an implicit floor of 0 (so a
    # brand-new category can never silently appear unfloored and unchecked).
    all_labels = sorted(set(floors) | set(got))

    print(f"parity gate :: set={args.setname} "
          f"reference={setcfg.get('reference', '?')} "
          f"tolerance={tolerance}")
    print(f"{'category':<28} {'got':>7} {'floor':>7} {'delta':>7}  status")
    print("-" * 64)

    violations = []
    rises = []
    for label in all_labels:
        g = got.get(label, 0)
        fl = floors.get(label, 0)
        delta = g - fl
        if g < fl - tolerance:
            status = "FAIL"
            violations.append((label, g, fl, delta))
        elif delta > 0:
            status = "rose"
            rises.append((label, g, fl, delta))
        else:
            status = "ok"
        print(f"{label:<28} {g:>7} {fl:>7} {delta:>+7}  {status}")

    print("-" * 64)
    t_delta = got_total - total_floor
    t_status = "FAIL" if got_total < total_floor - tolerance else (
        "rose" if t_delta > 0 else "ok")
    print(f"{'TOTAL':<28} {got_total:>7} {total_floor:>7} {t_delta:>+7}  {t_status}")
    if got_total < total_floor - tolerance:
        violations.append(("TOTAL", got_total, total_floor, t_delta))
    elif t_delta > 0:
        rises.append(("TOTAL", got_total, total_floor, t_delta))

    print()
    if rises:
        print("note: categories rose above floor (candidate floor bumps -- "
              "re-bless to ratchet):")
        for label, g, fl, delta in rises:
            print(f"  {label} {fl} -> {g} (+{delta})")
        print()

    if violations:
        print(f"REGRESSION: {len(violations)} category(ies) below floor:")
        for label, g, fl, delta in violations:
            print(f"  {label}: got {g}, floor {fl} ({delta})")
        print("\nFAIL")
        sys.exit(3)

    print("PASS")
    sys.exit(0)


if __name__ == "__main__":
    main()
