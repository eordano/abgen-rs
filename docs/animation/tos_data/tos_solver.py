#!/usr/bin/env python3
"""Consolidated m_TOS ordering falsification harness.

Run: python3 tos_solver.py
Inputs: the *.tsv files in this directory (emoteNN.tsv = the 21 reference
glb-emote AnimatorController m_TOS tables, in serialized order; probe-tos-*.tsv =
controlled-name probe emotes run through the real Unity converter).

Each .tsv line is "<crc32_key>\t<name>" in the order Unity serialized m_TOS.
The key is Mecanim CRC32 (== zlib crc32) of the UTF-8 name; '' -> 0,
'Loop' -> 23966416, 'Base Layer' -> 756556552, 'GravityWeight' -> 2105523844.

The goal: find a deterministic function of the NAME SET that reproduces the
observed serialization order for all 21 reference tables simultaneously.

STATUS: UNSOLVED. This harness records the negative results. See
../emote_animator_tos_order.md for the prose summary. The decisive open
unknown is Unity's internal INSERTION ORDER during AnimatorController build
(native, recoverable only via controlled probes), compounded with an
unidentified container/hash. Every standard (hash, table-size/growth, probe,
within-bucket-order, iteration-direction) combination below is ruled out.
"""
import os, binascii
from collections import defaultdict
M = 0xffffffff
HERE = os.path.dirname(os.path.abspath(__file__))


def crc(s):  # Mecanim name hash
    return binascii.crc32(s.encode()) & M


def load(f):
    rows = []
    for line in open(f):
        line = line.rstrip("\n")
        if "\t" not in line:
            continue
        h, n = line.split("\t", 1)
        rows.append((int(h), n))
    return rows


EM = {f: load(os.path.join(HERE, f)) for f in sorted(os.listdir(HERE))
      if f.startswith("emote") and f.endswith(".tsv")}
PROBE = {f: load(os.path.join(HERE, f)) for f in sorted(os.listdir(HERE))
         if f.startswith("probe-tos-") and f.endswith(".tsv")}

# 2026-06-11 controlled clip-count + crc-collision + glb-order probes (probes2/).
# cNN = NN identical-content clips (names k00..); x16/x32 = 4 clip names whose
# crc collides mod 16 / mod 32; o_fwd/o_rev = same 4-clip name set in forward /
# reversed GLB animation order. See probes2/map.tsv + ../emote_animator_tos_order.md.
P2DIR = os.path.join(HERE, "probes2")
PROBE2 = {}
if os.path.isdir(P2DIR):
    PROBE2 = {"p2_" + f: load(os.path.join(P2DIR, f))
              for f in sorted(os.listdir(P2DIR))
              if f.endswith(".tsv") and f != "map.tsv"}


def ident(k): return k
def dcrc(k): return binascii.crc32((k & M).to_bytes(4, "little")) & M
def murmur(h):
    h ^= h >> 16; h = (h * 0x85ebca6b) & M; h ^= h >> 13
    h = (h * 0xc2b2ae35) & M; h ^= h >> 16; return h


def abgen_insertion(rows):
    """Best guess at Unity's TOS insertion order (== abgen's build order).
    KNOWN to be wrong for Unity, but the closest reconstruction available."""
    names = set(n for _, n in rows); layer = "Base Layer"
    clips = []
    for _, n in rows:
        if n in ("", "Loop", "GravityWeight", layer):
            continue
        if ("->" in n or n.startswith("AnyState") or n.startswith("Entry")
                or n.startswith("Base Layer") or n.endswith(" 0")):
            continue
        if layer + "." + n in names:
            clips.append(n)
    seen = set(); cl = [c for c in clips if not (c in seen or seen.add(c))]
    seq = ["", "Loop", "GravityWeight"] + cl
    for clip in cl:
        n0 = clip.replace(".", "_"); n1 = (clip + " 0") if n0 == clip else n0
        f0 = layer + "." + n0; f1 = layer + "." + n1
        seq += [n0 + " -> " + n1, f0 + " -> " + f1, n1 + " -> " + n0,
                f1 + " -> " + f0, n0, f0, n1, f1,
                "AnyState -> " + n0, "Entry -> " + f0]
    seq += [layer]
    seen = set(); out = []
    for n in seq:
        k = crc(n)
        if k not in seen:
            seen.add(k); out.append(k)
    return out


def lin_layout(ins, N, homef):
    slots = [None] * N
    for k in ins:
        p = homef(k) % N; d = 0
        while slots[p] is not None:
            p = (p + 1) % N; d += 1
            if d > N:
                return None
        slots[p] = k
    return [s for s in slots if s is not None]


