use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

struct Tex {
    name: String,
    w: usize,
    h: usize,
    fmt: i64,
    cs: i64,
    mips: i64,
    wrap_u: i64,
    wrap_v: i64,
    filter: i64,
    aniso: i64,
    rgba: Option<Vec<u8>>,
}

type DecodeFn = fn(&[u8], usize, usize, &mut [u32]) -> Result<(), &'static str>;

fn decode_blocks(data: &[u8], w: usize, h: usize, out: &mut [u32], f: DecodeFn) -> bool {
    f(data, w, h, out).is_ok()
}

fn read_texes(b: &Bundle) -> BTreeMap<i64, Tex> {
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
            let Ok(v) = sf.read_typetree(obj) else {
                continue;
            };
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("tex")
                .to_string();
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let fmt = gi(&v, "m_TextureFormat");
            let cs = gi(&v, "m_ColorSpace");
            let mips = gi(&v, "m_MipCount").max(gi(&v, "m_MipMap"));
            let ts = v.get("m_TextureSettings");
            let g = |k: &str| ts.map(|t| gi(t, k)).unwrap_or(-1);
            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(bts) if !bts.is_empty() => Some(bts.as_slice()),
                _ => None,
            });
            let stream_buf;
            let payload: Option<&[u8]> = if let Some(d) = inline {
                Some(d)
            } else if let Some(sd) = v.get("m_StreamData") {
                let off = gi(sd, "offset") as usize;
                let size = gi(sd, "size") as usize;
                let path = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                let base = path.rsplit('/').next().unwrap_or(path);
                if let Some((_, data)) = ress.iter().find(|(nm, _)| nm == base) {
                    if off + size <= data.len() {
                        stream_buf = data[off..off + size].to_vec();
                        Some(Box::leak(stream_buf.into_boxed_slice()) as &[u8])
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            let mut rgba = None;
            if let Some(p) = payload {
                if w > 0 && h > 0 {
                    let mut px = vec![0u32; w * h];
                    let ok = match fmt {
                        25 => decode_blocks(p, w, h, &mut px, texture2ddecoder::decode_bc7),
                        29 => decode_blocks(p, w, h, &mut px, texture2ddecoder::decode_bc5),
                        10 => decode_blocks(p, w, h, &mut px, texture2ddecoder::decode_bc1),
                        12 => decode_blocks(p, w, h, &mut px, texture2ddecoder::decode_bc3),
                        _ => false,
                    };
                    let mut buf = vec![0u8; w * h * 4];
                    if ok {
                        for i in 0..w * h {
                            let pp = px[i];
                            buf[i * 4] = ((pp >> 16) & 0xff) as u8;
                            buf[i * 4 + 1] = ((pp >> 8) & 0xff) as u8;
                            buf[i * 4 + 2] = (pp & 0xff) as u8;
                            buf[i * 4 + 3] = ((pp >> 24) & 0xff) as u8;
                        }
                        rgba = Some(buf);
                    } else if fmt == 4 && p.len() >= w * h * 4 {
                        buf.copy_from_slice(&p[..w * h * 4]);
                        rgba = Some(buf);
                    } else if fmt == 5 && p.len() >= w * h * 4 {
                        for i in 0..w * h {
                            buf[i * 4] = p[i * 4 + 1];
                            buf[i * 4 + 1] = p[i * 4 + 2];
                            buf[i * 4 + 2] = p[i * 4 + 3];
                            buf[i * 4 + 3] = p[i * 4];
                        }
                        rgba = Some(buf);
                    } else if fmt == 3 && p.len() >= w * h * 3 {
                        for i in 0..w * h {
                            buf[i * 4] = p[i * 3];
                            buf[i * 4 + 1] = p[i * 3 + 1];
                            buf[i * 4 + 2] = p[i * 3 + 2];
                            buf[i * 4 + 3] = 255;
                        }
                        rgba = Some(buf);
                    }
                }
            }
            map.insert(
                obj.path_id,
                Tex {
                    name,
                    w,
                    h,
                    fmt,
                    cs,
                    mips,
                    wrap_u: g("m_WrapU"),
                    wrap_v: g("m_WrapV"),
                    filter: g("m_FilterMode"),
                    aniso: g("m_Aniso"),
                    rgba,
                },
            );
        }
    }
    map
}

