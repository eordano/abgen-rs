"""300-entity validation corpus, COVERAGE-oriented v3.

- 20 emote (animation variety)
- 180 wearable, stratified across BODY-SLOT categories (shape variety) + feature coverage
- 100 scene, weighted toward LARGE/HUGE models (big-model/shape/size stress)
- 1 profile (standalone-texture format validation)
- outfits: skipped (not synced to local content store; converter can't fetch)

Seed 'val300c'. Reuses build_test_corpus helpers.
"""
import json, sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import build_test_corpus as b

b.SEED = "val300c"
SEED = b.SEED

# body-slot allocation (sums to 180); covers every category, weights populous + larger-model slots
WEAR_CAT_ALLOC = {
    "skin": 23, "hands_wear": 20, "helmet": 18, "upper_body": 18, "feet": 16,
    "lower_body": 10, "hat": 10, "earring": 8, "top_head": 8, "eyewear": 8,
    "mask": 7, "tiara": 6, "hair": 6, "eyes": 5, "facial_hair": 4, "mouth": 4,
    "eyebrows": 4, "simple": 3, "body_shape": 2,
}
# scenes weighted toward large/huge
SCENE_BUCKETS = [(1, 3, 8), (4, 10, 12), (11, 50, 25), (51, 200, 30), (201, 10**9, 25)]


def has_normal_map(doc):
    return bool(doc) and any(isinstance(m, dict) and m.get("normalTexture")
                             for m in (doc.get("materials") or []))


def classify_full(eid):
    d = m = n = mm = False
    for _k, cid in b.entity_glb_cids(eid):
        doc = b.read_glb_json(cid)
        if doc is None:
            continue
        d = d or b.has_draco(doc); m = m or b.has_morph_targets(doc); n = n or has_normal_map(doc)
        mm = mm or len(doc.get("materials") or []) >= 2
    return (d, m, n, mm)


def present_pairs(pool, target):
    out = []
    with ThreadPoolExecutor(max_workers=16) as ex:
        for (eid, fc), ok in zip(pool, ex.map(lambda p: b.entity_full_present(p[0]), pool)):
            if ok:
                out.append((eid, fc))
                if len(out) >= target:
                    break
    return out


def q_type_bucket(t, lo, hi, limit, exclude=None):
    q = f"""
SELECT d.entity_id, count(c.*)::int fc FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_type='{t}'
GROUP BY d.id, d.entity_id HAVING count(c.*) BETWEEN {lo} AND {hi}
ORDER BY md5(d.entity_id || '{SEED}') LIMIT {limit};"""
    rows = [(e, int(f)) for e, f in b.psql(q)]
    return [r for r in rows if not exclude or r[0] not in exclude]


