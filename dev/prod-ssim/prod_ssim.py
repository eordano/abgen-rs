#!/usr/bin/env python3
"""prod_ssim — categorized prod-comparison harness for abgen-rs.

Per entity AND per category (scene / wearable / emote, read from entity.json
"type"), produce a MULTI-BUCKET verdict against the real production AB-CDN
mirror — NOT a single SSIM number. A high per-texture SSIM can hide an
encoding / ordering / PathID / binding error; this harness keeps the visual
score and the *structural* verdict on separate axes so visual similarity can
never mask a structural defect.

Three independent signals are gathered for every entity:

  (a) byte-identity  — is abgen's bundle byte-for-byte equal to prod?
  (b) structural     — `examples/classify_pair` decomposes each non-identical
                       bundle into byte/id-ordering/value-noise/structural/
                       texture-far. This catches object-set, PathID and
                       preload-ordering differences that pixels cannot.
  (c) visual SSIM    — `ab-render-harness` decodes every Texture2D and scores
                       per-texture SSIM vs prod (Unity-free).

The two axes are crossed into a per-bundle cell. The cell we exist to surface
is HIGH-SSIM + STRUCTURAL-DIFF: a bundle that looks right pixel-for-pixel but
differs in object set / ids / ordering / binding.

Usage:
  prod_ssim.py --entities-file FILE  [--out DIR] [--platform windows]
               [--abgen-out DIR] [--keep-abgen]
  prod_ssim.py --entity CID [--entity CID ...] ...

entities-file: one CID per line ('#' comments ok). Each CID must exist in BOTH
content_rust and the prod mirror.

Outputs (under --out, default /tmp/agent-06/prod-ssim-report):
  report.json   — structured per-entity + per-category + per-bundle
  report.md     — readable categorized scoreboard table
"""
import argparse, hashlib, json, os, re, subprocess, sys, shutil, time

ABGEN_RS = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CONTENT_ROOT = os.environ.get(
    "ABGEN_CONTENT_ROOT", "/path/to/content/contents")
PROD = os.environ.get("ABGEN_PROD_AB", "/path/to/asset-bundles")
DCL_SHELL = os.environ.get("ABGEN_FHS_SHELL", "/path/to/fhs-shell")
HARNESS = os.environ.get("ABGEN_RENDER_HARNESS", "/path/to/ab-render-harness")
ABGEN_CORPUS = f"{ABGEN_RS}/target/release/abgen-corpus"
CLASSIFY_PAIR = f"{ABGEN_RS}/target/release/examples/classify_pair"
V38_TIMESTAMP = "639168155420400256"
TURBOJPEG_LIB = os.environ.get("TURBOJPEG_LIB", "/path/to/libturbojpeg.so.0")

# category -> SSIM floor (committed; ratchet UP only, never down silently)
CATEGORY_FLOOR = {
    "scene": 0.99,
    "wearable": 0.99,
    "emote": 0.99,
    "standalone-tex": 0.995,
    "_default": 0.99,
}

# ---- store resolution -------------------------------------------------------

def content_path(cid):
    h = hashlib.sha1(cid.encode()).hexdigest()[:4]
    return os.path.join(CONTENT_ROOT, h, cid)

def prod_entdir(cid):
    shard = cid[8:10] if len(cid) > 10 and cid.startswith("baf") else None
    if shard:
        d = os.path.join(PROD, shard, cid)
        if os.path.isdir(d):
            return d
    import glob
    hits = glob.glob(os.path.join(PROD, "*", cid))
    return next((c for c in hits if os.path.isdir(c)), None)

def entity_type(cid):
    p = content_path(cid)
    if not os.path.exists(p):
        return None
    try:
        return json.load(open(p)).get("type")
    except Exception:
        return None

def prod_version(cid):
    d = prod_entdir(cid)
    if not d:
        return None
    mf = os.path.join(d, "windows.manifest.json")
    if not os.path.exists(mf):
        return None
    try:
        return json.load(open(mf)).get("version")
    except Exception:
        return None

# ---- stage B: build abgen output (prod-style recipe) ------------------------

