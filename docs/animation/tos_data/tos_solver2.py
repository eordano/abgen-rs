#!/usr/bin/env python3
"""m_TOS ordering — second-generation falsification harness (2026-06-12).

Extends tos_solver.py with the decisive negatives found while attacking the
o_fwd/o_rev positive evidence (only 5 of 44 positions swap when glb clip order
is reversed). The earlier prose read those 5 swaps as "within-collision-group
insertion order". This harness shows that reading is UNSUPPORTABLE by any
standard hash: no home function even co-locates the swapped pairs.

Run: python3 tos_solver2.py
Inputs: probes2/*.tsv (count/collision/glb-order probes with KNOWN insertion
order) + emote*.tsv + probe-tos-*.tsv. The k-clip probes' insertion order is the
glb forward clip order, pinned by c04 == o_fwd (byte-identical, see [0]).

STATUS: UNSOLVED. Every offline container/hash family is now falsified jointly
over all 46 orderings. The blocker is recorded precisely so a future adaptive
Unity probe can target the one remaining unknown (the engine placement
function), not re-derive these negatives.
"""
import os, zlib
M = 0xFFFFFFFF
HERE = os.path.dirname(os.path.abspath(__file__))
P2 = os.path.join(HERE, "probes2")


def crc(s):
    return zlib.crc32(s.encode()) & M


def load(f):
    rows = []
    for line in open(f):
        line = line.rstrip("\n")
        if "\t" in line:
            h, n = line.split("\t", 1)
            rows.append((int(h), n))
    return rows


def dcrc(k):
    return zlib.crc32((k & M).to_bytes(4, "little")) & M


def fib(k):
    return (k * 2654435769) & M


def murmur(h):
    h ^= h >> 16
    h = (h * 0x85EBCA6B) & M
    h ^= h >> 13
    h = (h * 0xC2B2AE35) & M
    h ^= h >> 16
    return h


def revbits(x, w=32):
    r = 0
    for _ in range(w):
        r = (r << 1) | (x & 1)
        x >>= 1
    return r


def byteswap(k):
    return int.from_bytes(k.to_bytes(4, "little"), "big")


HF = {"ident": lambda k: k, "dcrc": dcrc, "fib": fib,
      "murmur": murmur, "revbits": revbits, "byteswap": byteswap}


def insnames(clips):
    """abgen == glb-forward insertion order (== Unity's, confirmed by c04==o_fwd)."""
    seq = ["", "Loop", "GravityWeight"] + list(clips)
    for nm in clips:
        n0 = nm
        n1 = nm + " 0"
        f0 = "Base Layer." + n0
        f1 = "Base Layer." + n1
        seq += [n0 + " -> " + n1, f0 + " -> " + f1, n1 + " -> " + n0,
                f1 + " -> " + f0, n0, f0, n1, f1,
                "AnyState -> " + n0, "Entry -> " + f0]
    seq += ["Base Layer"]
    seen = set()
    out = []
    for nm in seq:
        k = crc(nm)
        if k not in seen:
            seen.add(k)
            out.append(k)
    return out


