use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn main() {
    for path in std::env::args().skip(1) {
        let b = match Bundle::load(std::path::Path::new(&path)) {
            Ok(b) => b,
            Err(_) => {
                println!("{path}\tERR");
                continue;
            }
        };
        let mut ress: Vec<(String, Vec<u8>)> = Vec::new();
        for f in &b.files {
            if let FileContent::Raw(data) = &f.content {
                ress.push((f.name.clone(), data.clone()));
            }
        }
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
                let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("tex");
                let w = gi(&v, "m_Width");
                let h = gi(&v, "m_Height");
                let fmt = gi(&v, "m_TextureFormat");
                let cs = gi(&v, "m_ColorSpace");
                let payload: Vec<u8> = match v.get("image data") {
                    Some(Value::Bytes(bts)) if !bts.is_empty() => bts.clone(),
                    _ => match v.get("m_StreamData") {
                        Some(sd) => {
                            let off = gi(sd, "offset") as usize;
                            let size = gi(sd, "size") as usize;
                            let p = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                            let base = p.rsplit('/').next().unwrap_or(p);
                            match ress.iter().find(|(nm, _)| nm == base) {
                                Some((_, d)) if off + size <= d.len() => d[off..off + size].to_vec(),
                                _ => continue,
                            }
                        }
                        None => continue,
                    },
                };
                let mut hist = [0u64; 9];
                if fmt == 25 {
                    for blk in payload.chunks_exact(16) {
                        let m = blk[0].trailing_zeros().min(8) as usize;
                        hist[m] += 1;
                    }
                }
                let hs: Vec<String> = hist.iter().map(|c| c.to_string()).collect();
                println!("{path}\t{name}\t{w}x{h}\tfmt={fmt}\tcs={cs}\t{}", hs.join(","));
            }
        }
    }
}