def build_abgen(cids, outdir, platform):
    os.makedirs(outdir, exist_ok=True)
    ids_file = os.path.join(outdir, "_ids.txt")
    with open(ids_file, "w") as f:
        f.write("\n".join(cids) + "\n")
    # ABGEN_ROOT points at the dir holding template/all-types.windows.bundle.
    # Default derivation is parent-of-manifest-dir (= wt/), which has no
    # template; the committed template lives at ab-generator/template.
    abgen_root = os.environ.get("ABGEN_ROOT", "/path/to/ab-generator")
    env = (f"ABGEN_ROOT={abgen_root} "
           f"ABGEN_CONTENT_ROOT={CONTENT_ROOT} "
           f"ABGEN_V38_TIMESTAMP={V38_TIMESTAMP} "
           f"ABGEN_V38_COMPAT=1 ABGEN_REAL_TEXTURES=1 "
           f"TURBOJPEG_LIB={TURBOJPEG_LIB} ")
    cmd = (f"{env} {ABGEN_CORPUS} --entity-ids {ids_file} {outdir}/out "
           f"--platform {platform} --cdn-layout --real-textures --v38-compat -j 8")
    t0 = time.time()
    r = subprocess.run([DCL_SHELL, "-c", cmd], capture_output=True, text=True)
    dt = time.time() - t0
    print(f"[abgen] {r.returncode} in {dt:.1f}s :: "
          f"{(r.stdout + r.stderr).strip().splitlines()[-1] if (r.stdout+r.stderr).strip() else ''}")
    if r.returncode != 0:
        print(r.stdout[-2000:]); print(r.stderr[-2000:])
    return os.path.join(outdir, "out")

# ---- stage C1: visual SSIM via ab-render-harness ----------------------------

def run_harness(cid, abgen_out, outdir, platform, ssim_min):
    od = os.path.join(outdir, "harness", cid)
    cmd = [HARNESS, "--target-content", cid,
           "--a", f"folder:{abgen_out}", "--b", "store:prod",
           "--platform", platform, "--out", od,
           "--allow-copy-mismatch", "--ssim-min", str(ssim_min)]
    r = subprocess.run(cmd, capture_output=True, text=True)
    rp = os.path.join(od, "report.json")
    rep = json.load(open(rp)) if os.path.exists(rp) else None
    return rep, r.returncode, (r.stdout + r.stderr)

# ---- stage C2: structural classification via classify_pair ------------------

CAT_BUCKET = {
    1: "byte-identical",
    2: "ordering/id-only",   # same length, only PathIDs / preload order differ
    3: "value-noise",        # same length, texel/float noise
    4: "value-noise",        # smaller, noise, ids same
    5: "ordering/id-only",   # smaller, ids changed
    6: "value-noise",        # larger, noise
    7: "STRUCTURAL",         # object-set / size / extra-or-missing objects
    8: "STRUCTURAL-tex-far", # structural AND pixel-far
    9: "error",
}

def abgen_bundles_for(cid, abgen_out, platform):
    wd = os.path.join(abgen_out, cid, platform)
    if not os.path.isdir(wd):
        return {}
    sfx = f"_{platform}"
    return {f: os.path.join(wd, f) for f in os.listdir(wd) if f.endswith(sfx)}

def prod_bundles_for(cid, platform):
    d = prod_entdir(cid)
    if not d:
        return {}
    wd = os.path.join(d, platform)
    if not os.path.isdir(wd):
        return {}
    sfx = f"_{platform}"
    return {f: os.path.join(wd, f) for f in os.listdir(wd) if f.endswith(sfx)}

