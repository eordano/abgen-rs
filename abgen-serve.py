#!/usr/bin/env python3
"""abgen-serve — expose an abgen output dir as a Decentraland asset-bundle CDN.

The zero-infrastructure serving path: a single-file HTTP server that exposes a
directory produced by abgen-corpus in the exact URL shape unity-explorer
fetches, so the client can be pointed at a 100%-abgen tree. Handles both
output layouts automatically:
  * collection (`--collection-urn`): FLAT  -> <out>/<assetHash>_<platform>
  * per-entity (default / --from-reference): NESTED -> <out>/<entityCID>/<assetHash>_<platform>

(abgen-corpus also has a third output shape, `--cdn-layout`:
<entity>/<platform>/<hash>_<platform> binaries + per-entity
<platform>.manifest.json. That is the production AB-CDN on-disk shape, served
by an ab-cdn-compatible server — use such a service for cdn-layout
trees; this script serves the flat/nested shapes above and synthesizes its
manifest responses instead of reading manifest files.)

Routes (verified against unity-explorer):
  GET  /<version>/<entityCID>/<assetHash>_<platform>   -> the bundle file
  GET  /<version>/<assetHash>_<platform>               -> bundle (flat)
  GET  /manifest/<entityCID>_<platform>.json           -> {"version","date"}  (SceneAbDto)
  POST /entities/versions   {"pointers":[...]}          -> asset-bundle-registry version map

The manifest only carries version + date; the client derives bundle URLs from the
scene/wearable's own content hashes, so the registry is optional (the client's
manifest-fallback covers it). Bundle filenames == production AB-CDN names, so an
abgen output dir drops straight in. Point the client with:
  --lsd-use-remote-ab --lsd-remote-ab-server http://127.0.0.1:<port>

For visually correct serving, build the tree with --real-textures (and
--v38-compat for production-shape bundles) — default abgen output is
byte-faithful to the parity reference and ships flat-color stubs for
oversized textures. See README.md "Build modes".

version must be `v<int>` >= 25 (sceneID-in-path layout) and >= 15/16 (min supported);
default v41.
"""
import argparse, json, os, sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = "."
VERSION = "v41"
DATE = "2024-01-01T00:00:00.000Z"  # fixed -> stable client cache key (buildDate+hash)
FLAT = False  # detected at startup: bundles live directly in ROOT (collection output)


def _bundle_path(parts):
    """parts = path after the <version> segment: [entity, file] or [file]."""
    fname = parts[-1]
    if FLAT:
        p = os.path.join(ROOT, fname)
        return p if os.path.isfile(p) else None
    if len(parts) == 2:
        p = os.path.join(ROOT, parts[0], parts[1])
        if os.path.isfile(p):
            return p
    for ent in os.listdir(ROOT):  # nested fallback: find the file under any entity dir
        p = os.path.join(ROOT, ent, fname)
        if os.path.isfile(p):
            return p
    return None


def _entity_known(entity: str) -> bool:
    # in a flat collection every requested entity is "covered"; nested -> dir must exist
    return FLAT or os.path.isdir(os.path.join(ROOT, entity))


class Handler(BaseHTTPRequestHandler):
    server_version = "abgen-serve/1.0"

    def _send(self, code, body=b"", ctype="application/octet-stream"):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def _send_file(self, path):
        try:
            with open(path, "rb") as f:
                data = f.read()
        except OSError:
            return self._send(404, b"not found", "text/plain")
        self._send(200, data, "application/octet-stream")

    def do_OPTIONS(self):
        self._send(204)

    def do_HEAD(self):
        self.do_GET()

    def do_GET(self):
        parts = self.path.split("?", 1)[0].strip("/").split("/")
        if len(parts) == 2 and parts[0] == "manifest" and parts[1].endswith(".json"):
            entity = parts[1][:-5].rsplit("_", 1)[0]  # strip ".json" then "_<platform>"
            if not _entity_known(entity):
                return self._send(404, b'{"error":"unknown entity"}', "application/json")
            return self._send(200, json.dumps({"version": VERSION, "date": DATE}).encode(),
                              "application/json")
        if len(parts) >= 2:  # <version>/<entity>/<file> or <version>/<file>
            f = _bundle_path(parts[1:])
            if f:
                return self._send_file(f)
        return self._send(404, b"not found", "text/plain")

    def do_POST(self):
        if self.path.split("?", 1)[0].strip("/").endswith("entities/versions"):
            n = int(self.headers.get("Content-Length", 0) or 0)
            try:
                pointers = json.loads(self.rfile.read(n) or b"{}").get("pointers", [])
            except json.JSONDecodeError:
                pointers = []
            ver = {"version": VERSION, "buildDate": DATE}
            resp = {"pointers": pointers,
                    "versions": {"assets": {"windows": ver, "mac": ver}},
                    "bundles": {"assets": {p: ver for p in pointers}}}
            return self._send(200, json.dumps(resp).encode(), "application/json")
        return self._send(404, b"not found", "text/plain")

    def log_message(self, fmt, *args):
        sys.stderr.write("%s - %s\n" % (self.address_string(), fmt % args))


def detect_flat(root: str) -> bool:
    # flat if bundles sit directly in root (no per-entity subdirs holding them)
    for e in os.listdir(root):
        if os.path.isfile(os.path.join(root, e)) and (e.endswith("_windows") or e.endswith("_mac")):
            return True
    return False


def main():
    global ROOT, VERSION, DATE, FLAT
    ap = argparse.ArgumentParser(description="Serve an abgen output dir as a DCL asset-bundle CDN.")
    ap.add_argument("root", help="abgen output dir (flat collection or <entity>/<asset> nested)")
    ap.add_argument("--version", default=VERSION, help="AB manifest version v<int> >=25 (default v41)")
    ap.add_argument("--port", type=int, default=5185)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--date", default=DATE, help="manifest buildDate (fixed -> stable cache)")
    ap.add_argument("--flat", action="store_true", help="force flat layout (else auto-detected)")
    a = ap.parse_args()
    ROOT, VERSION, DATE = os.path.abspath(a.root), a.version, a.date
    if not os.path.isdir(ROOT):
        sys.exit(f"root not a dir: {ROOT}")
    try:
        int(VERSION[1:])
    except ValueError:
        sys.exit(f"--version must be v<int> (got {VERSION})")
    FLAT = a.flat or detect_flat(ROOT)
    n = len(os.listdir(ROOT))
    base = f"http://{a.host}:{a.port}"
    print(f"abgen-serve: {ROOT} ({'flat collection' if FLAT else 'nested'}, {n} entries) "
          f"version={VERSION} at {base}", file=sys.stderr)
    print(f"point unity-explorer: --lsd-use-remote-ab --lsd-remote-ab-server {base}", file=sys.stderr)
    ThreadingHTTPServer((a.host, a.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
