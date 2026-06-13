#!/usr/bin/env python3
"""Reconstruct abgen's (== best-known Unity) TOS insertion order for a probe's
clip-name set, then test whether the observed m_TOS order is a STABLE sort of
that insertion order by some bucket function. Within-bucket == insertion order
is already proven by o_fwd/o_rev."""
import zlib, os
M = 0xffffffff
def crc(s): return zlib.crc32(s.encode()) & M
def dcrc(k): return zlib.crc32((k & M).to_bytes(4, "little")) & M
def murmur(h):
    h ^= h >> 16; h = (h*0x85ebca6b)&M; h ^= h>>13; h=(h*0xc2b2ae35)&M; h^=h>>16; return h
def fib(k): return (k*2654435769)&M

LAYER = "Base Layer"

def insertion_names(clips):
    """abgen build order. Returns list of names in insertion order, deduped by crc."""
    seq = ["", "Loop", "GravityWeight"]
    seq += list(clips)  # clip bare names (as triggers)
    for nm in clips:
        n0 = nm.replace(".", "_")
        n1 = (nm + " 0") if n0 == nm else n0
        f0 = LAYER + "." + n0
        f1 = LAYER + "." + n1
        # order within transition()/state() calls per source:
        # t01: name0->name1 (name, full), t10: name1->name0 (name, full),
        # state0: name0, full0 ; state1: name1, full1 ; any: AnyState->name0, Entry->full0
        seq += [
            n0 + " -> " + n1, f0 + " -> " + f1,
            n1 + " -> " + n0, f1 + " -> " + f0,
            n0, f0,
            n1, f1,
            "AnyState -> " + n0, "Entry -> " + f0,
        ]
    seq += [LAYER]
    seen = set(); out = []
    for nm in seq:
        k = crc(nm)
        if k not in seen:
            seen.add(k); out.append((k, nm))
    return out

def loadrows(f):
    R = []
    for line in open(f):
        line = line.rstrip("\n")
        if "\t" in line:
            h, n = line.split("\t", 1); R.append((int(h), n))
    return R

# map probe -> clip names
META = {}
for line in open("/tmp/tosbuild/store/map.tsv"):
    p, nclip, names, ent, glb = line.rstrip("\n").split("\t")
    META[p] = names.split(",")

HF = {"ident": lambda k: k, "dcrc": dcrc, "murmur": murmur, "fib": fib}

def stable_bucket(ins, hf, N, top=False, sb=0, rev=False):
    def b(k):
        v = hf(k)
        return (v >> (32 - sb)) if top else (v % N)
    keyed = list(enumerate(ins))
    # stable sort by bucket
    keyed.sort(key=lambda t: ((N-1-b(t[1][0])) if rev else b(t[1][0]), t[0]))
    return [k for _, (k, _) in keyed]

files = [f for f in sorted(os.listdir("/tmp/tosbuild/tos")) if f.endswith(".tsv")]
print("=== stable-bucket-sort of abgen insertion order vs observed ===")
for hn, hf in HF.items():
    hits = []
    for f in files:
        p = f[:-4]
        if p not in META: continue
        obs = [h for h, _ in loadrows("/tmp/tosbuild/tos/" + f)]
        ins = insertion_names(META[p])
        if set(k for k, _ in ins) != set(obs):
            # insertion reconstruction mismatch — note it
            continue
        ok = False
        for sb in range(3, 14):
            N = 1 << sb
            if N < len(obs): continue
            for top in (False, True):
                for rev in (False, True):
                    if stable_bucket(ins, hf, N, top, sb, rev) == obs:
                        ok = True; hits.append((p, hn, N, top, rev)); break
                if ok: break
            if ok: break
    print("  %-6s: %d/%d  %s" % (hn, len(hits), len(files), hits[:6]))

# sanity: does insertion reconstruction cover the observed key set?
print("\n=== insertion-set coverage ===")
for f in files:
    p = f[:-4]
    if p not in META: continue
    obs = set(h for h, _ in loadrows("/tmp/tosbuild/tos/" + f))
    ins = set(k for k, _ in insertion_names(META[p]))
    print("  %-6s obs=%d ins=%d match=%s extra_ins=%d missing=%d" %
          (p, len(obs), len(ins), obs == ins, len(ins - obs), len(obs - ins)))
