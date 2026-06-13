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
    mips: i64,
    color_space: i64,
    payload: Vec<u8>,
}
fn extract(b: &Bundle) -> BTreeMap<i64, Tex> {
    let mut out = BTreeMap::new();
    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(d) = &f.content {
            ress.push((f.name.clone(), d));
        }
    }
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for o in &sf.objects {
            if o.class_id != 28 {
                continue;
            }
            let Ok(v) = sf.read_typetree(o) else { continue };
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let fmt = gi(&v, "m_TextureFormat");
            let mips = gi(&v, "m_MipCount");
            let color_space = gi(&v, "m_ColorSpace");
            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(b) if !b.is_empty() => Some(b.as_slice()),
                _ => None,
            });
            let payload: Vec<u8> = if let Some(d) = inline {
                d.to_vec()
            } else if let Some(sd) = v.get("m_StreamData") {
                let off = gi(sd, "offset") as usize;
                let sz = gi(sd, "size") as usize;
                let path = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                let base = path.rsplit('/').next().unwrap_or(path);
                let Some((_, data)) = ress.iter().find(|(n, _)| n == base) else {
                    continue;
                };
                if off + sz > data.len() {
                    continue;
                }
                data[off..off + sz].to_vec()
            } else {
                continue;
            };
            out.insert(
                o.path_id,
                Tex {
                    w,
                    h,
                    fmt,
                    mips,
                    color_space,
                    payload,
                },
            );
        }
    }
    out
}

fn l1(i: usize, s: i32, w: usize) -> i32 {
    let (x, y) = ((i % w) as i32, (i / w) as i32);
    let (sx, sy) = (s % w as i32, s / w as i32);
    (x - sx).abs() + (y - sy).abs()
}

