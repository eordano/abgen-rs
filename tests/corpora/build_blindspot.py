"""Mine the catalyst content store for entities exercising coverage blind-spots.

Read-only: queries the `content` postgres DB to enumerate wearable/emote/scene
glbs, resolves each glb CID to the on-disk content store, parses the glTF JSON
chunk, and classifies which rare features it exercises. Produces a deduped,
feature-balanced CID list (one entity CID per line) for a FUTURE Unity
asset-bundle reference run that deliberately over-samples these blind spots.

Target features (detected by parsing the glTF JSON chunk):
  - morph        : mesh primitive `targets` (morph targets / blendshapes)
  - draco        : KHR_draco_mesh_compression in extensionsUsed
  - specgloss    : KHR_materials_pbrSpecularGlossiness on a material
  - texturexform : KHR_texture_transform on a texture reference
  - transmission : KHR_materials_transmission on a material
  - multimat     : a single mesh with >=2 primitives using distinct materials
  - multiclip    : >=2 animations (multi-clip)
  - highjoints   : a skin with a large joints[] array
  - vertexcolor  : a primitive with a COLOR_0 attribute
  - uv1          : a primitive with a TEXCOORD_1 attribute

NO converter / abgen invocation. NO ppm. Pure corpus curation.
"""
import json
import os
import sys
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor

import build_test_corpus as b

# ----- feature detectors over a parsed glTF doc -----

HIGH_JOINT_THRESHOLD = 40  # "high-joint-count skin"


def _meshes(doc):
    return [m for m in (doc.get("meshes") or []) if isinstance(m, dict)]


def _prims(mesh):
    return [p for p in (mesh.get("primitives") or []) if isinstance(p, dict)]


def has_morph(doc):
    if not isinstance(doc, dict):
        return False
    for mesh in _meshes(doc):
        for prim in _prims(mesh):
            if prim.get("targets"):
                return True
    return False


def has_draco(doc):
    return b.has_draco(doc)


def has_specgloss(doc):
    if not isinstance(doc, dict):
        return False
    if "KHR_materials_pbrSpecularGlossiness" in (doc.get("extensionsUsed") or []):
        return True
    for mat in (doc.get("materials") or []):
        if isinstance(mat, dict) and "KHR_materials_pbrSpecularGlossiness" in (mat.get("extensions") or {}):
            return True
    return False


def has_texture_transform(doc):
    if not isinstance(doc, dict):
        return False
    return "KHR_texture_transform" in (doc.get("extensionsUsed") or [])


def has_transmission(doc):
    if not isinstance(doc, dict):
        return False
    if "KHR_materials_transmission" in (doc.get("extensionsUsed") or []):
        return True
    for mat in (doc.get("materials") or []):
        if isinstance(mat, dict) and "KHR_materials_transmission" in (mat.get("extensions") or {}):
            return True
    return False


def has_multimat(doc):
    """A single mesh whose primitives reference >=2 distinct materials."""
    if not isinstance(doc, dict):
        return False
    for mesh in _meshes(doc):
        mats = set()
        for prim in _prims(mesh):
            if "material" in prim:
                mats.add(prim["material"])
        if len(mats) >= 2:
            return True
    return False


def has_multiclip(doc):
    if not isinstance(doc, dict):
        return False
    anims = doc.get("animations")
    return isinstance(anims, list) and len(anims) >= 2


def max_joints(doc):
    if not isinstance(doc, dict):
        return 0
    m = 0
    for skin in (doc.get("skins") or []):
        if isinstance(skin, dict):
            m = max(m, len(skin.get("joints") or []))
    return m


def has_highjoints(doc):
    return max_joints(doc) >= HIGH_JOINT_THRESHOLD


def has_vertexcolor(doc):
    if not isinstance(doc, dict):
        return False
    for mesh in _meshes(doc):
        for prim in _prims(mesh):
            if any(k.startswith("COLOR_") for k in (prim.get("attributes") or {})):
                return True
    return False


