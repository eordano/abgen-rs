import os, binascii
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
        if f.endswith(".tsv"):
            SETS[d+"/"+f]=load(d+"/"+f)

def dcrc_le(h): return binascii.crc32((h&M).to_bytes(4,'little'))&M
def dcrc_be(h): return binascii.crc32((h&M).to_bytes(4,'big'))&M

def buckets_sorted(b): return all(b[i]>=b[i-1] for i in range(1,len(b)))

for name,hf in [("dcrc_le",dcrc_le),("dcrc_be",dcrc_be)]:
    for useTop in [False,True]:
        for rev in [False,True]:
            hits=0; total=0; details=[]
            for f,rows in SETS.items():
                total+=1
                hs=[h for h,_ in rows]; cnt=len(hs)
                ok=False; goodN=None
                for sb in range(2,16):
                    N=1<<sb
                    if N<cnt: continue
                    hh=[hf(h) for h in hs]
                    if useTop: b=[(x>>(32-sb)) for x in hh]
                    else: b=[x%N for x in hh]
                    if rev: b=[ ( (N-1-x) ) for x in b]
                    if buckets_sorted(b): ok=True; goodN=N; break
                if ok: hits+=1
                else: details.append(os.path.basename(f))
            print("%-8s top=%-5s rev=%-5s : %d/%d  miss=%s"%(name,useTop,rev,hits,total,details[:6]))
