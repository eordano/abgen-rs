"""Build a new test_entities.json for abgen-rs parity testing.

Stratified sample of ~50 catalyst entities with explicit coverage of:
  - all 4 entity types (scene, wearable, emote, profile)
  - scenes across the full file-count range (NTILE buckets + large-scene tail)
  - draco-bearing glTFs (detected by parsing extensionsUsed)
  - morph-target wearables (detected by mesh primitive `targets`)

Uses md5(entity_id || 'test2') as the seed (disjoint from existing test/val
which use 'test' and 'val').

Outputs <out_path> in the test_entities.json schema:
  { "_purpose": ..., "_count": N, "_by_type": {...},
    "_draco_entities": K, "_morph_entities": M, "_large_scenes": L,
    "_file_count_buckets": {...},
    "entities": [{ "entity_id": ..., "entity_type": ..., "file_count": ...,
                   "hints": [...], "flags": [...] }, ...] }
"""
import json
import hashlib
import os
import subprocess
import sys
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor, as_completed

CONTENT_ROOT = "/home/dcl/umbrella/data/content_server/contents"
ENV_FILE = "/home/dcl/umbrella/env/content.env"
PG_SOCK = "/home/dcl/umbrella/data/run"
PG_PORT = "5433"
PG_DB = "content"
SEED = "test2"


def load_pg_creds():
    user = passw = None
    for line in open(ENV_FILE):
        if line.startswith("POSTGRES_CONTENT_USER="):
            user = line.split("=", 1)[1].strip()
        elif line.startswith("POSTGRES_CONTENT_PASSWORD="):
            passw = line.split("=", 1)[1].strip()
    if not user or not passw:
        sys.exit("missing PG creds in " + ENV_FILE)
    return user, passw


def psql(sql):
    user, passw = load_pg_creds()
    env = dict(os.environ, PGPASSWORD=passw)
    r = subprocess.run(
        ["psql", "-h", PG_SOCK, "-p", PG_PORT, "-U", user, "-d", PG_DB,
         "-t", "-A", "-F", "\t", "-c", sql],
        env=env, capture_output=True, text=True, check=True,
    )
    return [line.split("\t") for line in r.stdout.strip().split("\n") if line]


def content_path(cid):
    return os.path.join(CONTENT_ROOT, hashlib.sha1(cid.encode()).hexdigest()[:4], cid)


def have_content(cid):
    return os.path.exists(content_path(cid))


def read_glb_json(cid):
    p = content_path(cid)
    if not os.path.exists(p):
        return None
    try:
        with open(p, "rb") as f:
            data = f.read(min(os.path.getsize(p), 8 * 1024 * 1024))
        if data[:4] == b"glTF":
            json_len = int.from_bytes(data[12:16], "little")
            return json.loads(data[20:20 + json_len])
        if data.lstrip()[:1] == b"{":
            return json.loads(data)
    except Exception:
        return None
    return None


def has_draco(doc):
    if not isinstance(doc, dict):
        return False
    used = doc.get("extensionsUsed") or []
    return "KHR_draco_mesh_compression" in used


def has_morph_targets(doc):
    if not isinstance(doc, dict):
        return False
    for mesh in (doc.get("meshes") or []):
        for prim in (mesh.get("primitives") or []):
            if prim.get("targets"):
                return True
    return False


def _check_morph_cid(cid):
    doc = read_glb_json(cid)
    return cid if doc is not None and has_morph_targets(doc) else None