def has_uv1(doc):
    if not isinstance(doc, dict):
        return False
    for mesh in _meshes(doc):
        for prim in _prims(mesh):
            if "TEXCOORD_1" in (prim.get("attributes") or {}):
                return True
    return False


FEATURES = {
    "morph": has_morph,
    "draco": has_draco,
    "specgloss": has_specgloss,
    "texturexform": has_texture_transform,
    "transmission": has_transmission,
    "multimat": has_multimat,
    "multiclip": has_multiclip,
    "highjoints": has_highjoints,
    "vertexcolor": has_vertexcolor,
    "uv1": has_uv1,
}


def classify_cid(cid):
    """Return (cid, frozenset_of_feature_names, max_joints) for one glb, or None."""
    doc = b.read_glb_json(cid)
    if not isinstance(doc, dict):
        return None
    try:
        feats = frozenset(name for name, fn in FEATURES.items() if fn(doc))
        return (cid, feats, max_joints(doc))
    except Exception:
        return None


def enumerate_glbs(entity_types, limit):
    """Return list of (content_hash, entity_id, entity_type) for unique glbs.

    One representative entity per distinct glb content_hash (smallest by md5
    ordering) keeps the entity list deduped at the CID level.
    """
    in_types = ",".join("'" + t + "'" for t in entity_types)
    q = f"""
WITH g AS (
  SELECT c.content_hash, d.entity_id, d.entity_type,
         ROW_NUMBER() OVER (PARTITION BY c.content_hash
                            ORDER BY md5(d.entity_id || 'blindspot')) rn
  FROM deployments d JOIN content_files c ON c.deployment=d.id
  WHERE d.deleter_deployment IS NULL
    AND d.entity_type IN ({in_types})
    AND c.key LIKE '%.glb'
)
SELECT content_hash, entity_id, entity_type FROM g WHERE rn = 1
ORDER BY md5(content_hash || 'blindspot') LIMIT {limit};
"""
    return [(h, e, t) for h, e, t in b.psql(q)]


