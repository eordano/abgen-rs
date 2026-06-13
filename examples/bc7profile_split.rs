use abgen::bc7_pure::{encode_blocks, Params};
use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::{HashMap, HashSet};
use std::io::Read;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn extract(b: &Bundle) -> Vec<(usize, usize, Vec<u8>)> {
    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(d) = &f.content {
            ress.push((f.name.clone(), d));
        }
    }
    let mut e: Vec<(i64, (usize, usize, Vec<u8>))> = Vec::new();
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for obj in &sf.objects {
            if obj.class_id != 28 {
                continue;
            }
            let Ok(v) = sf.read_typetree(obj) else {
                continue;
            };
            if gi(&v, "m_TextureFormat") != 25 {
                continue;
            }
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let pay: Vec<u8> = if let Some(Value::Bytes(d)) = v
                .get("image data")
                .filter(|x| matches!(x, Value::Bytes(b) if !b.is_empty()))
            {
                d.clone()
            } else if let Some(sd) = v.get("m_StreamData") {
                let off = gi(sd, "offset") as usize;
                let size = gi(sd, "size") as usize;
                let path = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                let base = path.rsplit('/').next().unwrap_or(path);
                let Some((_, d)) = ress.iter().find(|(nm, _)| nm == base) else {
                    continue;
                };
                if off + size > d.len() {
                    continue;
                }
                d[off..off + size].to_vec()
            } else {
                continue;
            };
            e.push((obj.path_id, (w, h, pay)));
        }
    }
    e.sort_by_key(|(p, _)| *p);
    e.into_iter().map(|(_, x)| x).collect()
}

