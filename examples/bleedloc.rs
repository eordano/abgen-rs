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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ob = Bundle::load(std::path::Path::new(&args[0])).unwrap();
    let rb = Bundle::load(std::path::Path::new(&args[1])).unwrap();
    let pid_filter: Option<i64> = args.get(2).and_then(|s| s.parse().ok());
    let ot = extract(&ob);
    let rt = extract(&rb);

    for (pid, o) in &ot {
        if let Some(pf) = pid_filter {
            if pf != *pid {
                continue;
            }
        }
        let Some(r) = rt.get(pid) else { continue };
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
        let mut dist = vec![u32::MAX; n];
        let mut q = std::collections::VecDeque::new();
        for i in 0..n {
            if rd[i * 4 + 3] > 0 {
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

        let mut n_diff = 0;
        let mut by_dist: BTreeMap<u32, usize> = BTreeMap::new();
        let mut by_border: BTreeMap<&str, usize> = BTreeMap::new();
        let mut beyond31 = 0;
        let mut samples: Vec<String> = Vec::new();
        for i in 0..n {
            if rd[i * 4 + 3] != 0 {
                continue;
            }
            let dr = (od[i * 4] as i32 - rd[i * 4] as i32).abs();
            let dg = (od[i * 4 + 1] as i32 - rd[i * 4 + 1] as i32).abs();
            let db = (od[i * 4 + 2] as i32 - rd[i * 4 + 2] as i32).abs();
            if dr + dg + db == 0 {
                continue;
            }
            n_diff += 1;
            let (x, y) = (i % w, i / w);
            let d = dist[i];
            *by_dist.entry(d.min(99)).or_default() += 1;
            if d > 31 {
                beyond31 += 1;
            }

            let edge = x.min(w - 1 - x).min(y.min(h - 1 - y));
            let b = if edge == 0 {
                "edge0"
            } else if edge < 16 {
                "edge<16"
            } else {
                "interior"
            };
            *by_border.entry(b).or_default() += 1;
            if samples.len() < 25 {
                samples.push(format!(
                    "  ({x},{y}) d={d} ours=[{},{},{}] ref=[{},{},{}]",
                    od[i * 4],
                    od[i * 4 + 1],
                    od[i * 4 + 2],
                    rd[i * 4],
                    rd[i * 4 + 1],
                    rd[i * 4 + 2]
                ));
            }
        }
        println!("pid={pid} {w}x{h} transparent-diff texels: {n_diff}");
        println!("  by L1-dist-to-seed: {by_dist:?}  (>31: {beyond31})");
        println!("  by border: {by_border:?}");
        for s in &samples {
            println!("{s}");
        }
    }
}
