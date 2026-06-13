import os, sys

def load(f):
    rows=[]
    for line in open(f):
        line=line.rstrip("\n")
        if "\t" not in line: continue
        h,n=line.split("\t",1)
        rows.append((int(h),n))
    return rows
EM={f:load("/tmp/tos-ref/"+f) for f in sorted(os.listdir("/tmp/tos-ref")) if f.endswith(".tsv")}
M=0xffffffff
M64=0xffffffffffffffff

def fnv1a(s):
    h=0x811c9dc5
    for b in s: h=((h^b)*16777619)&M
    return h
def fnv1(s):
    h=0x811c9dc5
    for b in s: h=((h*16777619)&M)^b
    return h
def fnv1a64(s):
    h=0xcbf29ce484222325
    for b in s: h=((h^b)*0x100000001b3)&M64
    return h
def djb2(s):
    h=5381
    for b in s: h=((h*33)+b)&M
    return h
def djb2x(s):
    h=5381
    for b in s: h=((h*33)^b)&M
    return h
def sdbm(s):
    h=0
    for b in s: h=(b+(h<<6)+(h<<16)-h)&M
    return h
def java31(s):
    h=0
    for b in s: h=((h*31)+b)&M
    return h
def elf(s):
    h=0
    for b in s:
        h=(h<<4)+b
        x=h&0xF0000000
        if x: h^=x>>24
        h&=~x
        h&=M
    return h
def jenkins(s):
    h=0
    for b in s:
        h=(h+b)&M; h=(h+(h<<10))&M; h^=h>>6
    h=(h+(h<<3))&M; h^=h>>11; h=(h+(h<<15))&M
    return h
def bkdr(s):
    h=0
    for b in s: h=(h*131+b)&M
    return h
HF={"fnv1a":fnv1a,"fnv1":fnv1,"fnv1a64":fnv1a64,"djb2":djb2,"djb2x":djb2x,"sdbm":sdbm,
    "java31":java31,"elf":elf,"jenkins":jenkins,"bkdr":bkdr}

def buckets_sorted(b): return all(b[i]>=b[i-1] for i in range(1,len(b)))

print("string-hash sorted-bucket (strict, no wrap), pow2 mod & top-bits, both directions")
for hn,hf in HF.items():
    for useTop in [False,True]:
        for rev in [False,True]:
            hits=0
            for f,rows in EM.items():
                names=[n.encode('utf-8','surrogatepass') if isinstance(n,str) else n for _,n in rows]
                names=[n if isinstance(n,(bytes,)) else n for n in names]
                ok=False
                for sb in range(2,14):
                    N=1<<sb
                    if N<len(names): continue
                    hh=[hf(list(n)) for n in names]
                    if useTop: b=[(x&M)>>(32-sb) for x in hh]
                    else: b=[x%N for x in hh]
                    if rev: b=[N-1-x for x in b]
                    if buckets_sorted(b): ok=True; break
                if ok: hits+=1
            if hits>0:
                print("%-8s top=%-5s rev=%-5s : %d/21"%(hn,useTop,rev,hits))
print("DONE (only nonzero printed)")