def run():
    of = [h for h, _ in load(os.path.join(P2, "o_fwd.tsv"))]
    orr = [h for h, _ in load(os.path.join(P2, "o_rev.tsv"))]
    c04 = [h for h, _ in load(os.path.join(P2, "c04.tsv"))]
    ins_f = insnames(["k00", "k01", "k02", "k03"])
    ins_r = insnames(["k03", "k02", "k01", "k00"])
    rfwd = {k: i for i, k in enumerate(ins_f)}
    rrev = {k: i for i, k in enumerate(ins_r)}

    print("[0] insertion order is pinned: c04 (4 clips, fwd glb) == o_fwd:",
          c04 == of, " -> glb-forward clip order IS the build/insertion order.")

    moved = [k for k in of if of.index(k) != orr.index(k)]
    print("\n[1] o_fwd vs o_rev: %d/%d keys change position." % (len(moved), len(of)))
    gA = [419095881, 1614764057]
    gB = [1310708297, 493921129, 1302005287]
    print("    groupA(pos0-1)=%s  groupB(pos6-8)=%s" % (gA, gB))

    print("\n[2] NO home function co-locates the swapped pairs (necessary for any")
    print("    chaining/probe 'within-bucket insertion order' reading):")
    pairs_needed = [(gA[0], gA[1])] + [(gB[i], gB[j])
                                       for i in range(3) for j in range(i + 1, 3)]
    found = 0
    for hn, hf in HF.items():
        for mode in ("mod", "top"):
            for b in range(2, 18):
                N = 1 << b
                if mode == "mod":
                    lab = {k: hf(k) % N for k in of}
                else:
                    lab = {k: hf(k) >> (32 - b) for k in of}
                if all(lab[a] == lab[c] for a, c in pairs_needed):
                    found += 1
    print("    home functions co-locating ALL swapped pairs: %d  (0 => the"
          " 'within-bucket' model is unsupported)" % found)

    print("\n[3] NO stable sort by any lossy key f(hash), ties=insertion, fits")
    print("    BOTH o_fwd and o_rev:")

    def stable_ok(g):
        return (sorted(of, key=lambda k: (g(k), rfwd[k])) == of and
                sorted(orr, key=lambda k: (g(k), rrev[k])) == orr)

    hits = 0
    for hn, hf in HF.items():
        for sh in range(0, 32):
            if stable_ok(lambda k, hf=hf, sh=sh: hf(k) >> sh):
                hits += 1
            if stable_ok(lambda k, hf=hf, sh=sh: hf(k) & (M >> sh)):
                hits += 1
        for Mod in range(2, 1 << 14):
            if stable_ok(lambda k, hf=hf, Mod=Mod: hf(k) % Mod):
                hits += 1
                break
    print("    stable-sort candidates (radix/mask/mod over 6 hashes): %d" % hits)

    print("\n[4] NO open-addressing slot-read: min home-monotonicity wraps over")
    print("    ALL capacities n..8192 (open-addressing needs <=1):")
    for fn in ("o_fwd.tsv", "c05.tsv", "c16.tsv"):
        ks = [h for h, _ in load(os.path.join(P2, fn))]
        n = len(ks)
        best = (10 ** 9, "")
        for hn, hf in HF.items():
            for N in range(n, 8192):
                hv = [hf(k) % N for k in ks]
                w = sum(1 for i in range(1, n) if hv[i] < hv[i - 1])
                if w < best[0]:
                    best = (w, "%s/N=%d" % (hn, N))
                if w == 0:
                    break
        print("    %-9s n=%-3d min-wraps=%d (%s)" % (fn, n, best[0], best[1]))

    print("\n[5] NO chaining (FIFO/LIFO, mod/top, both bucket directions) and NO")
    print("    open-addressing (lin/quad, all caps) reproduces o_fwd from the")
    print("    pinned insertion order:")

    def chain(ins, N, hf, head, top, brev):
        bk = [[] for _ in range(N)]
        for k in ins:
            b = (hf(k) >> (32 - N.bit_length() + 1)) % N if top else hf(k) % N
            (bk[b].insert(0, k) if head else bk[b].append(k))
        rng = range(N - 1, -1, -1) if brev else range(N)
        out = []
        for b in rng:
            out += bk[b]
        return out

    def lin(ins, N, hf, quad):
        slots = [None] * N
        for k in ins:
            h = hf(k) % N
            d = 0
            p = h
            while slots[p] is not None:
                d += 1
                p = (h + (d * d if quad else d)) % N
                if d > 2 * N:
                    return None
            slots[p] = k
        return [s for s in slots if s is not None]

    cm = om = 0
    for hn, hf in HF.items():
        for N in range(len(of), 2048):
            for head in (False, True):
                for top in (False, True):
                    for brev in (False, True):
                        if chain(ins_f, N, hf, head, top, brev) == of:
                            cm += 1
        for N in range(len(of), 512):
            for quad in (False, True):
                if lin(ins_f, N, hf, quad) == of:
                    om += 1
    print("    chaining hits: %d   open-addressing hits: %d" % (cm, om))

    print("\nCONCLUSION: the placement is not a sort, not chaining, not open")
    print("addressing, and standard hashes do not even co-locate the keys whose")
    print("order is insertion-sensitive. The remaining unknown is engine-internal")
    print("and only an adaptive Unity probe (controlled CRC keys) can read it off.")


if __name__ == "__main__":
    run()