fn main() {
    let ours = std::env::args().nth(1).expect("ours");
    let refp = std::env::args().nth(2).expect("ref");
    let byte_id = match (std::fs::read(&ours), std::fs::read(&refp)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    let bo = match Bundle::load(std::path::Path::new(&ours)) {
        Ok(b) => b,
        Err(_) => {
            println!("ERR ours");
            return;
        }
    };
    let br = match Bundle::load(std::path::Path::new(&refp)) {
        Ok(b) => b,
        Err(_) => {
            println!("ERR ref");
            return;
        }
    };
    let to = read_texes(&bo);
    let tr = read_texes(&br);

    let mut struct_diff = to.len() != tr.len();
    let mut hdr_fields: Vec<&str> = Vec::new();
    let mut pxmax = 0i64;
    let mut amax = 0i64;
    let mut wmax = 0i64;
    let mut wsum = 0f64;
    let mut wcnt = 0u64;
    let mut wgt8 = 0u64;
    let mut undecoded = false;
    let mut dimmis = false;

    let by_name_r: BTreeMap<&str, &Tex> = tr.values().map(|t| (t.name.as_str(), t)).collect();
    for (pid, o) in &to {
        let r = tr.get(pid).or_else(|| by_name_r.get(o.name.as_str()).copied());
        let Some(r) = r else {
            struct_diff = true;
            continue;
        };
        if o.fmt != r.fmt && !hdr_fields.contains(&"format") {
            hdr_fields.push("format");
        }
        if o.cs != r.cs && !hdr_fields.contains(&"colorspace") {
            hdr_fields.push("colorspace");
        }
        if o.mips != r.mips && !hdr_fields.contains(&"mips") {
            hdr_fields.push("mips");
        }
        if (o.wrap_u != r.wrap_u || o.wrap_v != r.wrap_v) && !hdr_fields.contains(&"wrap") {
            hdr_fields.push("wrap");
        }
        if o.filter != r.filter && !hdr_fields.contains(&"filter") {
            hdr_fields.push("filter");
        }
        if o.aniso != r.aniso && !hdr_fields.contains(&"aniso") {
            hdr_fields.push("aniso");
        }
        if o.w != r.w || o.h != r.h {
            dimmis = true;
            continue;
        }
        match (&o.rgba, &r.rgba) {
            (Some(a), Some(b)) if a.len() == b.len() => {
                for i in 0..a.len() / 4 {
                    let wgt = (a[i * 4 + 3] as i64).min(b[i * 4 + 3] as i64);
                    for c in 0..3 {
                        let d = (a[i * 4 + c] as i64 - b[i * 4 + c] as i64).abs();
                        if d > pxmax {
                            pxmax = d;
                        }
                        let wd = d * wgt / 255;
                        if wd > wmax {
                            wmax = wd;
                        }
                        wsum += wd as f64;
                        wcnt += 1;
                        if wd > 8 {
                            wgt8 += 1;
                        }
                    }
                    let da = (a[i * 4 + 3] as i64 - b[i * 4 + 3] as i64).abs();
                    if da > amax {
                        amax = da;
                    }
                }
            }
            _ => undecoded = true,
        }
    }

    let wmean = if wcnt > 0 { wsum / wcnt as f64 } else { 0.0 };
    let wfrac8 = if wcnt > 0 {
        wgt8 as f64 / wcnt as f64
    } else {
        0.0
    };
    println!(
        "byte={} struct={} ntex={} hdr={} pxmax={} amax={} wmax={} wmean={:.4} wfrac8={:.6} undecoded={} dim={}",
        byte_id as u8,
        struct_diff as u8,
        to.len(),
        if hdr_fields.is_empty() {
            "none".to_string()
        } else {
            hdr_fields.join(",")
        },
        pxmax,
        amax,
        wmax,
        wmean,
        wfrac8,
        undecoded as u8,
        dimmis as u8,
    );
}
