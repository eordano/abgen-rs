use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn decode_blocks(
    data: &[u8],
    w: usize,
    h: usize,
    out: &mut [u32],
    f: fn(&[u8], usize, usize, &mut [u32]) -> Result<(), &'static str>,
) -> bool {
    f(data, w, h, out).is_ok()
}

fn decode_mip0(b: &Bundle) -> BTreeMap<i64, (String, usize, usize, i64, Vec<u8>)> {
    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data));
        }
    }
    let mut map = BTreeMap::new();
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for obj in &sf.objects {
            if obj.class_id != 28 {
                continue;
            }
            let v = match sf.read_typetree(obj) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("tex")
                .to_string();
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let fmt = gi(&v, "m_TextureFormat");
            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(bts) if !bts.is_empty() => Some(bts.as_slice()),
                _ => None,
            });
            let stream_buf;
            let payload: &[u8] = if let Some(d) = inline {
                d
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
                stream_buf = data[off..off + size].to_vec();
                &stream_buf
            } else {
                continue;
            };
            if w == 0 || h == 0 {
                continue;
            }
            let mut px = vec![0u32; w * h];
            let ok = match fmt {
                25 => decode_blocks(payload, w, h, &mut px, texture2ddecoder::decode_bc7),
                29 => decode_blocks(payload, w, h, &mut px, texture2ddecoder::decode_bc5),
                10 => decode_blocks(payload, w, h, &mut px, texture2ddecoder::decode_bc1),
                12 => decode_blocks(payload, w, h, &mut px, texture2ddecoder::decode_bc3),
                _ => false,
            };
            let mut rgba = vec![0u8; w * h * 4];
            if ok {
                for i in 0..w * h {
                    let p = px[i];
                    rgba[i * 4] = ((p >> 16) & 0xff) as u8;
                    rgba[i * 4 + 1] = ((p >> 8) & 0xff) as u8;
                    rgba[i * 4 + 2] = (p & 0xff) as u8;
                    rgba[i * 4 + 3] = ((p >> 24) & 0xff) as u8;
                }
            } else if fmt == 4 {
                if payload.len() >= w * h * 4 {
                    rgba.copy_from_slice(&payload[..w * h * 4]);
                } else {
                    continue;
                }
            } else if fmt == 5 {
                if payload.len() >= w * h * 4 {
                    for i in 0..w * h {
                        rgba[i * 4] = payload[i * 4 + 1];
                        rgba[i * 4 + 1] = payload[i * 4 + 2];
                        rgba[i * 4 + 2] = payload[i * 4 + 3];
                        rgba[i * 4 + 3] = payload[i * 4];
                    }
                } else {
                    continue;
                }
            } else if fmt == 3 {
                if payload.len() >= w * h * 3 {
                    for i in 0..w * h {
                        rgba[i * 4] = payload[i * 3];
                        rgba[i * 4 + 1] = payload[i * 3 + 1];
                        rgba[i * 4 + 2] = payload[i * 3 + 2];
                        rgba[i * 4 + 3] = 255;
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            }
            map.insert(obj.path_id, (name, w, h, fmt, rgba));
        }
    }
    map
}

fn main() {
    let ours = std::env::args().nth(1).expect("ours");
    let refp = std::env::args().nth(2).expect("ref");
    let bo = match Bundle::load(std::path::Path::new(&ours)) {
        Ok(b) => b,
        Err(_) => {
            println!("ERR load ours");
            return;
        }
    };
    let br = match Bundle::load(std::path::Path::new(&refp)) {
        Ok(b) => b,
        Err(_) => {
            println!("ERR load ref");
            return;
        }
    };
    let mo = decode_mip0(&bo);
    let mr = decode_mip0(&br);
    for (pid, (name, w, h, fmt, ro)) in &mr {
        let Some((_, ow, oh, _ofmt, oo)) = mo.get(pid) else {
            continue;
        };
        if ow != w || oh != h {
            println!("{name} DIMMISMATCH ours={ow}x{oh} ref={w}x{h}");
            continue;
        }
        let n = w * h;

        let mut sum = 0u64;
        let mut maxd = 0u32;
        let mut gt8 = 0u64;
        let mut gt32 = 0u64;
        let mut asum = 0u64;
        let mut amax = 0u32;
        let mut diffs: Vec<u32> = Vec::with_capacity(n * 3);
        for i in 0..n {
            let ar = ro[i * 4 + 3] as i32;
            let ao = oo[i * 4 + 3] as i32;
            let wgt = ar.min(ao);
            let ad = (ar - ao).unsigned_abs();
            asum += ad as u64;
            if ad > amax {
                amax = ad;
            }
            for c in 0..3 {
                let a = ro[i * 4 + c] as i32;
                let b = oo[i * 4 + c] as i32;

                let d = (((a - b).abs() * wgt) / 255) as u32;
                sum += d as u64;
                if d > maxd {
                    maxd = d;
                }
                if d > 8 {
                    gt8 += 1;
                }
                if d > 32 {
                    gt32 += 1;
                }
                diffs.push(d);
            }
        }
        diffs.sort_unstable();
        let p99 = diffs[(diffs.len() * 99 / 100).min(diffs.len() - 1)];
        let mean = sum as f64 / (n * 3) as f64;
        let amean = asum as f64 / n as f64;
        println!("{name} {w}x{h} fmt={fmt} mean={mean:.3} max={maxd} p99={p99} gt8={:.4} gt32={:.4} amean={amean:.3} amax={amax}",
            gt8 as f64/(n*3) as f64, gt32 as f64/(n*3) as f64);
    }
}
