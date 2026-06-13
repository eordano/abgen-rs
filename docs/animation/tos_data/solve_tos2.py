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

def slot_chain(N, order, prepend):
    buckets=[[] for _ in range(N)]
    for h in order:
        b=h%N
        if prepend: buckets[b].insert(0,h)
        else: buckets[b].append(h)
    out=[]
    for b in buckets: out.extend(b)
    return out

def slot_open(N, order, probe):
    table=[None]*N
    for h in order:
        i=h%N; step=0
        while table[i] is not None:
            step+=1
            if step>N: return None
            if probe=="linear": i=(i+1)%N
            elif probe=="quadtri": i=(h%N + step*(step+1)//2)%N
            else: i=(h%N + step*step)%N
        table[i]=h
    return [x for x in table if x is not None]

def orders_for(hashes):
    return {
      "asc":sorted(hashes),
      "desc":sorted(hashes,reverse=True),
      "observed":list(hashes),
      "rev_observed":list(hashes)[::-1],
    }

# Per emote, find ALL (N, mode, insert) that reproduce its order. N can be any.
print("Per-emote feasible (N, model, insert):")
for f,rows in sorted(EM.items()):
    tgt=[h for h,_ in rows]
    hs=[h for h,_ in rows]
    cnt=len(hs)
    sols=[]
    for N in range(cnt, 4*cnt+200):
        ords=orders_for(hs)
        for oname,order in ords.items():
            if slot_chain(N,order,False)==tgt: sols.append((N,"chain_app",oname))
            if slot_chain(N,order,True)==tgt: sols.append((N,"chain_pre",oname))
            for probe in ["linear","quadtri"]:
                if slot_open(N,order,probe)==tgt: sols.append((N,"open_"+probe,oname))
    print("%-14s cnt=%2d  sols=%d  %s"%(f,cnt,len(sols),sols[:12]))
    sys.stdout.flush()
