use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
fn cf(c: &Value, k: &str) -> i64 {
    c.as_map().unwrap().get(k).and_then(|x| x.as_i64()).unwrap()
}
fn rd(p: &Path) -> BTreeMap<i64, Vec<[u32; 4]>> {
    let mut r = BTreeMap::new();
    let bytes = match std::fs::read(p) {
        Ok(b) => b,
        _ => return r,
    };
    let b = match Bundle::load_bytes(&bytes) {
        Ok(b) => b,
        _ => return r,
    };
    let sf = match b.serialized() {
        Some(s) => s,
        None => return r,
    };
    for o in sf.objects.iter() {
        if o.class_id != 43 {
            continue;
        }
        let v = match sf.read_typetree(o) {
            Ok(v) => v,
            _ => continue,
        };
        let m = match v.as_map() {
            Some(m) => m,
            None => continue,
        };
        let vd = match m.get("m_VertexData").and_then(|x| x.as_map()) {
            Some(x) => x,
            None => continue,
        };
        let chans = match vd.get("m_Channels").and_then(|x| x.as_array()) {
            Some(c) => c.to_vec(),
            None => continue,
        };
        let data = match vd.get("m_DataSize") {
            Some(Value::Bytes(d)) => d.clone(),
            _ => continue,
        };
        let vc = match vd.get("m_VertexCount").and_then(|x| x.as_i64()) {
            Some(x) => x as usize,
            None => continue,
        };
        const BW: usize = 12;
        if chans.len() <= BW || cf(&chans[BW], "dimension") <= 0 {
            continue;
        }
        let mut bs: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
        for (i, c) in chans.iter().enumerate() {
            if cf(c, "dimension") > 0 {
                bs.entry(cf(c, "stream")).or_default().push(i);
            }
        }
        let mut base = 0usize;
        let mut sb: BTreeMap<i64, (usize, usize)> = BTreeMap::new();
        for (si, (s, cis)) in bs.iter().enumerate() {
            if si > 0 {
                while !base.is_multiple_of(16) {
                    base += 1;
                }
            }
            let sr = cis
                .iter()
                .map(|&ci| cf(&chans[ci], "offset") + cf(&chans[ci], "dimension") * 4)
                .max()
                .unwrap();
            let st = ((sr + 3) & !3) as usize;
            sb.insert(*s, (base, st));
            base += st * vc;
        }
        let s = cf(&chans[BW], "stream");
        let (b0, st) = sb[&s];
        let coff = cf(&chans[BW], "offset") as usize;
        let mut wv = vec![];
        let mut ok = true;
        for vi in 0..vc {
            let row = b0 + vi * st;
            let mut w = [0u32; 4];
            for (k, wk) in w.iter_mut().enumerate() {
                let off = row + coff + k * 4;
                if off + 4 > data.len() {
                    ok = false;
                    break;
                }
                *wk = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            }
            if !ok {
                break;
            }
            wv.push(w);
        }
        if ok {
            r.insert(o.path_id, wv);
        }
    }
    r
}
fn b2f(u: u32) -> f32 {
    f32::from_bits(u)
}
fn main() {
    let mut a = std::env::args().skip(1);
    let pdir = a.next().unwrap();
    let rdir = a.next().unwrap();
    let (mut ma, mut mb, mut mc, mut md, mut me, mut none, mut tot) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for e in std::fs::read_dir(&rdir).unwrap().filter_map(|x| x.ok()) {
        let cid = e.file_name().to_string_lossy().to_string();
        if cid.contains('.') {
            continue;
        }
        let pd = PathBuf::from(&pdir).join(&cid);
        let rd2 = e.path();
        if !pd.is_dir() {
            continue;
        }
        for bf in std::fs::read_dir(&pd)
            .into_iter()
            .flatten()
            .filter_map(|x| x.ok())
        {
            let n = bf.file_name().to_string_lossy().to_string();
            if !(n.ends_with("_windows") || n.ends_with("_mac")) {
                continue;
            }
            let pp = pd.join(&n);
            let rp = rd2.join(&n);
            if !rp.exists() {
                continue;
            }
            let pm = rd(&pp);
            let rm = rd(&rp);
            for (pid, pw) in &pm {
                let rw = match rm.get(pid) {
                    Some(x) => x,
                    None => continue,
                };
                if pw.len() != rw.len() {
                    continue;
                }
                for i in 0..pw.len() {
                    let p = pw[i];
                    let r = rw[i];
                    if p == r {
                        continue;
                    }
                    tot += 1;
                    let av = [b2f(p[0]), b2f(p[1]), b2f(p[2]), b2f(p[3])];
                    let ws = ((av[0] + av[1]) + av[2]) + av[3];
                    let ws64 = ((av[0] as f64) + (av[1] as f64) + (av[2] as f64) + (av[3] as f64))
                        as f32 ;
                    let rcp = 1.0f32 / ws;
                    let rcp64 = 1.0f32 / ws64;
                    let mut ea = true;
                    let mut eb = true;
                    let mut ec = true;
                    let mut ed = true;
                    let mut ee = true;
                    for k in 0..4 {
                        let div = if av[k] == 0.0 { 0.0 } else { av[k] / ws };
                        let mul = if av[k] == 0.0 { 0.0 } else { av[k] * rcp };
                        let div64 = if av[k] == 0.0 { 0.0 } else { av[k] / ws64 };
                        let mul64 = if av[k] == 0.0 { 0.0 } else { av[k] * rcp64 };
                        if p[k] != r[k] {
                            ea = false;
                        }
                        if div.to_bits() != r[k] {
                            eb = false;
                        }
                        if mul.to_bits() != r[k] {
                            ec = false;
                        }
                        if div64.to_bits() != r[k] {
                            ed = false;
                        }
                        if mul64.to_bits() != r[k] {
                            ee = false;
                        }
                    }
                    if ea {
                        ma += 1;
                    } else if eb {
                        mb += 1;
                    } else if ec {
                        mc += 1;
                    } else if ed {
                        md += 1;
                    } else if ee {
                        me += 1;
                    } else {
                        none += 1;
                    }
                }
            }
        }
    }
    println!("divergent vertices={tot}");
    println!("  passthrough(A)={ma} f32div(B)={mb} f32mul(C)={mc} f64div(D)={md} f64mul(E)={me} none={none}");
}
