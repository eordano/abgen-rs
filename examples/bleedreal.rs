use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

struct Tex {
    w: usize,
    h: usize,
    fmt: i64,
    payload: Vec<u8>,
}

fn extract(bundle: &Bundle) -> BTreeMap<i64, Tex> {
    let mut out = BTreeMap::new();
    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &bundle.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data));
        }
    }
    for f in &bundle.files {
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
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let fmt = gi(&v, "m_TextureFormat");
            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(bts) if !bts.is_empty() => Some(bts.as_slice()),
                _ => None,
            });
            let payload: Vec<u8> = if let Some(d) = inline {
                d.to_vec()
            } else if let Some(sd) = v.get("m_StreamData") {
                let off = gi(sd, "offset") as usize;
                let size = gi(sd, "size") as usize;
                let path = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                let base = path.rsplit('/').next().unwrap_or(path);
                let Some((_, data)) = ress.iter().find(|(nm, _)| nm == base) else {
                    continue;
                };
                if off + size > data.len() {
                    continue;
                }
                data[off..off + size].to_vec()
            } else {
                continue;
            };
            out.insert(obj.path_id, Tex { w, h, fmt, payload });
        }
    }
    out
}

fn decode_bc7_level(data: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut px = vec![0u32; w * h];
    texture2ddecoder::decode_bc7(data, w, h, &mut px).unwrap();
    let mut rgba = vec![0u8; w * h * 4];
    for (i, p) in px.iter().enumerate() {
        let [b, g, r, a] = p.to_le_bytes();
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = a;
    }
    rgba
}

fn l1_dist(seed: &[bool], w: usize, h: usize) -> Vec<u32> {
    let n = w * h;
    let mut dist = vec![u32::MAX; n];
    let mut q = std::collections::VecDeque::new();
    for (i, &s) in seed.iter().enumerate() {
        if s {
            dist[i] = 0;
            q.push_back(i);
        }
    }
    while let Some(i) = q.pop_front() {
        let (x, y) = (i % w, i / w);
        let d = dist[i];
        let mut step = |nx: usize, ny: usize, q: &mut std::collections::VecDeque<usize>| {
            let j = ny * w + nx;
            if dist[j] > d + 1 {
                dist[j] = d + 1;
                q.push_back(j);
            }
        };
        if x > 0 {
            step(x - 1, y, &mut q);
        }
        if x + 1 < w {
            step(x + 1, y, &mut q);
        }
        if y > 0 {
            step(x, y - 1, &mut q);
        }
        if y + 1 < h {
            step(x, y + 1, &mut q);
        }
    }
    dist
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pairs: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(&args[0]).unwrap()).unwrap();
    let limit: usize = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(usize::MAX);

    let mut tex_zero_wrong = 0usize;
    let mut tex_with_wrong = 0usize;
    let mut total_wrong = 0u64;
    let mut total_noise = 0u64;
    let mut wrong_by_dist: BTreeMap<u32, u64> = BTreeMap::new();
    let mut wrong_beyond31 = 0u64;
    let mut ntex = 0usize;

    let walk_mips = std::env::args().any(|a| a == "--mips");
    let mut per_mip: BTreeMap<usize, (u64, u64)> = BTreeMap::new();

    for pair in pairs.iter().take(limit) {
        let op = pair["ours"].as_str().unwrap();
        let rp = pair["ref"].as_str().unwrap();
        let pid: i64 = pair["pid"].as_i64().unwrap();
        let (Ok(ob), Ok(rb)) = (
            Bundle::load(std::path::Path::new(op)),
            Bundle::load(std::path::Path::new(rp)),
        ) else {
            continue;
        };
        let (ot, rt) = (extract(&ob), extract(&rb));
        let (Some(o), Some(r)) = (ot.get(&pid), rt.get(&pid)) else {
            continue;
        };
        if o.fmt != 25 || o.w != r.w || o.h != r.h {
            continue;
        }
        let (w, h) = (o.w, o.h);
        let blk = w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 16;
        if o.payload.len() < blk || r.payload.len() < blk {
            continue;
        }
        let od = decode_bc7_level(&o.payload[..blk], w, h);
        let rd = decode_bc7_level(&r.payload[..blk], w, h);
        let n = w * h;
        let mut seed = vec![false; n];
        for i in 0..n {
            if rd[i * 4 + 3] > 0 {
                seed[i] = true;
            }
        }
        let dist = l1_dist(&seed, w, h);
        ntex += 1;

        if walk_mips {
            let (mut mw, mut mh) = (w, h);
            let (mut ooff, mut roff) = (0usize, 0usize);
            let mut lvl = 0usize;
            loop {
                let lb = mw.div_ceil(4).max(1) * mh.div_ceil(4).max(1) * 16;
                if ooff + lb > o.payload.len() || roff + lb > r.payload.len() {
                    break;
                }
                let ol = decode_bc7_level(&o.payload[ooff..ooff + lb], mw, mh);
                let rl = decode_bc7_level(&r.payload[roff..roff + lb], mw, mh);
                let e = per_mip.entry(lvl).or_default();
                for i in 0..mw * mh {
                    if rl[i * 4 + 3] != 0 {
                        continue;
                    }
                    e.1 += 1;
                    let dr = (ol[i * 4] as i32 - rl[i * 4] as i32).abs();
                    let dg = (ol[i * 4 + 1] as i32 - rl[i * 4 + 1] as i32).abs();
                    let db = (ol[i * 4 + 2] as i32 - rl[i * 4 + 2] as i32).abs();
                    if dr.max(dg).max(db) > 3 {
                        e.0 += 1;
                    }
                }
                ooff += lb;
                roff += lb;
                mw = (mw / 2).max(1);
                mh = (mh / 2).max(1);
                lvl += 1;
                if mw == 1 && mh == 1 {
                    break;
                }
            }
        }
        let mut twrong = 0u64;
        for i in 0..n {
            if rd[i * 4 + 3] != 0 {
                continue;
            }
            let dr = (od[i * 4] as i32 - rd[i * 4] as i32).abs();
            let dg = (od[i * 4 + 1] as i32 - rd[i * 4 + 1] as i32).abs();
            let db = (od[i * 4 + 2] as i32 - rd[i * 4 + 2] as i32).abs();
            let m = dr.max(dg).max(db);
            if m == 0 {
                continue;
            }
            if m <= 3 {
                total_noise += 1;
            } else {
                twrong += 1;
                total_wrong += 1;
                *wrong_by_dist.entry(dist[i].min(40)).or_default() += 1;
                if dist[i] > 31 {
                    wrong_beyond31 += 1;
                }
            }
        }
        if twrong == 0 {
            tex_zero_wrong += 1;
        } else {
            tex_with_wrong += 1;
        }
    }

    println!("scored {ntex} textures");
    println!("  textures with ZERO genuine wrong-seed (noise-only): {tex_zero_wrong}");
    println!("  textures with genuine wrong-seed: {tex_with_wrong}");
    println!("  total wrong-seed texels: {total_wrong}  (noise texels: {total_noise})");
    println!("  wrong beyond reach-31: {wrong_beyond31}");
    println!("  wrong-by-L1-dist: {wrong_by_dist:?}");
    if !per_mip.is_empty() {
        println!("  per-mip-level transparent wrong>3 / transparent:");
        for (lvl, (wr, t)) in &per_mip {
            let pct = if *t > 0 {
                100.0 * *wr as f64 / *t as f64
            } else {
                0.0
            };
            println!("    mip{lvl}: {wr}/{t}  ({pct:.2}%)");
        }
    }
}
