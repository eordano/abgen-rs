#!/usr/bin/env python3
"""Build controlled-clip-count TOS probe emotes from a single-clip source GLB.

For each probe we duplicate the source animation block N times under distinct
names, recompute a self-consistent raw-CIDv1 hash, and emit an emote entity
that references the multi-clip GLB. Output: a flat content store (contents/)
plus a queue.txt of entity hashes and map.tsv (probe-name -> entity, glb).
"""
import struct, json, hashlib, base64, os, sys, zlib

SRC_GLB = "/tmp/emoteprobe/store/contents/bafkreip7v2xru7aydzr2yh3d5ozztyw2g7pj56ny3fkuqbome4ugs4usua"
SRC_ENT = "/tmp/emoteprobe/store/contents/bafkreidhta5harqydaidd7hl5h7sba6cqyltcur75wvu7fcx6sonslkmbq"
OUT = "/tmp/tosbuild/store"
CONTENTS = os.path.join(OUT, "contents")
os.makedirs(CONTENTS, exist_ok=True)

def raw_cid(data):
    h = hashlib.sha256(data).digest()
    pref = b"\x01\x55\x12\x20" + h
    return "b" + base64.b32encode(pref).decode().lower().rstrip("=")

def load_glb(path):
    d = open(path, "rb").read()
    assert d[:4] == b"glTF"
    ver, total = struct.unpack("<II", d[4:12])
    off = 12
    json_chunk = None
    bin_chunk = None
    while off < len(d):
        clen, ctype = struct.unpack("<II", d[off:off+8])
        body = d[off+8:off+8+clen]
        if ctype == 0x4E4F534A:  # JSON
            json_chunk = body
        elif ctype == 0x004E4942:  # BIN
            bin_chunk = body
        off += 8 + clen
    return json.loads(json_chunk), bin_chunk

def write_glb(j, binc):
    js = json.dumps(j, separators=(",", ":")).encode("utf-8")
    while len(js) % 4 != 0:
        js += b" "
    chunks = struct.pack("<II", len(js), 0x4E4F534A) + js
    if binc is not None:
        pad = (-len(binc)) % 4
        b = binc + b"\x00" * pad
        chunks += struct.pack("<II", len(b), 0x004E4942) + b
    total = 12 + len(chunks)
    return b"glTF" + struct.pack("<II", 2, total) + chunks

def crc(s):
    return zlib.crc32(s.encode()) & 0xffffffff

SJ, SBIN = load_glb(SRC_GLB)
SRC_ANIM = SJ["animations"][0]

def make_probe(clip_names):
    j = json.loads(json.dumps(SJ))  # deep copy
    anims = []
    for nm in clip_names:
        a = json.loads(json.dumps(SRC_ANIM))
        a["name"] = nm
        anims.append(a)
    j["animations"] = anims
    return write_glb(j, SBIN)

ENT = json.loads(open(SRC_ENT).read())

def make_entity(glb_hash, name_tag):
    e = json.loads(json.dumps(ENT))
    male = "male/emote.glb"
    e["content"] = [
        {"file": "thumbnail.png", "hash": e["content"][0]["hash"]},
        {"file": male, "hash": glb_hash},
        {"file": "female/emote.glb", "hash": glb_hash},
        {"file": "image.png", "hash": e["content"][3]["hash"]},
    ]
    md = e["metadata"]
    md["id"] = "urn:decentraland:matic:collections-v2:0x%040x:0" % (abs(hash(name_tag)) % (16**40))
    md["name"] = "tosprobe " + name_tag
    md["emoteDataADR74"]["representations"] = [
        {"bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"],
         "mainFile": male, "contents": [male]},
        {"bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale"],
         "mainFile": "female/emote.glb", "contents": ["female/emote.glb"]},
    ]
    md["pointers"] = e["pointers"] = [md["id"]]
    return e

# ---- probe set ----
probes = {}

# (1) clip-count probes: distinct clip names, bracketing pow2 growth.
# clip names "kAA","kAB",... avoid suffix collisions, keep simple ascii.
def clipname(i):
    return "k%02d" % i

for n in [1, 2, 3, 4, 5, 8, 9, 16, 17]:
    probes["c%02d" % n] = [clipname(i) for i in range(n)]

# (2) CRC-collision probes mod 16 and mod 32 (2-clip, names whose crc collide).
# Find pairs of short names colliding mod given M.
def find_collisions(M, count, base_alpha="abcdefghijklmnopqrstuvwxyz0123456789"):
    buckets = {}
    # search 2-4 char names
    import itertools
    names = []
    for L in (2, 3):
        for tup in itertools.product(base_alpha, repeat=L):
            names.append("".join(tup))
            if len(names) > 200000:
                break
        if len(names) > 200000:
            break
    for nm in names:
        b = crc(nm) % M
        buckets.setdefault(b, []).append(nm)
    # return a bucket with >=count members
    for b, mem in buckets.items():
        if len(mem) >= count:
            return mem[:count]
    return None

col16 = find_collisions(16, 4)
col32 = find_collisions(32, 4)
if col16:
    probes["x16"] = col16          # 4 clips colliding mod 16
if col32:
    probes["x32"] = col32          # 4 clips colliding mod 32

# (3) same-name-set in different glb storage order (separates name-order from
# glb-order). Two probes: forward and reversed order of a 4-clip set.
base4 = [clipname(i) for i in range(4)]
probes["o_fwd"] = base4[:]
probes["o_rev"] = base4[::-1]

# write everything
qlines = []
maptsv = []
for pname in sorted(probes):
    names = probes[pname]
    glb = make_probe(names)
    gh = raw_cid(glb)
    open(os.path.join(CONTENTS, gh), "wb").write(glb)
    ent = make_entity(gh, pname)
    eb = json.dumps(ent).encode("utf-8")
    eh = raw_cid(eb)
    open(os.path.join(CONTENTS, eh), "wb").write(eb)
    # thumbnail/image referenced hashes must exist; copy from src store if present
    for h in (ent["content"][0]["hash"], ent["content"][3]["hash"]):
        srcp = "/tmp/emoteprobe/store/contents/" + h
        dstp = os.path.join(CONTENTS, h)
        if os.path.exists(srcp) and not os.path.exists(dstp):
            open(dstp, "wb").write(open(srcp, "rb").read())
    qlines.append(eh)
    maptsv.append("%s\t%d\t%s\t%s\t%s" % (pname, len(names), ",".join(names), eh, gh))

open(os.path.join(OUT, "queue.txt"), "w").write("\n".join(qlines) + "\n")
open(os.path.join(OUT, "map.tsv"), "w").write("\n".join(maptsv) + "\n")
print("wrote %d probes to %s" % (len(probes), OUT))
for line in maptsv:
    print(line)