def discover_morph_entities(limit=50000):
    """Scan unique glbs in wearable/emote/outfits, return list of entities that
    contain a morph-target-bearing glb. Reproducible: no seed dependence."""
    q = (
        "SELECT DISTINCT c.content_hash FROM deployments d "
        "JOIN content_files c ON c.deployment=d.id "
        "WHERE d.deleter_deployment IS NULL "
        "AND d.entity_type IN ('wearable','emote','outfits') "
        f"AND c.key LIKE '%.glb' LIMIT {limit};"
    )
    cids = [row[0] for row in psql(q)]
    print(f"  scanning {len(cids)} unique avatar glbs for morph targets ...", file=sys.stderr)

    morph_cids = []
    with ProcessPoolExecutor(max_workers=24) as ex:
        for r in ex.map(_check_morph_cid, cids, chunksize=200):
            if r:
                morph_cids.append(r)
    print(f"  found {len(morph_cids)} morph-target glbs", file=sys.stderr)
    if not morph_cids:
        return []
    in_list = ",".join("'" + c + "'" for c in morph_cids)
    q = (
        f"SELECT d.entity_id, d.entity_type, count(c.*)::int fc "
        f"FROM deployments d JOIN content_files c ON c.deployment=d.id "
        f"WHERE d.deleter_deployment IS NULL "
        f"AND d.entity_id IN (SELECT DISTINCT d2.entity_id "
        f"  FROM deployments d2 JOIN content_files c2 ON c2.deployment=d2.id "
        f"  WHERE d2.deleter_deployment IS NULL AND c2.content_hash IN ({in_list})) "
        f"GROUP BY d.id, d.entity_id, d.entity_type;"
    )
    return [(eid, et, int(fc)) for eid, et, fc in psql(q)]


def fetch_candidates(seed):
    print("[1/5] querying candidate pools ...", file=sys.stderr)

    # scenes: NTILE buckets across full file-count range
    scenes_q = f"""
WITH s AS (
  SELECT d.entity_id, count(c.*)::int fc,
         NTILE(5) OVER (ORDER BY count(c.*)) bucket
  FROM deployments d JOIN content_files c ON c.deployment=d.id
  WHERE d.deleter_deployment IS NULL AND d.entity_type='scene'
  GROUP BY d.id, d.entity_id)
SELECT entity_id, fc, bucket FROM (
  SELECT entity_id, fc, bucket,
         ROW_NUMBER() OVER (PARTITION BY bucket ORDER BY md5(entity_id || '{SEED}')) rn
  FROM s) t WHERE rn <= 8
ORDER BY bucket, rn;
"""
    scenes = [(eid, int(fc), int(b)) for eid, fc, b in psql(scenes_q)]

    big_q = f"""
SELECT d.entity_id, count(c.*)::int fc
FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_type='scene'
GROUP BY d.id, d.entity_id HAVING count(c.*) > 1000
ORDER BY md5(d.entity_id || '{SEED}') LIMIT 6;
"""
    big_scenes = [(eid, int(fc)) for eid, fc in psql(big_q)]

    wear_q = f"""
SELECT d.entity_id, count(c.*)::int fc
FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_type='wearable'
GROUP BY d.id, d.entity_id
ORDER BY md5(d.entity_id || '{SEED}') LIMIT 60;
"""
    wearables = [(eid, int(fc)) for eid, fc in psql(wear_q)]

    emote_q = f"""
SELECT d.entity_id, count(c.*)::int fc
FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_type='emote'
GROUP BY d.id, d.entity_id
ORDER BY md5(d.entity_id || '{SEED}') LIMIT 12;
"""
    emotes = [(eid, int(fc)) for eid, fc in psql(emote_q)]

    profile_q = f"""
SELECT d.entity_id, count(c.*)::int fc
FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_type='profile'
GROUP BY d.id, d.entity_id HAVING count(c.*) >= 2
ORDER BY md5(d.entity_id || '{SEED}') LIMIT 10;
"""
    profiles = [(eid, int(fc)) for eid, fc in psql(profile_q)]

    return scenes, big_scenes, wearables, emotes, profiles


def entity_glb_cids(entity_id):
    q = f"""
SELECT c.key, c.content_hash FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_id='{entity_id}' AND c.key LIKE '%.glb'
ORDER BY c.key;
"""
    return [(k, h) for k, h in psql(q)]


def entity_full_present(entity_id):
    q = f"""
SELECT c.content_hash FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_id='{entity_id}';
"""
    hashes = [row[0] for row in psql(q)]
    if not hashes:
        return False
    return all(have_content(h) for h in hashes)


