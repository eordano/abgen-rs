"""Build an abgen-corpus manifest by scanning the Unity reference output dir.

Reads entity_type from a pre-built entities JSON (abgen-rs/tests/corpora/*).
For each <entity>/<cid>_<platform> bundle written by the converter, derives
the build flags (source_file, model_referenced, metadata_deps) the same way
the old dev/build_corpus_for_verify.py did.

Usage:
  python3 build_abgen_manifest.py <ref-dir> <entities-json> <out-manifest> [platform]
"""
import sys, os, glob, hashlib, json

CONTENT_ROOT = os.environ.get("ABGEN_CONTENT_ROOT", "./content")
REF_DIR = sys.argv[1]
ENTITIES_PATH = sys.argv[2]
OUT_PATH = sys.argv[3]
PLATFORM = sys.argv[4] if len(sys.argv) > 4 else "windows"
IMAGE_EXTS = (".png", ".jpg", ".jpeg")


def cp(cid):
    return os.path.join(CONTENT_ROOT, hashlib.sha1(cid.encode()).hexdigest()[:4], cid)


def load_entity(eid):
    p = cp(eid)
    if not os.path.exists(p): return None
    try: return json.loads(open(p, "rb").read())
    except Exception: return None


def cid_from_bundle_name(name):
    base = name[:-len(f"_{PLATFORM}")] if name.endswith(f"_{PLATFORM}") else name
    return base.split("_", 1)[0]


def file_extension(file):
    f = file.lower()
    for e in (".gltf", ".glb", ".png", ".jpg", ".jpeg"):
        if f.endswith(e): return e
    return ""


def extract_gltf_json(data, ext):
    if ext == ".gltf": return data
    if len(data) < 20 or data[:4] != b"glTF":
        raise ValueError("not glb")
    json_len = int.from_bytes(data[12:16], "little")
    return data[20:20 + json_len]


def resolve_uri(uri, glb_file):
    if uri.startswith(("data:", "http://", "https://")):
        raise ValueError("non-local")
    base_dir = os.path.dirname(glb_file)
    return os.path.normpath(os.path.join(base_dir, uri)) if base_dir else uri


def parse_gltf_dep_refs(data, ext):
    try:
        raw = extract_gltf_json(data, ext)
        doc = json.loads(raw)
    except Exception:
        return []
    out = []
    for img in (doc.get("images") or []):
        if not isinstance(img, dict): continue
        if "bufferView" in img: continue
        uri = img.get("uri")
        if uri and not uri.startswith("data:"):
            out.append(uri)
    return out


def metadata_deps_for_glb(glb_bytes, glb_file, content_by_file):
    try:
        ext = file_extension(glb_file)
        raw = extract_gltf_json(glb_bytes, ext)
        doc = json.loads(raw)
    except Exception:
        return []
    if not isinstance(doc, dict): return []
    out, seen = [], set()
    for img in (doc.get("images") or []):
        if not isinstance(img, dict): continue
        if "bufferView" in img: continue
        uri = img.get("uri")
        if not uri or uri.startswith("data:"): continue
        try: resolved = resolve_uri(uri, glb_file)
        except Exception: continue
        h = content_by_file.get(resolved.lower())
        if not h: continue
        name = f"{h}_{PLATFORM}"
        if name in seen: continue
        seen.add(name); out.append(name)
    return out


def collect_model_referenced_hashes(content_by_file):
    refs = set()
    for f, h in content_by_file.items():
        fl = f.lower()
        if not (fl.endswith(".glb") or fl.endswith(".gltf")): continue
        gp = cp(h)
        if not os.path.exists(gp): continue
        try:
            data = open(gp, "rb").read()
            ext = file_extension(f)
            uris = parse_gltf_dep_refs(data, ext)
        except Exception: continue
        per_glb = set()
        ok = True
        for uri in uris:
            try: resolved = resolve_uri(uri, f)
            except Exception: ok = False; break
            h2 = content_by_file.get(resolved.lower())
            if h2 is None: ok = False; break
            per_glb.add(h2)
        if ok: refs |= per_glb
    return refs


def main():
    ent_lookup = {e["entity_id"]: e for e in json.load(open(ENTITIES_PATH))["entities"]}
    out = {"content_dir": CONTENT_ROOT, "entities": []}
    bundles_seen = 0
    for ent_dir in sorted(glob.glob(f"{REF_DIR}/*/")):
        ent_id = os.path.basename(ent_dir.rstrip("/"))
        ent = load_entity(ent_id)
        if not ent:
            print(f"skip {ent_id}: no entity file", file=sys.stderr); continue
        entity_type = ent_lookup.get(ent_id, {}).get("entity_type", "scene")
        content_by_file = {c["file"].lower(): c["hash"] for c in ent.get("content", [])}
        inv = {}
        for f, h in content_by_file.items():
            inv.setdefault(h, f)
        model_refs = collect_model_referenced_hashes(content_by_file)
        bundles = []
        for b in sorted(glob.glob(f"{ent_dir}*_{PLATFORM}")):
            name = os.path.basename(b)
            cid = cid_from_bundle_name(name)
            glb_path = cp(cid)
            if not os.path.exists(glb_path): continue
            glb_file = inv.get(cid, f"{cid}.glb")
            try:
                gb = open(glb_path, "rb").read()
                m_deps = metadata_deps_for_glb(gb, glb_file, content_by_file)
            except Exception:
                m_deps = []
            is_image = glb_file.lower().endswith(IMAGE_EXTS)
            model_ref = is_image and cid in model_refs
            source_file = None
            if inv.get(cid) and (glb_file.lower().endswith(".gltf") or glb_file.lower().endswith("_emote.glb") or glb_file.lower().endswith(".glb")):
                source_file = glb_file
            elif is_image:
                source_file = glb_file
            spec = {"cid": cid, "bundle_name": name}
            if source_file: spec["source_file"] = source_file
            if entity_type and entity_type != "scene": spec["entity_type"] = entity_type
            if m_deps: spec["metadata_deps"] = m_deps
            if model_ref: spec["model_referenced"] = True
            bundles.append(spec)
        if bundles:
            out["entities"].append({
                "entity_id": ent_id,
                "content": [{"file": c["file"], "hash": c["hash"]} for c in ent.get("content", [])],
                "bundles": bundles,
            })
            bundles_seen += len(bundles)
    json.dump(out, open(OUT_PATH, "w"), indent=2)
    print(f"manifest: {len(out['entities'])} entities, {bundles_seen} bundles -> {OUT_PATH}")


if __name__ == "__main__":
    main()
