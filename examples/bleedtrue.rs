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
            out.insert(o.path_id, Tex { w, h, fmt, payload });
        }
    }
    out
}
fn dec(data: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut px = vec![0u32; w * h];
    texture2ddecoder::decode_bc7(data, w, h, &mut px).unwrap();
    let mut r = vec![0u8; w * h * 4];
    for (i, p) in px.iter().enumerate() {
        let [b, g, rr, a] = p.to_le_bytes();
        r[i * 4] = rr;
        r[i * 4 + 1] = g;
        r[i * 4 + 2] = b;
        r[i * 4 + 3] = a;
    }
    r
}
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let pairs: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(&a[0]).unwrap()).unwrap();
    let limit: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let root = std::env::var("ABGEN_CONTENT_ROOT").unwrap();
    use sha1::{Digest, Sha1};
    let (mut ntex, mut true_wrong, mut alpha_edge_wrong, mut tex_clean) =
        (0usize, 0u64, 0u64, 0usize);
    for pair in pairs.iter().take(limit) {
        let op = pair["ours"].as_str().unwrap();
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
        if o.fmt != 25 || (sw, sh) != (o.w, o.h) {
            continue;
        }
        let (w, h) = (o.w, o.h);
        let blk = w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 16;
        if o.payload.len() < blk || r.payload.len() < blk {
            continue;
        }
        let od = dec(&o.payload[..blk], w, h);
        let rd = dec(&r.payload[..blk], w, h);
        let raw = src.as_raw();
        ntex += 1;
        let mut tw = 0u64;
        for py in 0..h {
            for px in 0..w {
                let pi = py * w + px;
                let dr = (od[pi * 4] as i32 - rd[pi * 4] as i32).abs();
                let dg = (od[pi * 4 + 1] as i32 - rd[pi * 4 + 1] as i32).abs();
                let db = (od[pi * 4 + 2] as i32 - rd[pi * 4 + 2] as i32).abs();
                if dr.max(dg).max(db) <= 8 {
                    continue;
                }

                let si = ((h - 1 - py) * w + px) * 4;
                let salpha = raw[si + 3];
                if salpha == 0 {
                    tw += 1;
                    true_wrong += 1;
                } else {
                    alpha_edge_wrong += 1;
                }
            }
        }
        if tw == 0 {
            tex_clean += 1;
        }
    }
    println!("textures={ntex} clean(no true-transparent wrong)={tex_clean}");
    println!("genuine transparent-bleed wrong (source alpha==0, >8): {true_wrong}");
    println!("alpha-edge wrong (source alpha>0 but decoded differs, >8 = BC7 edge encoder): {alpha_edge_wrong}");
}