def run():
    ALL = {**EM, **PROBE, **PROBE2}
    print("tables: %d reference + %d name-probe + %d count/collision-probe\n"
          % (len(EM), len(PROBE), len(PROBE2)))

    print("[1] sort-by-(key mod N) asc, and cyclic-rotation-of-that:")
    for N in (16, 32, 64, 128, 256, 1024):
        a = rot = 0; tot = 0
        for r in ALL.values():
            if len(r) > N:
                continue
            tot += 1
            obs = [h for h, _ in r]
            srt = sorted(obs, key=lambda k: k % N)
            if obs == srt:
                a += 1
            dbl = srt + srt
            if any(dbl[i:i + len(obs)] == obs for i in range(len(srt))):
                rot += 1
        print("    N=%-5d exact:%d/%d  cyclic-rotation:%d/%d" % (N, a, tot, rot, tot))

    print("\n[2] linear-probe, abgen insertion order, home in {ident,dcrc,murmur}:")
    for hn, hf in (("ident", ident), ("dcrc", dcrc), ("murmur", murmur)):
        best = 0
        for N in (16, 32, 64, 128, 256, 512, 1024):
            h = 0
            for r in ALL.values():
                ins = abgen_insertion(r)
                if set(ins) != set(k for k, _ in r) or len(ins) > N:
                    continue
                if lin_layout(ins, N, hf) == [k for k, _ in r]:
                    h += 1
            best = max(best, h)
        print("    home=%-7s best: %d/%d" % (hn, best, len(ALL)))

    print("\n[3] bucket-monotone violations on the 114-key n=14 ground truth"
          " (0=exact, ~57=chance):")
    nodes = set(); adj = defaultdict(set)
    for r in (v for v in EM.values() if len(v) == 14):
        ks = [h for h, _ in r]; nodes.update(ks)
        for i in range(len(ks) - 1):
            adj[ks[i]].add(ks[i + 1])
    indeg = defaultdict(int)
    for a in adj:
        for b in adj[a]:
            indeg[b] += 1
    q = sorted(n for n in nodes if indeg[n] == 0); order = []
    while q:
        q.sort(); n = q.pop(0); order.append(n)
        for b in sorted(adj[n]):
            indeg[b] -= 1
            if indeg[b] == 0:
                q.append(b)
    for hn, hf in (("ident", ident), ("dcrc", dcrc), ("murmur", murmur)):
        bestv = 10 ** 9
        for sb in range(7, 16):
            N = 1 << sb
            b = [hf(k) % N for k in order]
            bestv = min(bestv, sum(1 for i in range(1, len(b)) if b[i] < b[i - 1]))
        print("    home=%-7s min violations over N: %d / %d" % (hn, bestv, len(order) - 1))

    # [4] CONTROLLED-PROBE disproofs (2026-06-11). The count/collision/glb-order
    # probes in probes2/ let us falsify the container models far more sharply
    # than the corpus alone:
    if PROBE2:
        print("\n[4] glb-storage-order sensitivity (o_fwd vs o_rev, same name set):")
        of = PROBE2.get("p2_o_fwd.tsv"); orr = PROBE2.get("p2_o_rev.tsv")
        if of and orr:
            ofk = [h for h, _ in of]; ork = [h for h, _ in orr]
            diff = [i for i in range(len(ofk)) if ofk[i] != ork[i]]
            print("    %d/%d positions differ -> insertion order matters ONLY"
                  " within collision groups (chaining/probe within-bucket =="
                  " insertion order)." % (len(diff), len(ofk)))

        print("\n[5] open-addressing slot-read parking test (home-monotonicity"
              " wraps; valid linear-probe read needs <=1):")
        for hn, hf in (("ident", ident), ("dcrc", dcrc), ("murmur", murmur)):
            worst = 0
            for r in PROBE2.values():
                ks = [h for h, _ in r]; n = len(ks)
                mn = 10 ** 9
                for sb in range(4, 13):
                    N = 1 << sb
                    if N < n: continue
                    hm = [hf(k) % N for k in ks]
                    mn = min(mn, sum(1 for i in range(1, n) if hm[i] < hm[i - 1]))
                worst = max(worst, mn)
            print("    home=%-7s max min-wraps over probes: %d (>>1 => NOT"
                  " open-addressing)" % (hn, worst))

        print("\n[6] chaining contiguity test (key%%N buckets must form"
              " contiguous runs):")
        for N in (16, 32, 64):
            feasible = 0
            for r in PROBE2.values():
                ks = [h for h, _ in r]
                pos = defaultdict(list)
                for i, k in enumerate(ks):
                    pos[k % N].append(i)
                ok = all(ps == list(range(ps[0], ps[0] + len(ps)))
                         for ps in pos.values())
                feasible += ok
            print("    N=%-4d contiguous-chaining-feasible: %d/%d"
                  % (N, feasible, len(PROBE2)))


if __name__ == "__main__":
    run()