def main():
    target_root = os.environ.get("ABGEN_CONTENT_ROOT")
    if target_root:
        b.CONTENT_ROOT = target_root

    avatar_limit = int(sys.argv[1]) if len(sys.argv) > 1 else 40000
    scene_limit = int(sys.argv[2]) if len(sys.argv) > 2 else 30000

    print("[1/3] enumerating unique glbs ...", file=sys.stderr)
    avatar_glbs = enumerate_glbs(["wearable", "emote", "outfits"], avatar_limit)
    scene_glbs = enumerate_glbs(["scene"], scene_limit)
    print(f"  avatar glbs: {len(avatar_glbs)}  scene glbs: {len(scene_glbs)}", file=sys.stderr)

    rep = {}
    for h, e, t in avatar_glbs + scene_glbs:
        rep.setdefault(h, (e, t))

    all_cids = [h for h in rep if b.have_content(h)]
    print(f"  present on disk: {len(all_cids)}/{len(rep)}", file=sys.stderr)

    print("[2/3] classifying glbs (parallel) ...", file=sys.stderr)
    results = []
    with ProcessPoolExecutor(max_workers=24) as ex:
        for r in ex.map(classify_cid, all_cids, chunksize=200):
            if r is not None and r[1]:
                results.append(r)
    print(f"  glbs with >=1 target feature: {len(results)}", file=sys.stderr)

    is_scene = {h: (rep[h][1] == "scene") for h in rep}

    by_feat = defaultdict(list)
    for cid, feats, joints in results:
        for f in feats:
            by_feat[f].append((cid, feats, joints))

    print("[3/3] feature availability (avatar / scene):", file=sys.stderr)
    for f in FEATURES:
        pool = by_feat.get(f, [])
        sc = sum(1 for c, _, _ in pool if is_scene.get(c))
        print(f"  {f:14s} {len(pool):6d}  ({len(pool)-sc} avatar / {sc} scene)", file=sys.stderr)

    # Selection: per-feature quota, split so each feature draws from BOTH the
    # avatar (wearable/emote/outfits) and scene pipelines where available.
    # The two pipelines exercise different converter paths
    # (SkinnedMeshRenderer/blendshapes vs. scene MeshRenderer/colliders), so a
    # blind-spot list must cover both. Greedy still prefers multi-feature glbs
    # within each (feature, pipeline) bucket to keep the list compact.
    PER_FEATURE = 24
    PER_SIDE = 12 # soft target per pipeline-side per feature (avatar, scene)
    selected = {}  # cid -> feats
    feat_count = defaultdict(int)
    feat_side_count = defaultdict(int)  # (feat, is_scene) -> n

    def take(feat, want_scene, side_cap):
        pool = [it for it in by_feat.get(feat, [])
                if is_scene.get(it[0]) == want_scene]
        pool.sort(key=lambda it: (sum(1 for ff in it[1] if feat_count[ff] < PER_FEATURE),
                                  it[2]), reverse=True)
        for cid, feats, joints in pool:
            if feat_count[feat] >= PER_FEATURE:
                return
            if side_cap is not None and feat_side_count[(feat, want_scene)] >= side_cap:
                return
            if cid in selected:
                continue
            selected[cid] = feats
            for ff in feats:
                feat_count[ff] += 1
                feat_side_count[(ff, want_scene)] += 1

    order = sorted(FEATURES, key=lambda f: len(by_feat.get(f, [])))
    # Pass 1: rarest features first; fill avatar side then scene side, soft cap.
    for feat in order:
        take(feat, False, PER_SIDE)  # avatar
        take(feat, True, PER_SIDE)   # scene
    # Pass 2: top up any feature still short of PER_FEATURE, ignoring side cap
    # (features that live almost exclusively in scene glbs, e.g. draco/specgloss).
    for feat in order:
        take(feat, False, None)
        take(feat, True, None)

    cids = list(selected.keys())
    print(f"\nselected {len(cids)} unique glbs", file=sys.stderr)
    print("coverage per feature in selection:", file=sys.stderr)
    for f in FEATURES:
        print(f"  {f:14s} {feat_count[f]}", file=sys.stderr)

    entity_for_glb = {h: rep[h][0] for h in cids}
    type_for_glb = {h: rep[h][1] for h in cids}
    entities = {}  # entity_id -> (entity_type, feats)
    for glb in cids:
        eid = entity_for_glb[glb]
        et = type_for_glb[glb]
        prev = entities.get(eid)
        merged = set(selected[glb]) | (prev[1] if prev else set())
        entities[eid] = (et, merged)

    out_txt = sys.argv[3] if len(sys.argv) > 3 else "/tmp/blindspot_queue.txt"
    out_json = out_txt.replace(".txt", "_detail.json")

    eid_list = sorted(entities.keys())
    with open(out_txt, "w") as f:
        for eid in eid_list:
            f.write(eid + "\n")

    detail = {
        "_purpose": "blind-spot oversampled CID list for future Unity reference run",
        "_count": len(eid_list),
        "_per_feature_target": PER_FEATURE,
        "_feature_coverage": {f: feat_count[f] for f in FEATURES},
        "_by_type": {},
        "entities": [],
    }
    type_counter = defaultdict(int)
    for eid in eid_list:
        et, feats = entities[eid]
        type_counter[et] += 1
        detail["entities"].append({
            "entity_id": eid,
            "entity_type": et,
            "features": sorted(feats),
        })
    detail["_by_type"] = dict(type_counter)
    with open(out_json, "w") as f:
        json.dump(detail, f, indent=2)

    print(f"\nwrote {out_txt} ({len(eid_list)} entities)", file=sys.stderr)
    print(f"wrote {out_json}", file=sys.stderr)
    print(f"by type: {dict(type_counter)}", file=sys.stderr)


if __name__ == "__main__":
    main()