def run_classify(cid, abgen_out, outdir, platform):
    """-> {bundle_filename: {"cat": n, "bucket": str, "evidence": str,
                             "byte_identical": bool, "in_prod": bool,
                             "in_abgen": bool}}"""
    a = abgen_bundles_for(cid, abgen_out, platform)
    p = prod_bundles_for(cid, platform)
    common = sorted(set(a) & set(p))
    only_a = sorted(set(a) - set(p))
    only_p = sorted(set(p) - set(a))

    result = {}
    # bundles present on only one side are themselves a structural signal
    for name in only_a:
        result[name] = {"cat": None, "bucket": "BUNDLE-only-abgen",
                        "evidence": "bundle absent from prod", "in_prod": False,
                        "in_abgen": True, "byte_identical": False}
    for name in only_p:
        result[name] = {"cat": None, "bucket": "BUNDLE-only-prod",
                        "evidence": "bundle absent from abgen", "in_prod": True,
                        "in_abgen": False, "byte_identical": False}

    if common:
        pairs_tsv = os.path.join(outdir, "classify", cid)
        os.makedirs(pairs_tsv, exist_ok=True)
        tsv = os.path.join(pairs_tsv, "pairs.tsv")
        with open(tsv, "w") as f:
            for name in common:
                f.write(f"{a[name]}\t{p[name]}\t{name}\n")
        r = subprocess.run([CLASSIFY_PAIR, tsv], capture_output=True, text=True)
        for line in r.stdout.splitlines():
            parts = line.split("\t")
            if len(parts) < 3:
                continue
            label, base, catstr = parts[0], parts[1], parts[2]
            ev = parts[3] if len(parts) > 3 else ""
            cat = int(catstr[3:]) if catstr.startswith("CAT") else None
            result[label] = {
                "cat": cat, "bucket": CAT_BUCKET.get(cat, "unknown"),
                "evidence": ev, "in_prod": True, "in_abgen": True,
                "byte_identical": cat == 1,
            }
    return result

# ---- combine: per-bundle cross-axis cell ------------------------------------

def visual_band(ssim, floor):
    if ssim is None:
        return "no-texture"
    if ssim >= 0.99999:
        return "visual-identical"
    if ssim >= floor:
        return "visual-ok"
    if ssim >= 0.85:
        return "visual-degraded"
    return "visual-broken"   # ~0.71 stub-vs-real signature

def is_structural_bucket(bucket):
    return bucket in ("STRUCTURAL", "STRUCTURAL-tex-far",
                      "BUNDLE-only-abgen", "BUNDLE-only-prod")

