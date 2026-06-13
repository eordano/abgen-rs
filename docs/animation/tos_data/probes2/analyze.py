#!/usr/bin/env python3
"""Analyze fresh TOS count/collision probes against the insertion-order/container
question. Reads /tmp/tosbuild/tos/<probe>.tsv (dump_tos output) + map.tsv."""
import os, zlib, struct
from collections import defaultdict, OrderedDict
M = 0xffffffff
TOS = "/tmp/tosbuild/tos"
MAP = "/tmp/tosbuild/store/map.tsv"

def crc(s): return zlib.crc32(s.encode()) & M
def dcrc(k): return zlib.crc32((k & M).to_bytes(4, "little")) & M

def load(f):
    rows = []
    for line in open(f):
        line = line.rstrip("\n")
        if "\t" not in line: continue
        h, n = line.split("\t", 1)
        rows.append((int(h), n))
    return rows

probes = OrderedDict()
meta = {}
for line in open(MAP):
    p, nclip, names, ent, glb = line.rstrip("\n").split("\t")
    meta[p] = (int(nclip), names.split(","))
    f = os.path.join(TOS, p + ".tsv")
    if os.path.exists(f):
        probes[p] = load(f)

print("=== loaded probes ===")
for p, rows in probes.items():
    print("  %-7s nclip=%d keys=%d" % (p, meta[p][0], len(rows)))

# (1) Count-vs-keys: confirm growth steps. Print table size vs nclip.
print("\n=== [1] table size vs clip count ===")
for p in sorted(probes, key=lambda x: meta[x][0]):
    if p.startswith("c"):
        print("  nclip=%-3d keys=%-4d  probe=%s" % (meta[p][0], len(probes[p]), p))

# (2) Collision probes: print the ORDER of the colliding keys and their full crc
print("\n=== [2] collision probe within-bucket order ===")
for p in ("x16", "x32"):
    if p not in probes: continue
    rows = probes[p]
    nclip, names = meta[p]
    print("  probe %s names=%s" % (p, names))
    # position of each colliding clip-name key in the serialized order
    order = [n for _, n in rows]
    for nm in names:
        # the bare clip name key
        if nm in order:
            print("    %-4s crc=%d  serial_pos=%d" % (nm, crc(nm), order.index(nm)))
    print("    full order:")
    for i, (h, n) in enumerate(rows):
        print("      %2d  %10d  %s" % (i, h, n))

# (3) o_fwd vs o_rev: same name set, different glb storage order. Are the
#     m_TOS orders identical? If yes -> order is name-derived, NOT glb-order.
print("\n=== [3] glb-storage-order independence (o_fwd vs o_rev) ===")
if "o_fwd" in probes and "o_rev" in probes:
    of = [h for h, _ in probes["o_fwd"]]
    orr = [h for h, _ in probes["o_rev"]]
    print("  o_fwd == o_rev (key order): %s" % (of == orr))
    if of != orr:
        print("  o_fwd:", of)
        print("  o_rev:", orr)

# (4) Try sort-by-(key mod N) and dcrc-mod-N on each probe table
print("\n=== [4] per-probe bucket-sort check ===")
for p, rows in probes.items():
    obs = [h for h, _ in rows]
    n = len(obs)
    hit = []
    for N in (16, 32, 64, 128, 256, 512):
        if n > N:
            srt = sorted(obs, key=lambda k: k % N)
            if obs == srt: hit.append("ident%%%d" % N)
            srtd = sorted(obs, key=lambda k: dcrc(k) % N)
            if obs == srtd: hit.append("dcrc%%%d" % N)
    print("  %-7s %s" % (p, hit or "none"))
