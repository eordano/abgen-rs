"""Build a feature-coverage-balanced 30-entity test corpus for abgen-rs parity.

Composition (per user request): ~12 scenes / ~12 wearables / ~6 emotes.
Coverage forced: draco, morph/blendshape, normal-map (BC5), large scene,
plus natural collider/empty/multi-material/animation coverage.

Per-CID conversion (ExportSceneBatch -> ConvertEntityById) is entity-type
agnostic, so wearables/emotes can be referenced by CID alongside scenes.

Disjoint from prior corpora via SEED='test30'. Reuses DB/content helpers from
build_test_corpus.py.
"""
import json
import sys
from collections import Counter
from concurrent.futures import ProcessPoolExecutor, as_completed

import build_test_corpus as b

b.SEED = "test30"
SEED = b.SEED


def pool(etype, limit, min_files=1):
    q = f"""
SELECT d.entity_id, count(c.*)::int fc
FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_type='{etype}'
GROUP BY d.id, d.entity_id HAVING count(c.*) >= {min_files}
ORDER BY md5(d.entity_id || '{SEED}') LIMIT {limit};
"""
    return [(e, int(f)) for e, f in b.psql(q)]


def scene_buckets(per_bucket=3, nb=6):
    q = f"""
WITH s AS (
  SELECT d.entity_id, count(c.*)::int fc,
         NTILE({nb}) OVER (ORDER BY count(c.*)) bucket
  FROM deployments d JOIN content_files c ON c.deployment=d.id
  WHERE d.deleter_deployment IS NULL AND d.entity_type='scene'
  GROUP BY d.id, d.entity_id)
SELECT entity_id, fc, bucket FROM (
  SELECT entity_id, fc, bucket,
         ROW_NUMBER() OVER (PARTITION BY bucket ORDER BY md5(entity_id || '{SEED}')) rn
  FROM s) t WHERE rn <= {per_bucket}
ORDER BY bucket, rn;
"""
    return [(e, int(f), int(bk)) for e, f, bk in b.psql(q)]


def has_normal_map(doc):
    if not doc:
        return False
    for m in (doc.get("materials") or []):
        if isinstance(m, dict) and m.get("normalTexture"):
            return True
    return False


def classify_full(eid):
    """(has_draco, has_morph, has_normal_map) by scanning the entity's glbs."""
    d = m = n = False
    for _k, cid in b.entity_glb_cids(eid):
        doc = b.read_glb_json(cid)
        if doc is None:
            continue
        d = d or b.has_draco(doc)
        m = m or b.has_morph_targets(doc)
        n = n or has_normal_map(doc)
    return (d, m, n)


def present(pairs):
    return [t for t in pairs if b.entity_full_present(t[0])]


