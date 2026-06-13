import os, sys

def load(f):
    rows=[]
    for line in open(f):
        line=line.rstrip("\n")
        if "\t" not in line: continue
        h,n=line.split("\t",1)
        rows.append((int(h),n))
    return rows

EM={}
for f in sorted(os.listdir("/tmp/tos-ref")):
    if f.endswith(".tsv"):
        EM[f]=load("/tmp/tos-ref/"+f)

def slot_scan_open(N, order, probe):
    table=[None]*N
    for h in order:
        i=h%N; step=0
        while table[i] is not None:
            step+=1
            if step>N: return None
            if probe=="linear": i=(i+1)%N
            else: i=(h%N + step*step)%N
        table[i]=h
    return [x for x in table if x is not None]

def slot_scan_chain(N, order, prepend):
    # chaining: each bucket is a list. iterate bucket 0..N-1, within bucket the chain order.
    buckets=[[] for _ in range(N)]
    for h in order:
        b=h%N
        if prepend: buckets[b].insert(0,h)
        else: buckets[b].append(h)
    out=[]
    for b in buckets: out.extend(b)
    return out

def orders_for(hashes):
    return {
      "asc":sorted(hashes),
      "desc":sorted(hashes,reverse=True),
      "observed":list(hashes),
      "rev_observed":list(hashes)[::-1],
    }

# Test: does ONE (N, mode, insert-order, direction) reproduce ALL 21 orderings?
modes=[]
for N in range(8, 4097):
    modes.append(N)

def fits_all(testfn):
    for f,rows in EM.items():
        tgt=[h for h,_ in rows]
        if testfn([h for h,_ in rows], tgt) != tgt:
            return False
    return True

found=[]
# chaining models (no infinite loop)
for N in range(8,4097):
    for oname in ["asc","desc","observed","rev_observed"]:
        for prepend in [False,True]:
            ok=True
            for f,rows in EM.items():
                tgt=[h for h,_ in rows]
                hs=[h for h,_ in rows]
                order=orders_for(hs)[oname]
                if slot_scan_chain(N,order,prepend)!=tgt:
                    ok=False; break
            if ok:
                found.append(("chain",N,oname,prepend))
                print("CHAIN MATCH N=%d insert=%s prepend=%s"%(N,oname,prepend)); sys.stdout.flush()

print("chain done, matches=",len(found)); sys.stdout.flush()

# open addressing - guarded
for N in range(8,2049):
    for oname in ["asc","desc","observed","rev_observed"]:
        for probe in ["linear"]:
            ok=True
            for f,rows in EM.items():
                tgt=[h for h,_ in rows]
                hs=[h for h,_ in rows]
                order=orders_for(hs)[oname]
                if slot_scan_open(N,order,probe)!=tgt:
                    ok=False; break
            if ok:
                print("OPEN MATCH N=%d insert=%s probe=%s"%(N,oname,probe)); sys.stdout.flush()
print("ALL DONE"); sys.stdout.flush()