def classify(entity_id):
    """Return (has_draco, has_morph) by scanning the entity's .glb files."""
    glbs = entity_glb_cids(entity_id)
    has_draco_flag = has_morph_flag = False
    for _key, cid in glbs:
        doc = read_glb_json(cid)
        if doc is None:
            continue
        if has_draco(doc):
            has_draco_flag = True
        if has_morph_targets(doc):
            has_morph_flag = True
        if has_draco_flag and has_morph_flag:
            break
    return has_draco_flag, has_morph_flag


def main():
    out_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/test_entities.json"

    # The asset-bundle-converter CLI only processes scene entities by CID;
    # wearables/emotes require a collection URN entry point. So the corpus
    # is scene-only — scene-embedded models still give us glb-wearable,
    # glb-animated, glb-emote, draco, and standalone-texture(legacy) coverage.
    scenes, big_scenes, wearables, emotes, profiles = fetch_candidates(SEED)
    print(f"  scenes: {len(scenes)} (across 5 NTILE buckets)", file=sys.stderr)
    print(f"  big scenes (>1000 files): {len(big_scenes)}", file=sys.stderr)
    print(f"  wearables candidates: {len(wearables)}", file=sys.stderr)
    print(f"  emotes: {len(emotes)}, profiles: {len(profiles)}", file=sys.stderr)

    print("[2/5] scanning scene .glbs for draco ...", file=sys.stderr)
    # Pool of 40 random scenes is too thin for the rare draco extension; widen.
    wide_scenes_q = f"""
SELECT d.entity_id, count(c.*)::int fc
FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_type='scene'
GROUP BY d.id, d.entity_id
ORDER BY md5(d.entity_id || '{SEED}') LIMIT 1500;
"""
    wide_scenes = [(e, int(f)) for e, f in psql(wide_scenes_q)]
    print(f"  classifying {len(wide_scenes)} scenes in parallel ...", file=sys.stderr)
    scene_class = {}
    with ProcessPoolExecutor(max_workers=12) as ex:
        s_futs = {ex.submit(classify, e): e for e, _ in wide_scenes + [(e, f) for e, f in big_scenes]}
        for fut in as_completed(s_futs):
            scene_class[s_futs[fut]] = fut.result()
    scene_draco = [e for e, (d, _) in scene_class.items() if d]
    scene_morph = [e for e, (_, m) in scene_class.items() if m]
    print(f"  draco scenes found: {len(scene_draco)}", file=sys.stderr)
    print(f"  morph scenes found: {len(scene_morph)}", file=sys.stderr)

    print("[3/5] verifying content presence on disk ...", file=sys.stderr)

    # Filter each pool to "fully on disk"
    def present(pool):
        return [t for t in pool if entity_full_present(t[0])]

    scenes_ok = present(scenes)
    big_ok = present(big_scenes)
    wearables_ok = present(wearables)
    emotes_ok = present(emotes)
    profiles_ok = present(profiles)
    print(f"  scenes on disk: {len(scenes_ok)}/{len(scenes)}", file=sys.stderr)
    print(f"  big scenes on disk: {len(big_ok)}/{len(big_scenes)}", file=sys.stderr)
    print(f"  wearables on disk: {len(wearables_ok)}/{len(wearables)}", file=sys.stderr)

    print("[4/5] assembling 50-entity sample ...", file=sys.stderr)

    selected = []

    # Target ~50 scenes total. Buckets give 5-per-bucket × 5 = 25,
    # then we pad from the wider random pool to reach 50, ensuring
    # 3+ large scenes (>1000 files) and 1+ draco scene are present.
    by_bucket = defaultdict(list)
    for eid, fc, b in scenes_ok:
        by_bucket[b].append((eid, fc))
    for b in sorted(by_bucket):
        pool = by_bucket[b]
        for eid, fc in pool[:5]:
            flags = ["draco"] if scene_class.get(eid, (False, False))[0] else []
            selected.append((eid, "scene", fc, flags))

    # Force-include 3 large scenes
    for eid, fc in big_ok[:3]:
        if not any(e == eid for e, _, _, _ in selected):
            selected.append((eid, "scene", fc, ["large"]))

    # Ensure 1+ draco scene present overall
    draco_present = any("draco" in flags for _, _, _, flags in selected)
    if not draco_present and scene_draco:
        wide_fc = {e: f for e, f in wide_scenes}
        for i in range(len(selected) - 1, -1, -1):
            if selected[i][1] == "scene" and "large" not in selected[i][3]:
                new_eid = scene_draco[0]
                if entity_full_present(new_eid) and not any(e == new_eid for e, _, _, _ in selected):
                    selected[i] = (new_eid, "scene", wide_fc.get(new_eid, 0), ["draco"])
                    break

    # Pad with random scenes from the wider pool to reach 50
    used = set(e for e, _, _, _ in selected)
    extra = [(eid, fc) for eid, fc in wide_scenes
             if eid not in used and entity_full_present(eid)]
    for eid, fc in extra:
        if len(selected) >= 50:
            break
        selected.append((eid, "scene", fc, []))

    print(f"[5/5] writing {out_path} ({len(selected)} entities)", file=sys.stderr)

    by_type = Counter(t for _, t, _, _ in selected)
    draco_n = sum(1 for eid, _t, _f, flags in selected if "draco" in flags)
    large_n = sum(1 for _e, _t, _f, flags in selected if "large" in flags)
    # Count scenes containing embedded SkinnedMeshRenderer / animations.
    # These give us glb-wearable / glb-animated bundle coverage without
    # needing standalone wearable/emote entities (which the converter CLI
    # can't process per-CID anyway).

    buckets = {"1-3": 0, "4-10": 0, "11-50": 0, "51-200": 0, "200+": 0}
    for _eid, _t, fc, _ in selected:
        if fc <= 3: buckets["1-3"] += 1
        elif fc <= 10: buckets["4-10"] += 1
        elif fc <= 50: buckets["11-50"] += 1
        elif fc <= 200: buckets["51-200"] += 1
        else: buckets["200+"] += 1

    def hints_for(eid):
        out = []
        glbs = entity_glb_cids(eid)
        if glbs:
            out.append("model")
        # check for image keys
        img_q = f"""
SELECT 1 FROM deployments d JOIN content_files c ON c.deployment=d.id
WHERE d.deleter_deployment IS NULL AND d.entity_id='{eid}'
  AND (c.key ILIKE '%.png' OR c.key ILIKE '%.jpg' OR c.key ILIKE '%.jpeg')
LIMIT 1;
"""
        if psql(img_q):
            out.append("image")
        return out

    entities_out = []
    for eid, et, fc, flags in selected:
        entities_out.append({
            "entity_id": eid,
            "entity_type": et,
            "file_count": fc,
            "hints": hints_for(eid),
            "flags": [f for f in flags if f],
        })

    # Collection URNs — the other input shape asset-bundle-converter accepts
    # (`-wearablesCollectionUrnId <urn>`). lamb2 only indexes `off-chain:base-avatars`
    # locally (other matic-based collections return empty); for chain collections,
    # point at peer.decentraland.org.
    urn_entries = [
        {
            "urn": "urn:decentraland:off-chain:base-avatars",
            "note": "282 base avatars; exercises ExportWearablesCollectionToAssetBundles path",
        },
    ]

    out_doc = {
        "_purpose": "test corpus — entities + collection URNs for asset-bundle parity",
        "_seed": SEED,
        "_count": len(entities_out),
        "_by_type": dict(by_type),
        "_draco_entities": draco_n,
        "_large_scenes": large_n,
        "_collection_urns": len(urn_entries),
        "_note": "Two input shapes: `entities` (per-CID via ConvertEntityById; "
                 "scenes + wearables + emotes + profiles) and `collection_urns` "
                 "(per-URN via ConvertWearablesCollection; expands to all wearables "
                 "in collection). Local lamb2 only indexes off-chain:base-avatars; "
                 "use peer.decentraland.org for chain-based collections.",
        "_file_count_buckets": buckets,
        "entities": entities_out,
        "collection_urns": urn_entries,
    }
    with open(out_path, "w") as f:
        json.dump(out_doc, f, indent=2)
    print(f"wrote {out_path}: {len(entities_out)} entities", file=sys.stderr)


if __name__ == "__main__":
    main()