fn bleed_variant(rgba: &mut [u8], w: usize, h: usize, variant: &str) {
    let n = w * h;
    let has_t = rgba.chunks_exact(4).any(|p| p[3] == 0);
    let has_o = rgba.chunks_exact(4).any(|p| p[3] > 0);
    if !(has_t && has_o) {
        return;
    }
    if variant.starts_with("exact") {
        let lo = variant == "exact-lo";
        let mut src = vec![-1i32; n];
        let mut dist = vec![u32::MAX; n];
        let mut q = std::collections::VecDeque::new();
        for i in 0..n {
            if rgba[i * 4 + 3] > 0 {
                src[i] = i as i32;
                dist[i] = 0;
                q.push_back(i);
            }
        }
        while let Some(i) = q.pop_front() {
            if dist[i] >= 31 {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let s = src[i];
            let nd = dist[i] + 1;
            let mut step = |j: usize, q: &mut std::collections::VecDeque<usize>| {
                if dist[j] > nd {
                    dist[j] = nd;
                    src[j] = s;
                    q.push_back(j);
                } else if dist[j] == nd {
                    let better = if lo { s < src[j] } else { s > src[j] };
                    if better {
                        src[j] = s;
                    }
                }
            };
            if x > 0 {
                step(i - 1, &mut q);
            }
            if x + 1 < w {
                step(i + 1, &mut q);
            }
            if y > 0 {
                step(i - w, &mut q);
            }
            if y + 1 < h {
                step(i + w, &mut q);
            }
        }
        let snap: Vec<u8> = rgba.to_vec();
        for i in 0..n {
            if rgba[i * 4 + 3] == 0 && src[i] >= 0 {
                let s = src[i] as usize * 4;
                rgba[i * 4] = snap[s];
                rgba[i * 4 + 1] = snap[s + 1];
                rgba[i * 4 + 2] = snap[s + 2];
            }
        }
        return;
    }

    let mut seed: Vec<i32> = vec![-1; n];
    for i in 0..n {
        if rgba[i * 4 + 3] > 0 {
            seed[i] = i as i32;
        }
    }
    let strict_lt = variant != "last";
    let sched: Vec<usize> = match variant {
        "jfa+1" => vec![16, 8, 4, 2, 1, 1],
        "plus1" => vec![1, 16, 8, 4, 2, 1],
        "twosweep" => vec![16, 8, 4, 2, 1, 16, 8, 4, 2, 1],
        "onepass1" => vec![
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            1, 1,
        ],
        _ => vec![16, 8, 4, 2, 1],
    };
    for k in sched {
        let snap = seed.clone();
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if rgba[idx * 4 + 3] > 0 {
                    continue;
                }
                let mut best = seed[idx];
                let mut bestd = if best >= 0 {
                    l1(idx, best, w)
                } else {
                    i32::MAX
                };
                let l = (x >= k).then(|| idx - k);
                let r = (x + k < w).then(|| idx + k);
                let u = (y >= k).then(|| idx - k * w);
                let d = (y + k < h).then(|| idx + k * w);
                let order: [Option<usize>; 4] = match variant {
                    "ud-first" => [u, d, l, r],
                    "du-first" => [d, u, r, l],
                    _ => [l, r, u, d],
                };
                for tap in order.into_iter().flatten() {
                    let s = snap[tap];
                    if s >= 0 {
                        let dd = l1(idx, s, w);
                        let take = if strict_lt { dd < bestd } else { dd <= bestd };
                        if take {
                            bestd = dd;
                            best = s;
                        }
                    }
                }
                seed[idx] = best;
            }
        }
    }
    let snap_rgb: Vec<u8> = rgba.to_vec();
    for i in 0..n {
        if rgba[i * 4 + 3] == 0 && seed[i] >= 0 {
            let s = seed[i] as usize * 4;
            rgba[i * 4] = snap_rgb[s];
            rgba[i * 4 + 1] = snap_rgb[s + 1];
            rgba[i * 4 + 2] = snap_rgb[s + 2];
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let pairs: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(&a[0]).unwrap()).unwrap();
    let variant = a[1].clone();
    let limit: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let root = std::env::var("ABGEN_CONTENT_ROOT").unwrap();
    use sha1::{Digest, Sha1};

    let (mut ntex, mut byte_id, mut closer) = (0usize, 0usize, 0usize);
    let mut cur_id = 0usize;
    for pair in pairs.iter().take(limit) {
        let rp = pair["ref"].as_str().unwrap();
        let pid: i64 = pair["pid"].as_i64().unwrap();
        let fname = std::path::Path::new(rp)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let cid = fname
            .strip_suffix("_windows")
            .or_else(|| fname.strip_suffix("_mac"))
            .unwrap_or(fname);
        let mut hh = Sha1::new();
        hh.update(cid.as_bytes());
        let dg = hh.finalize();
        let hex: String = dg.iter().map(|b| format!("{b:02x}")).collect();
        let sp = format!("{root}/{}/{cid}", &hex[..4]);
        let Ok(bytes) = std::fs::read(&sp) else {
            continue;
        };
        let Ok(di) = image::load_from_memory(&bytes) else {
            continue;
        };
        let src = di.to_rgba8();
        let (sw, sh) = (src.width() as usize, src.height() as usize);
        let Ok(rb) = Bundle::load(std::path::Path::new(rp)) else {
            continue;
        };
        let rt = extract(&rb);
        let Some(r) = rt.get(&pid) else { continue };
        if r.fmt != 25 || (sw, sh) != (r.w, r.h) {
            continue;
        }
        ntex += 1;
        let (w, h) = (r.w, r.h);
        let srgb = r.color_space == 1;

        let mut cur_buf = src.as_raw().clone();
        bleed_variant(&mut cur_buf, w, h, "cur");
        let (cur_pay, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
            &cur_buf,
            w as u32,
            h as u32,
            Some(r.mips as i32),
            true,
            srgb,
            srgb,
            abgen::bc7_pure::Bc7Profile::Basic,
        );
        let cur_match = cur_pay == r.payload;
        if cur_match {
            cur_id += 1;
        }

        let mut buf = src.as_raw().clone();
        bleed_variant(&mut buf, w, h, &variant);
        let (pay, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
            &buf,
            w as u32,
            h as u32,
            Some(r.mips as i32),
            true,
            srgb,
            srgb,
            abgen::bc7_pure::Bc7Profile::Basic,
        );
        if pay == r.payload {
            byte_id += 1;
        }

        if !cur_match && pay.len() == r.payload.len() {
            let cur_diff = cur_pay
                .iter()
                .zip(&r.payload)
                .filter(|(a, b)| a != b)
                .count();
            let new_diff = pay.iter().zip(&r.payload).filter(|(a, b)| a != b).count();
            if new_diff < cur_diff {
                closer += 1;
            }
        }
    }
    println!(
        "variant={variant} textures={ntex}  cur-byte-id={cur_id}  variant-byte-id={byte_id}  closer-than-cur={closer}"
    );
}
