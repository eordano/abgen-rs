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
    cs: i64,
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
                    fmt: gi(&v, "m_TextureFormat"),
                    mips: gi(&v, "m_MipCount"),
                    cs: gi(&v, "m_ColorSpace"),
                    payload,
                },
            );
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
    let (mut ntex, mut cur_id, mut oracle_id, mut oracle_helps) = (0usize, 0usize, 0usize, 0usize);
    let mut sum_cur = 0u64;
    let mut sum_oracle = 0u64;
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
        let (w, h) = (r.w, r.h);
        let srgb = r.cs == 1;
        let blk = w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 16;
        if r.payload.len() < blk {
            continue;
        }
        let rd = dec(&r.payload[..blk], w, h);
        ntex += 1;

        let mut cur = src.as_raw().clone();
        abgen::alpha_bleed::alpha_bleed_inplace(&mut cur, w as u32, h as u32);
        let (cp, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
            &cur,
            w as u32,
            h as u32,
            Some(r.mips as i32),
            true,
            srgb,
            srgb,
            abgen::bc7_pure::Bc7Profile::Basic,
        );

        let mut orc = cur.clone();
        for py in 0..h {
            for px in 0..w {
                let sy = h - 1 - py;
                let si = (sy * w + px) * 4;
                let pi = (py * w + px) * 4;
                if src.as_raw()[si + 3] == 0 {
                    orc[si] = rd[pi];
                    orc[si + 1] = rd[pi + 1];
                    orc[si + 2] = rd[pi + 2];
                }
            }
        }
        let (op, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
            &orc,
            w as u32,
            h as u32,
            Some(r.mips as i32),
            true,
            srgb,
            srgb,
            abgen::bc7_pure::Bc7Profile::Basic,
        );
        let dc = cp.iter().zip(&r.payload).filter(|(a, b)| a != b).count();
        let dorc = op.iter().zip(&r.payload).filter(|(a, b)| a != b).count();
        sum_cur += dc as u64;
        sum_oracle += dorc as u64;
        if dc == 0 {
            cur_id += 1;
        }
        if dorc == 0 {
            oracle_id += 1;
        }
        if dorc < dc {
            oracle_helps += 1;
        }
    }
    println!("textures={ntex}");
    println!("  current bleed byte-id: {cur_id}");
    println!(
        "  PERFECT-oracle bleed byte-id: {oracle_id}   (oracle reduces diff on {oracle_helps})"
    );
    println!("  total differing bytes: current={sum_cur}  oracle={sum_oracle}  (bleed-attributable reduction: {})",sum_cur.saturating_sub(sum_oracle));
}