def q_wear_cat(cat, limit):
    q = f"""
SELECT d.entity_id, count(c.*)::int fc FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_type='wearable'
  AND d.entity_metadata->'v'->'data'->>'category'='{cat}'
GROUP BY d.id, d.entity_id ORDER BY md5(d.entity_id || '{SEED}') LIMIT {limit};"""
    return [(e, int(f)) for e, f in b.psql(q)]


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/validation300_entities.json"
    log = lambda *a: print(*a, file=sys.stderr)
    selected, used = [], set()

    # ---- wearables by body slot ----
    for cat, target in WEAR_CAT_ALLOC.items():
        pool = [p for p in q_wear_cat(cat, target * 4 + 15) if p[0] not in used]
        taken = present_pairs(pool, target)
        for e, fc in taken:
            selected.append((e, "wearable", fc, cat)); used.add(e)
        log(f"  wearable/{cat}: {len(taken)}/{target}")
    # fill any wearable shortfall from general pool to reach 180
    nwear = sum(1 for s in selected if s[1] == "wearable")
    if nwear < 180:
        pool = [p for p in q_type_bucket("wearable", 1, 10**9, 1500, used)]
        for e, fc in present_pairs(pool, 180 - nwear):
            selected.append((e, "wearable", fc, "?")); used.add(e)
        log(f"  wearable spillover -> {sum(1 for s in selected if s[1]=='wearable')}/180")

    # ---- scenes weighted large ----
    for lo, hi, target in SCENE_BUCKETS:
        pool = [p for p in q_type_bucket("scene", lo, hi, target * 4 + 15, used)]
        taken = present_pairs(pool, target)
        for e, fc in taken:
            selected.append((e, "scene", fc, None)); used.add(e)
        log(f"  scene {lo}-{hi}: {len(taken)}/{target}")

    # ---- emotes ----
    taken = present_pairs([p for p in q_type_bucket("emote", 1, 10**9, 120, used)], 20)
    for e, fc in taken:
        selected.append((e, "emote", fc, None)); used.add(e)
    log(f"  emote: {len(taken)}/20")

    # ---- 1 profile ----
    taken = present_pairs([p for p in q_type_bucket("profile", 2, 10**9, 40, used)], 1)
    for e, fc in taken:
        selected.append((e, "profile", fc, None)); used.add(e)
    log(f"  profile: {len(taken)}/1")

    # ---- feature classification + force morph/draco ----
    log("[feat] classifying ...")
    feats = {}
    with ThreadPoolExecutor(max_workers=16) as ex:
        for e, fl in zip([s[0] for s in selected], ex.map(classify_full, [s[0] for s in selected])):
            feats[e] = fl
    cov = lambda i: sum(1 for s in selected if feats.get(s[0], (0,0,0,0))[i])
    # force >=3 draco scenes
    if cov(0) < 3:
        wide = [e for e, _ in q_type_bucket("scene", 1, 10**9, 1500, used)]
        with ThreadPoolExecutor(max_workers=16) as ex:
            dr = [e for e, isd in zip(wide, ex.map(lambda e: classify_full(e)[0], wide)) if isd]
        dr = [e for e in dr if b.entity_full_present(e)]
        sw = [i for i, s in enumerate(selected) if s[1] == "scene" and not feats.get(s[0],(0,0,0,0))[0]]
        for e in dr[:max(0, 3 - cov(0))]:
            if not sw: break
            i = sw.pop(); used.discard(selected[i][0]); used.add(e)
            feats[e] = classify_full(e); selected[i] = (e, "scene", selected[i][2], None)
    # force >=4 morph wearables
    if cov(1) < 4:
        morph = [eid for eid, et, _ in (b.discover_morph_entities(limit=12000) or [])
                 if et == "wearable" and eid not in used and b.entity_full_present(eid)]
        sw = [i for i, s in enumerate(selected) if s[1] == "wearable" and not feats.get(s[0],(0,0,0,0))[1]]
        for e in morph[:max(0, 4 - cov(1))]:
            if not sw: break
            i = sw.pop(); used.discard(selected[i][0]); used.add(e)
            feats[e] = classify_full(e); selected[i] = (e, "wearable", selected[i][2], "morph")

    selected.sort(key=lambda x: (x[2], x[0]))
    by_type = Counter(t for _, t, _, _ in selected)
    wear_by_cat = Counter(c for _, t, _, c in selected if t == "wearable")
    buckets = {"1-3":0,"4-10":0,"11-50":0,"51-200":0,"200+":0}
    for _e,_t,fc,_c in selected:
        k=("1-3" if fc<=3 else "4-10" if fc<=10 else "11-50" if fc<=50 else "51-200" if fc<=200 else "200+")
        buckets[k]+=1
    doc = {
        "_purpose": "abgen-rs 300 validation corpus v3 — coverage (body slots + sizes + features)",
        "_seed": SEED, "_count": len(selected), "_by_type": dict(by_type),
        "_wearable_by_category": dict(wear_by_cat),
        "_coverage": {"draco": cov(0), "morph": cov(1), "normalmap": cov(2), "multi_material": cov(3)},
        "_file_count_buckets": buckets,
        "_note": "outfits skipped (not on local content store); 1 profile for texture-format coverage.",
        "entities": [{"entity_id": e, "entity_type": t, "file_count": fc,
                      "category": c if t == "wearable" else None,
                      "flags": [f for f,on in zip(("draco","morph","normalmap","multi_material"),
                                                  feats.get(e,(0,0,0,0))) if on]}
                     for e, t, fc, c in selected],
    }
    with open(out, "w") as f:
        json.dump(doc, f, indent=2)
    log(f"wrote {out}: {len(selected)} by_type={dict(by_type)} "
        f"coverage(draco={cov(0)},morph={cov(1)},normal={cov(2)},multimat={cov(3)}) buckets={buckets}")
    log(f"  wearable_by_category={dict(wear_by_cat)}")


if __name__ == "__main__":
    main()
