use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn extract(bundle: &Bundle) -> BTreeMap<i64, (String, usize, usize, i64, Vec<u8>)> {
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
            if gi(&v, "m_TextureFormat") != 25 {
                continue;
            }
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("tex")
                .to_string();
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let mips = gi(&v, "m_MipCount");
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
            out.insert(obj.path_id, (name, w, h, mips, payload));
        }
    }
    out
}

fn decode_mip(blocks: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    let pw = (w + 3) & !3;
    let ph = (h + 3) & !3;
    let mut px = vec![0u32; pw * ph];
    texture2ddecoder::decode_bc7(blocks, pw, ph, &mut px).ok()?;
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let [b, g, r, a] = px[y * pw + x].to_le_bytes();
            let d = (y * w + x) * 4;
            rgba[d] = r;
            rgba[d + 1] = g;
            rgba[d + 2] = b;
            rgba[d + 3] = a;
        }
    }
    Some(rgba)
}

struct MipSlice {
    level: usize,
    w: usize,
    h: usize,
    byte_off: usize,
    byte_len: usize,
}

fn mip_layout(w: usize, h: usize, mips: i64) -> Vec<MipSlice> {
    let mut v = Vec::new();
    let (mut mw, mut mh) = (w, h);
    let mut off = 0usize;
    for m in 0..mips as usize {
        let pw = mw.div_ceil(4).max(1);
        let ph = mh.div_ceil(4).max(1);
        let len = pw * ph * 16;
        v.push(MipSlice {
            level: m,
            w: mw,
            h: mh,
            byte_off: off,
            byte_len: len,
        });
        off += len;
        mw = (mw / 2).max(1);
        mh = (mh / 2).max(1);
    }
    v
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tsv = args.iter().any(|a| a == "--tsv");
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let ours = match Bundle::load(std::path::Path::new(paths[0])) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("load ours failed: {e}");
            return;
        }
    };
    let refb = match Bundle::load(std::path::Path::new(paths[1])) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("load ref failed: {e}");
            return;
        }
    };
    let op = extract(&ours);
    let rp = extract(&refb);

    for (pid, (name, w, h, mips, opay)) in &op {
        let Some((_rn, rw, rh, rmips, rpay)) = rp.get(pid) else {
            continue;
        };
        if w != rw || h != rh || mips != rmips || opay.len() != rpay.len() {
            continue;
        }
        let layout = mip_layout(*w, *h, *mips);
        let mut first_byte_diff: i64 = -1;
        for ms in &layout {
            let oa = &opay[ms.byte_off..ms.byte_off + ms.byte_len];
            let ra = &rpay[ms.byte_off..ms.byte_off + ms.byte_len];
            if oa != ra {
                first_byte_diff = ms.level as i64;
                break;
            }
        }
        if first_byte_diff < 0 {
            continue;
        }
        let mut first_pix_diff: i64 = -1;
        let mut permip: Vec<(usize, [u32; 4], usize, usize)> = Vec::new();
        for ms in &layout {
            let oa = &opay[ms.byte_off..ms.byte_off + ms.byte_len];
            let ra = &rpay[ms.byte_off..ms.byte_off + ms.byte_len];
            let (Some(od), Some(rd)) = (decode_mip(oa, ms.w, ms.h), decode_mip(ra, ms.w, ms.h))
            else {
                permip.push((ms.level, [999, 999, 999, 999], 0, ms.w * ms.h));
                continue;
            };
            let mut maxd = [0u32; 4];
            let mut ndiff = 0usize;
            for i in 0..(ms.w * ms.h) {
                let mut any = false;
                for c in 0..4 {
                    let d = (od[i * 4 + c] as i32 - rd[i * 4 + c] as i32).unsigned_abs();
                    if d > maxd[c] {
                        maxd[c] = d;
                    }
                    if d != 0 {
                        any = true;
                    }
                }
                if any {
                    ndiff += 1;
                }
            }
            if ndiff > 0 && first_pix_diff < 0 {
                first_pix_diff = ms.level as i64;
            }
            permip.push((ms.level, maxd, ndiff, ms.w * ms.h));
        }

        if tsv {
            let mips_s: Vec<String> = permip
                .iter()
                .map(|(l, md, nd, tot)| {
                    format!("m{l}:{nd}/{tot}:{}/{}/{}/{}", md[0], md[1], md[2], md[3])
                })
                .collect();
            println!(
                "{pid}\t{name}\t{w}x{h}\tmips={mips}\tfbd={first_byte_diff}\tfpd={first_pix_diff}\t{}",
                mips_s.join(" ")
            );
        } else {
            println!(
                "pid={pid} {name} {w}x{h} mips={mips} first_byte_diff_mip={first_byte_diff} first_pixel_diff_mip={first_pix_diff}"
            );
            for (l, md, nd, tot) in &permip {
                if *nd > 0 || first_byte_diff == *l as i64 {
                    println!(
                        "  m{l}: pix_diff {nd}/{tot}  maxd R={} G={} B={} A={}",
                        md[0], md[1], md[2], md[3]
                    );
                }
            }
        }
    }
}