def main():
    out_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/test30_entities.json"
    log = lambda *a: print(*a, file=sys.stderr)

    log("[1] fetching candidate pools ...")
    scenes_b = present(scene_buckets(per_bucket=3, nb=6))        # ~18 -> trim to 12
    big = present(pool("scene", 8, min_files=1000))              # large scenes
    wears = present(pool("wearable", 300))                       # wide: force morph/normal coverage
    emotes = present(pool("emote", 20))
    log(f"  scenes(buckets on disk)={len(scenes_b)} big={len(big)} wear={len(wears)} emote={len(emotes)}")

    log("[2] classifying for draco/morph/normal ...")
    targets = [e for e, _, _ in scenes_b] + [e for e, _ in big + wears + emotes]
    feats = {}
    with ProcessPoolExecutor(max_workers=16) as ex:
        futs = {ex.submit(classify_full, e): e for e in set(targets)}
        for fut in as_completed(futs):
            feats[futs[fut]] = fut.result()

    selected = []  # (eid, type, fc, flags)
    used = set()

    def add(eid, et, fc, extra=None):
        if eid in used:
            return False
        d, m, n = feats.get(eid, (False, False, False))
        flags = []
        if d: flags.append("draco")
        if m: flags.append("morph")
        if n: flags.append("normalmap")
        if extra: flags += [x for x in extra if x not in flags]
        selected.append((eid, et, fc, flags))
        used.add(eid)
        return True

    # ---- scenes: 12 (one per bucket spread, force 1 draco + 1 large) ----
    fc_of = {e: f for e, f, _ in scenes_b}
    fc_of.update({e: f for e, f in big})
    # force a draco scene if any in pool
    draco_scene = next((e for e in [s for s, _, _ in scenes_b] if feats.get(e, (0,0,0))[0]), None)
    if draco_scene:
        add(draco_scene, "scene", fc_of.get(draco_scene, 0))
    # force a large scene
    if big:
        add(big[0][0], "scene", big[0][1], ["large"])
    # fill remaining scene slots round-robin across buckets
    by_bucket = {}
    for e, f, bk in scenes_b:
        by_bucket.setdefault(bk, []).append((e, f))
    bks = sorted(by_bucket)
    i = 0
    while sum(1 for _, t, _, _ in selected if t == "scene") < 12 and any(by_bucket.values()):
        bk = bks[i % len(bks)]
        if by_bucket[bk]:
            e, f = by_bucket[bk].pop(0)
            add(e, "scene", f)
        i += 1
        if i > 1000:
            break

    # ---- wearables: 12 (force >=2 morph, >=3 normalmap) ----
    wfc = {e: f for e, f in wears}
    morph_w = [e for e, _ in wears if feats.get(e, (0,0,0))[1]]
    norm_w = [e for e, _ in wears if feats.get(e, (0,0,0))[2]]
    # morph wearables are rare; if the 300-pool has none, discover from a wide avatar scan
    if not morph_w:
        log("  no morph wearable in pool; discovering ...")
        for eid, et, fc in (b.discover_morph_entities(limit=12000) or []):
            if et == "wearable" and b.entity_full_present(eid):
                feats[eid] = classify_full(eid)
                wfc[eid] = int(fc)
                morph_w.append(eid)
                if len(morph_w) >= 2:
                    break
    nwear = lambda: sum(1 for _, t, _, _ in selected if t == "wearable")
    for e in morph_w[:2]:
        add(e, "wearable", wfc.get(e, 0))
    for e in norm_w:
        if nwear() >= 6:
            break
        add(e, "wearable", wfc.get(e, 0))
    for e, f in wears:
        if nwear() >= 12:
            break
        add(e, "wearable", f)

    # ---- emotes: 6 ----
    for e, f in emotes:
        if sum(1 for _, t, _, _ in selected if t == "emote") >= 6:
            break
        add(e, "emote", f)

    # ---- write ----
    by_type = Counter(t for _, t, _, _ in selected)
    def cov(flag):
        return sum(1 for _, _, _, fl in selected if flag in fl)
    buckets = {"1-3": 0, "4-10": 0, "11-50": 0, "51-200": 0, "200+": 0}
    for _e, _t, fc, _ in selected:
        k = ("1-3" if fc <= 3 else "4-10" if fc <= 10 else "11-50" if fc <= 50
             else "51-200" if fc <= 200 else "200+")
        buckets[k] += 1

    entities_out = [{
        "entity_id": e, "entity_type": t, "file_count": fc, "flags": fl,
    } for e, t, fc, fl in selected]

    doc = {
        "_purpose": "abgen-rs 30-entity feature-balanced parity test corpus (per-CID)",
        "_seed": SEED,
        "_count": len(entities_out),
        "_by_type": dict(by_type),
        "_coverage": {"draco": cov("draco"), "morph": cov("morph"),
                       "normalmap": cov("normalmap"), "large": cov("large")},
        "_file_count_buckets": buckets,
        "entities": entities_out,
    }
    with open(out_path, "w") as f:
        json.dump(doc, f, indent=2)
    log(f"wrote {out_path}: {len(entities_out)} entities  by_type={dict(by_type)} "
        f"coverage(draco={cov('draco')},morph={cov('morph')},normal={cov('normalmap')},large={cov('large')})")


if __name__ == "__main__":
    main()
