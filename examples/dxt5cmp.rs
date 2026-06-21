use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn extract_dxt5(path: &str) -> BTreeMap<String, (usize, usize, Vec<u8>)> {
    let b = Bundle::load(std::path::Path::new(path)).unwrap();
    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data));
        }
    }
    let mut out: BTreeMap<String, (usize, usize, Vec<u8>)> = BTreeMap::new();
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
            let fmt = gi(&v, "m_TextureFormat");
            if fmt != 12 {
                continue;
            }
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("tex")
                .to_string();
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(bts) if !bts.is_empty() => Some(bts.as_slice()),
                _ => None,
            });
            let payload: Vec<u8> = if let Some(d) = inline {
                d.to_vec()
            } else if let Some(sd) = v.get("m_StreamData") {
                let off = gi(sd, "offset") as usize;
                let size = gi(sd, "size") as usize;
                let p = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                let base = p.rsplit('/').next().unwrap_or(p);
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
            out.insert(name, (w, h, payload));
        }
    }
    out
}

fn main() {
    let ours = std::env::args().nth(1).expect("ours bundle");
    let refb = std::env::args().nth(2).expect("ref bundle");
    let a = extract_dxt5(&ours);
    let r = extract_dxt5(&refb);
    for (name, (w, h, rb)) in &r {
        match a.get(name) {
            None => println!("{name}: {w}x{h} ONLY-IN-REF (ours has no fmt=12 with this name)"),
            Some((_, _, ob)) => {
                if ob == rb {
                    println!("{name}: {w}x{h} len={} BYTE-IDENTICAL", rb.len());
                } else if ob.len() != rb.len() {
                    println!(
                        "{name}: {w}x{h} LEN-DIFF ours={} ref={}",
                        ob.len(),
                        rb.len()
                    );
                } else {
                    let blocks_w = w.div_ceil(4);
                    let blocks_h = h.div_ceil(4);
                    let mip0 = blocks_w * blocks_h * 16;
                    let mut total_diff_bytes = 0usize;
                    for i in 0..rb.len() {
                        if ob[i] != rb[i] {
                            total_diff_bytes += 1;
                        }
                    }
                    let mut mip0_diff_blocks = 0usize;
                    let m0 = mip0.min(rb.len());
                    for blk in 0..(m0 / 16) {
                        let s = blk * 16;
                        if ob[s..s + 16] != rb[s..s + 16] {
                            mip0_diff_blocks += 1;
                        }
                    }
                    let first = (0..rb.len()).find(|&i| ob[i] != rb[i]).unwrap_or(0);
                    println!(
                        "{name}: {w}x{h} len={} DIFF total_diff_bytes={} ({:.1}%) mip0_blocks={}/{} diff={} first@{}",
                        rb.len(),
                        total_diff_bytes,
                        100.0 * total_diff_bytes as f64 / rb.len() as f64,
                        mip0_diff_blocks,
                        blocks_w * blocks_h,
                        mip0_diff_blocks,
                        first
                    );
                }
            }
        }
    }
    for name in a.keys() {
        if !r.contains_key(name) {
            println!("{name}: ONLY-IN-OURS");
        }
    }
}