# ---- main -------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--entities-file")
    ap.add_argument("--entity", action="append", default=[])
    ap.add_argument("--out", default="/tmp/agent-06/prod-ssim-report")
    ap.add_argument("--platform", default="windows")
    ap.add_argument("--abgen-out", default=None,
                    help="reuse an existing abgen output dir (skip build)")
    args = ap.parse_args()

    cids = list(args.entity)
    if args.entities_file:
        for ln in open(args.entities_file):
            ln = ln.strip()
            if ln and not ln.startswith("#"):
                cids.append(ln)
    cids = list(dict.fromkeys(cids))
    if not cids:
        ap.error("no entities given")

    # validate presence + capture type/version
    meta = {}
    for cid in cids:
        t = entity_type(cid)
        v = prod_version(cid)
        if t is None:
            print(f"SKIP {cid}: not in content_rust"); continue
        if v is None:
            print(f"SKIP {cid}: not in prod mirror"); continue
        meta[cid] = {"type": t, "prod_version": v}
    cids = list(meta)
    if not cids:
        sys.exit("no usable entities")

    out = args.out
    os.makedirs(out, exist_ok=True)

    if args.abgen_out:
        abgen_out = args.abgen_out
    else:
        abgen_out = build_abgen(cids, os.path.join(out, "abgen-build"), args.platform)

    git_rev = subprocess.run(["git", "-C", ABGEN_RS, "rev-parse", "--short", "HEAD"],
                             capture_output=True, text=True).stdout.strip()

    entities = []
    for cid in cids:
        t = meta[cid]["type"]
        floor = CATEGORY_FLOOR.get(t, CATEGORY_FLOOR["_default"])
        print(f"\n=== {cid} type={t} prod={meta[cid]['prod_version']} floor={floor} ===")

        # (b) structural per-bundle
        cls = run_classify(cid, abgen_out, out, args.platform)
        # (c) visual SSIM per-texture
        hrep, hrc, _ = run_harness(cid, abgen_out, out, args.platform, floor)

        # index harness pairs by bundle -> worst (min) ssim across its textures
        bundle_ssim = {}
        tex_count = {}
        if hrep:
            for pr in hrep.get("pairs", []):
                bn = pr["bundle"]
                s = pr.get("ssim")
                if s is None:
                    continue
                bundle_ssim[bn] = min(bundle_ssim.get(bn, 1.0), s)
                tex_count[bn] = tex_count.get(bn, 0) + 1

        # build per-bundle cells
        bundles = []
        all_names = sorted(set(cls) | set(bundle_ssim))
        for name in all_names:
            c = cls.get(name, {"bucket": "unknown", "cat": None,
                               "byte_identical": False, "evidence": "",
                               "in_prod": name in bundle_ssim, "in_abgen": True})
            ssim = bundle_ssim.get(name)
            band = visual_band(ssim, floor)
            cell = {
                "bundle": name,
                "byte_identical": c.get("byte_identical", False),
                "cat": c.get("cat"),
                "structural_bucket": c.get("bucket"),
                "evidence": c.get("evidence", ""),
                "min_ssim": ssim,
                "visual_band": band,
                "n_textures": tex_count.get(name, 0),
                # THE cell that proves SSIM cannot mask a structural error:
                "high_ssim_structural_diff": (
                    is_structural_bucket(c.get("bucket"))
                    and ssim is not None and ssim >= floor),
            }
            bundles.append(cell)

        n = len(bundles)
        byte_id = sum(b["byte_identical"] for b in bundles)
        structural = sum(is_structural_bucket(b["structural_bucket"]) for b in bundles)
        masked = sum(b["high_ssim_structural_diff"] for b in bundles)
        ssims = [b["min_ssim"] for b in bundles if b["min_ssim"] is not None]
        entity_min_ssim = min(ssims) if ssims else None
        visual_pass = entity_min_ssim is None or entity_min_ssim >= floor

        entities.append({
            "cid": cid, "type": t, "prod_version": meta[cid]["prod_version"],
            "floor": floor, "platform": args.platform,
            "n_bundles": n,
            "byte_identical_bundles": byte_id,
            "structural_diff_bundles": structural,
            "masked_structural_diffs": masked,   # high-SSIM but structural
            "entity_min_ssim": entity_min_ssim,
            "visual_pass": visual_pass,
            "harness_rc": hrc,
            "bundles": bundles,
        })

    # roll up per category
    cats = {}
    for e in entities:
        c = cats.setdefault(e["type"], {
            "n_entities": 0, "n_bundles": 0, "byte_identical": 0,
            "structural_diff": 0, "masked_structural": 0,
            "visual_pass_entities": 0, "min_ssim": None,
            "versions": {}, "floor": e["floor"]})
        c["n_entities"] += 1
        c["n_bundles"] += e["n_bundles"]
        c["byte_identical"] += e["byte_identical_bundles"]
        c["structural_diff"] += e["structural_diff_bundles"]
        c["masked_structural"] += e["masked_structural_diffs"]
        c["visual_pass_entities"] += int(e["visual_pass"])
        c["versions"][e["prod_version"]] = c["versions"].get(e["prod_version"], 0) + 1
        if e["entity_min_ssim"] is not None:
            c["min_ssim"] = (e["entity_min_ssim"] if c["min_ssim"] is None
                             else min(c["min_ssim"], e["entity_min_ssim"]))

    report = {
        "abgen_git_rev": git_rev,
        "date": time.strftime("%Y-%m-%d"),
        "platform": args.platform,
        "category_floors": CATEGORY_FLOOR,
        "n_entities": len(entities),
        "categories": cats,
        "entities": entities,
    }
    rp = os.path.join(out, "report.json")
    json.dump(report, open(rp, "w"), indent=1)
    write_md(report, os.path.join(out, "report.md"))
    print_scoreboard(report)
    print(f"\nreport.json: {rp}\nreport.md:   {os.path.join(out, 'report.md')}")

    # reliability gate: any masked structural diff is a hard finding
    total_masked = sum(e["masked_structural_diffs"] for e in entities)
    if total_masked:
        print(f"\nGATE: {total_masked} HIGH-SSIM-but-STRUCTURAL bundle(s) — "
              f"visual score would have masked these.")

# ---- presentation -----------------------------------------------------------

