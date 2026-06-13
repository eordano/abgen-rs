use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::{BTreeMap, HashMap, VecDeque};

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let src_path = args.next().expect("source image");
    let ref_path = args.next().expect("ref bundle");

    let raw = std::fs::read(&src_path).unwrap();
    let img = image::load_from_memory(&raw).expect("decode").to_rgba8();
    let (sw, sh) = img.dimensions();

    let b = Bundle::load(std::path::Path::new(&ref_path)).unwrap();
    let mut ress: Vec<(String, Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data.clone()));
        }
    }
    let mut found = None;
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
            let w = gi(&v, "m_Width") as u32;
            let h = gi(&v, "m_Height") as u32;
            let payload: Vec<u8> = match v.get("image data") {
                Some(Value::Bytes(bts)) if !bts.is_empty() => bts.clone(),
                _ => {
                    let sd = v.get("m_StreamData").unwrap();
                    let off = gi(sd, "offset") as usize;
                    let size = gi(sd, "size") as usize;
                    let path = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                    let base = path.rsplit('/').next().unwrap_or(path);
                    let d = &ress.iter().find(|(nm, _)| nm == base).unwrap().1;
                    d[off..off + size].to_vec()
                }
            };
            found = Some((w, h, payload));
        }
    }
    let (tw, th, rpay) = found.expect("no BC7 texture in ref");
    let (w, h) = (tw as usize, th as usize);

    let unbled: Vec<u8> = if (tw, th) != (sw, sh) {
        abgen::resize::box_downscale_rgba(img.as_raw(), sw as usize, sh as usize, w, h)
    } else {
        img.as_raw().clone()
    };
    let mut base = vec![0u8; w * h * 4];
    for y in 0..h {
        base[y * w * 4..(y + 1) * w * 4]
            .copy_from_slice(&unbled[(h - 1 - y) * w * 4..(h - y) * w * 4]);
    }

    let bw = w.div_ceil(4);
    let bh = h.div_ceil(4);
    let mut refpix = vec![0u32; w * h];
    texture2ddecoder::decode_bc7(&rpay[..bw * bh * 16], w, h, &mut refpix).unwrap();

    let mut dist = vec![u32::MAX; w * h];
    let mut q = VecDeque::new();
    for i in 0..w * h {
        if base[i * 4 + 3] > 0 {
            dist[i] = 0;
            q.push_back(i);
        }
    }
    while let Some(i) = q.pop_front() {
        let (x, y) = (i % w, i / w);
        let d = dist[i] + 1;
        let mut push = |j: usize| {
            if dist[j] > d {
                dist[j] = d;
                q.push_back(j);
            }
        };
        if y > 0 {
            push(i - w);
        }
        if y + 1 < h {
            push(i + w);
        }
        if x > 0 {
            push(i - 1);
        }
        if x + 1 < w {
            push(i + 1);
        }
    }

    let mut by_color: HashMap<[u8; 3], Vec<(i32, i32)>> = HashMap::new();
    for i in 0..w * h {
        if base[i * 4 + 3] > 0 {
            by_color
                .entry([base[i * 4], base[i * 4 + 1], base[i * 4 + 2]])
                .or_default()
                .push(((i % w) as i32, (i / w) as i32));
        }
    }

    let mut hist: BTreeMap<(i32, i32), usize> = BTreeMap::new();
    let mut nomatch = 0usize;
    let mut total = 0usize;
    let mut by_dist_axis: BTreeMap<u32, [usize; 3]> = BTreeMap::new();
    for i in 0..w * h {
        if base[i * 4 + 3] != 0 || dist[i] > 32 {
            continue;
        }
        total += 1;
        let p = refpix[i];
        let rgb = [
            ((p >> 16) & 0xFF) as u8,
            ((p >> 8) & 0xFF) as u8,
            (p & 0xFF) as u8,
        ];
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        let e = by_dist_axis.entry(dist[i]).or_default();
        if let Some(srcs) = by_color.get(&rgb) {
            let mut best = i32::MAX;
            let mut bd = (0, 0);
            for &(sx, sy) in srcs {
                let d = (sx - x).abs() + (sy - y).abs();
                if d < best {
                    best = d;
                    bd = (sx - x, sy - y);
                }
            }
            *hist.entry(bd).or_default() += 1;
            if bd.0 == 0 || bd.1 == 0 {
                e[0] += 1;
            } else {
                e[1] += 1;
            }
        } else {
            nomatch += 1;
            e[2] += 1;
        }
    }
    println!("transparent pixels in reach: {total}, no exact source color match: {nomatch}");
    println!("displacement histogram (top 40):");
    let mut v: Vec<_> = hist.iter().collect();
    v.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for ((dx, dy), c) in v.iter().take(40) {
        println!("  ({dx:>3},{dy:>3}): {c}");
    }
    println!("by dist [axis, diag, nomatch]:");
    for (d, a) in &by_dist_axis {
        println!("  d={d}: {a:?}");
    }
}
