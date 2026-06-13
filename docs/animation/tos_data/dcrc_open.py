import os, binascii, sys
M=0xffffffff
def load(f):
    rows=[]
    for line in open(f):
        line=line.rstrip("\n")
        if "\t" not in line: continue
        h,n=line.split("\t",1)
        rows.append((int(h),n))
    return rows
SETS={}
for d in ["/tmp/tos-ref","/tmp/tos-ord"]:
    for f in sorted(os.listdir(d)):
        if f.endswith(".tsv"): SETS[os.path.basename(f)]=load(d+"/"+f)

def dcrc(h): return binascii.crc32((h&M).to_bytes(4,'little'))&M

# Open-addressing FEASIBILITY: given home = dcrc(key)%N, does there exist an insertion order
# such that linear probing yields the observed slot order?
# Feasibility test: observed order o[0..k-1]. Assign them to increasing slots (with one wrap allowed).
# For a valid linear-probe arrangement of a SET (regardless of insertion order), a necessary&sufficient
# condition (Robin Hood / any order) is NOT simple. Instead test the standard "parking function" feasibility:
# there's an arrangement iff for the multiset of homes, sorted by slot, each prefix [0..s] has
# (#homes <= s, counting wrap) >= (#occupied <= s). Simpler: just try to PLACE in observed order greedily:
# place o[j] at the first free slot >= home(o[j]); the resulting occupied-slot order must equal observed order.
# But insertion order is free. Equivalent: observed is feasible iff we can pick slots s_0<s_1<...<s_{k-1}
# (mod N, <=1 wrap) with home(o[j]) reachable, i.e. there's a system-of-distinct-representatives.
# We'll just test: is observed sequence == sort by (home, then ???). Try home as primary key, tie-break by
# observed-order-stable. If sort by home (stable on observed) == observed, it's bucket-grouped feasible.
def fits_bucket(rows,N):
    o=[h for h,_ in rows]
    homes=[dcrc(h)%N for h in o]
    # stable sort by home
    idx=sorted(range(len(o)),key=lambda i:homes[i])
    return [o[i] for i in idx]==o, homes

# Find per-set N (any) where bucket-grouped (stable) reproduces, allow N up to 8192
print("double-crc bucket-grouped (stable, home=dcrc%N), best N per set:")
allfit=True
fitN={}
for f,rows in sorted(SETS.items()):
    cnt=len(rows); found=None
    for sb in range(2,16):
        N=1<<sb
        if N<cnt: continue
        ok,_=fits_bucket(rows,N)
        if ok: found=N; break
    # also try non-pow2 N
    if found is None:
        for N in range(cnt, cnt*6+50):
            ok,_=fits_bucket(rows,N)
            if ok: found=N; break
    fitN[f]=found
    if found is None: allfit=False
    print("%-26s cnt=%2d  N=%s"%(f,cnt,found))
print("ALL FIT:",allfit)