def print_scoreboard(report):
    print("\n" + "=" * 78)
    print("CATEGORIZED PROD-COMPARISON SCOREBOARD  (abgen %s, %s)"
          % (report["abgen_git_rev"], report["platform"]))
    print("=" * 78)
    hdr = f"{'cid':<24} {'cat':<9} {'ver':<5} {'bnd':>3} {'byteid':>6} {'struct':>6} {'minSSIM':>9} {'masked':>6}"
    print(hdr); print("-" * len(hdr))
    for e in report["entities"]:
        s = "  n/a  " if e["entity_min_ssim"] is None else f"{e['entity_min_ssim']:.6f}"
        print(f"{e['cid'][:24]:<24} {e['type']:<9} {e['prod_version'] or '?':<5} "
              f"{e['n_bundles']:>3} {e['byte_identical_bundles']:>6} "
              f"{e['structural_diff_bundles']:>6} {s:>9} {e['masked_structural_diffs']:>6}")
    print("-" * len(hdr))
    print("per-category roll-up:")
    for cat, c in report["categories"].items():
        ms = "n/a" if c["min_ssim"] is None else f"{c['min_ssim']:.6f}"
        vers = ",".join(f"{v}:{n}" for v, n in sorted(c["versions"].items()))
        print(f"  {cat:<10} entities={c['n_entities']} bundles={c['n_bundles']} "
              f"byte-id={c['byte_identical']} structural={c['structural_diff']} "
              f"masked={c['masked_structural']} minSSIM={ms} "
              f"visual-pass={c['visual_pass_entities']}/{c['n_entities']} "
              f"floor={c['floor']} versions[{vers}]")

def write_md(report, path):
    L = []
    L.append(f"# abgen prod-comparison scoreboard — {report['date']}")
    L.append("")
    L.append(f"abgen rev `{report['abgen_git_rev']}`, platform `{report['platform']}`.")
    L.append("")
    L.append("Three independent axes per bundle — byte-identity, **structural** "
             "(classify_pair), and **visual SSIM** (ab-render-harness). The "
             "structural axis is what stops a high SSIM from hiding an "
             "encoding / ordering / PathID / binding error: the `masked` "
             "column counts bundles that pass the SSIM floor yet differ "
             "structurally from prod.")
    L.append("")
    L.append("## Per-entity")
    L.append("")
    L.append("| cid | category | prod ver | bundles | byte-id | structural | min SSIM | masked struct |")
    L.append("|---|---|---|---|---|---|---|---|")
    for e in report["entities"]:
        s = "n/a" if e["entity_min_ssim"] is None else f"{e['entity_min_ssim']:.6f}"
        L.append(f"| `{e['cid'][:16]}…` | {e['type']} | {e['prod_version']} | "
                 f"{e['n_bundles']} | {e['byte_identical_bundles']} | "
                 f"{e['structural_diff_bundles']} | {s} | {e['masked_structural_diffs']} |")
    L.append("")
    L.append("## Per-category roll-up")
    L.append("")
    L.append("| category | entities | bundles | byte-id | structural | masked | min SSIM | visual-pass | floor | prod versions |")
    L.append("|---|---|---|---|---|---|---|---|---|---|")
    for cat, c in report["categories"].items():
        ms = "n/a" if c["min_ssim"] is None else f"{c['min_ssim']:.6f}"
        vers = ", ".join(f"{v}×{n}" for v, n in sorted(c["versions"].items()))
        L.append(f"| {cat} | {c['n_entities']} | {c['n_bundles']} | "
                 f"{c['byte_identical']} | {c['structural_diff']} | "
                 f"{c['masked_structural']} | {ms} | "
                 f"{c['visual_pass_entities']}/{c['n_entities']} | {c['floor']} | {vers} |")
    L.append("")
    L.append("## Per-bundle detail")
    L.append("")
    for e in report["entities"]:
        L.append(f"### `{e['cid']}` ({e['type']}, prod {e['prod_version']})")
        L.append("")
        L.append("| bundle | byte-id | structural bucket | CAT | min SSIM | visual band | masked? | evidence |")
        L.append("|---|---|---|---|---|---|---|---|")
        for b in sorted(e["bundles"], key=lambda x: (x["min_ssim"] if x["min_ssim"] is not None else 2.0)):
            s = "n/a" if b["min_ssim"] is None else f"{b['min_ssim']:.6f}"
            cat = "" if b["cat"] is None else f"CAT{b['cat']}"
            masked = "**YES**" if b["high_ssim_structural_diff"] else ""
            ev = b["evidence"].replace("|", "\\|")[:120]
            L.append(f"| `{b['bundle'][:22]}…` | {'Y' if b['byte_identical'] else ''} | "
                     f"{b['structural_bucket']} | {cat} | {s} | {b['visual_band']} | "
                     f"{masked} | {ev} |")
        L.append("")
    open(path, "w").write("\n".join(L) + "\n")

if __name__ == "__main__":
    main()