fn main() {
    let cap = std::env::args().nth(1).unwrap();
    let pairs = std::env::args().nth(2).unwrap();
    let perc = true;
    let pbasic = Params::basic(perc);
    let pslow = Params::slow(perc);

    let lines: Vec<String> = std::fs::read_to_string(&pairs)
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect();
    let mut wanted: HashSet<[u8; 16]> = HashSet::new();
    let mut tex: Vec<(Vec<[u8; 16]>, Vec<[u8; 16]>)> = Vec::new();
    for line in &lines {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() < 3 || !p[2].starts_with("standalone-texture") {
            continue;
        }
        let Ok(ob) = Bundle::load(std::path::Path::new(p[0])) else {
            continue;
        };
        let Ok(rb) = Bundle::load(std::path::Path::new(p[1])) else {
            continue;
        };
        let op = extract(&ob);
        let rp = extract(&rb);
        if op.len() != rp.len() {
            continue;
        }
        for (ti, (w, h, opay)) in op.iter().enumerate() {
            let (rw, rh, rpay) = &rp[ti];
            if w != rw || h != rh || opay.len() != rpay.len() {
                continue;
            }
            let nb = (w.div_ceil(4).max(1)) * (h.div_ceil(4).max(1));
            let mut ov = Vec::new();
            let mut rv = Vec::new();
            for i in 0..nb {
                if i * 16 + 16 > opay.len() {
                    break;
                }
                let mut o = [0u8; 16];
                o.copy_from_slice(&opay[i * 16..i * 16 + 16]);
                let mut r = [0u8; 16];
                r.copy_from_slice(&rpay[i * 16..i * 16 + 16]);
                wanted.insert(o);
                ov.push(o);
                rv.push(r);
            }
            tex.push((ov, rv));
        }
    }
    eprintln!("textures={} wanted-blocks={}", tex.len(), wanted.len());

    let mut map: HashMap<[u8; 16], [u8; 64]> = HashMap::with_capacity(wanted.len());
    let f = std::fs::File::open(&cap).unwrap();
    let mut rdr = std::io::BufReader::with_capacity(1 << 24, f);
    let mut bb = [0u8; 80];
    while rdr.read_exact(&mut bb).is_ok() {
        let mut k = [0u8; 16];
        k.copy_from_slice(&bb[..16]);
        if wanted.contains(&k) && !map.contains_key(&k) {
            let mut vv = [0u8; 64];
            vv.copy_from_slice(&bb[16..80]);
            map.insert(k, vv);
        }
    }
    eprintln!("recovered {} inputs", map.len());

    use rayon::prelude::*;
    #[derive(Default)]
    struct Acc {
        basic_wins: u64,
        slow_wins: u64,
        tie: u64,
        basic_perfect: u64,
        slow_perfect: u64,
        either_perfect: u64,
        tot_basic_ref: u64,
        tot_slow_ref: u64,
        tot_best_ref: u64,
        tot_blk: u64,
    }
    let acc = tex
        .par_iter()
        .map(|(ov, rv)| {
            let mut a = Acc::default();
            let mut bm = 0u64;
            let mut sm = 0u64;
            let mut bestm = 0u64;
            for (o, r) in ov.iter().zip(rv.iter()) {
                let Some(inp) = map.get(o) else { continue };
                let eb = encode_blocks(inp, 1, &pbasic);
                let es = encode_blocks(inp, 1, &pslow);
                let b = eb.as_slice() == r;
                let s = es.as_slice() == r;
                if b {
                    bm += 1;
                }
                if s {
                    sm += 1;
                }
                if b || s {
                    bestm += 1;
                }
                a.tot_blk += 1;
            }
            a.tot_basic_ref = bm;
            a.tot_slow_ref = sm;
            a.tot_best_ref = bestm;
            let nb = ov.len() as u64;
            if bm == nb {
                a.basic_perfect = 1;
            }
            if sm == nb {
                a.slow_perfect = 1;
            }
            if bm == nb || sm == nb {
                a.either_perfect = 1;
            }
            match bm.cmp(&sm) {
                std::cmp::Ordering::Greater => a.basic_wins = 1,
                std::cmp::Ordering::Less => a.slow_wins = 1,
                std::cmp::Ordering::Equal => a.tie = 1,
            }
            a
        })
        .reduce(Acc::default, |mut x, y| {
            x.basic_wins += y.basic_wins;
            x.slow_wins += y.slow_wins;
            x.tie += y.tie;
            x.basic_perfect += y.basic_perfect;
            x.slow_perfect += y.slow_perfect;
            x.either_perfect += y.either_perfect;
            x.tot_basic_ref += y.tot_basic_ref;
            x.tot_slow_ref += y.tot_slow_ref;
            x.tot_best_ref += y.tot_best_ref;
            x.tot_blk += y.tot_blk;
            x
        });
    let (basic_wins, slow_wins, tie) = (acc.basic_wins, acc.slow_wins, acc.tie);
    let (basic_perfect, slow_perfect, either_perfect) =
        (acc.basic_perfect, acc.slow_perfect, acc.either_perfect);
    let (tot_basic_ref, tot_slow_ref, tot_best_ref, tot_blk) = (
        acc.tot_basic_ref,
        acc.tot_slow_ref,
        acc.tot_best_ref,
        acc.tot_blk,
    );
    println!("=== per-texture basic vs slow (ref-match) ===");
    println!("textures: {}", tex.len());
    println!("  basic wins:  {basic_wins}");
    println!("  slow wins:   {slow_wins}");
    println!("  tie:         {tie}");
    println!("  basic-perfect (all mip0 blocks ==ref): {basic_perfect}");
    println!("  slow-perfect:                          {slow_perfect}");
    println!("  EITHER-perfect (per-texture oracle):   {either_perfect}");
    println!("=== block totals ===");
    println!("  blocks:            {tot_blk}");
    println!(
        "  basic ==ref:       {tot_basic_ref} ({:.2}%)",
        100.0 * tot_basic_ref as f64 / tot_blk as f64
    );
    println!(
        "  slow  ==ref:       {tot_slow_ref} ({:.2}%)",
        100.0 * tot_slow_ref as f64 / tot_blk as f64
    );
    println!(
        "  per-block best:    {tot_best_ref} ({:.2}%)",
        100.0 * tot_best_ref as f64 / tot_blk as f64
    );
}
