use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::io::Write;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn main() {
    let path = std::env::args().nth(1).expect("bundle");
    let out = std::env::args().nth(2).expect("out file");
    let b = Bundle::load(std::path::Path::new(&path)).unwrap();
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
            let payload: Vec<u8> = match v.get("image data") {
                Some(Value::Bytes(bts)) if !bts.is_empty() => bts.clone(),
                _ => {
                    let sd = v.get("m_StreamData").expect("no stream data");
                    let off = gi(sd, "offset") as usize;
                    let size = gi(sd, "size") as usize;
                    let p = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                    let base = p.rsplit('/').next().unwrap_or(p);
                    let (_, d) = ress
                        .iter()
                        .find(|(nm, _)| nm == base)
                        .expect("stream file missing");
                    d[off..off + size].to_vec()
                }
            };
            eprintln!(
                "{} {}x{} fmt={} payload {} bytes (offset info: {:?})",
                v.get("m_Name").and_then(|x| x.as_str()).unwrap_or(""),
                gi(&v, "m_Width"),
                gi(&v, "m_Height"),
                gi(&v, "m_TextureFormat"),
                payload.len(),
                v.get("m_StreamData").map(|sd| (gi(sd, "offset"), gi(sd, "size")))
            );
            let mut fo = std::fs::File::create(&out).unwrap();
            fo.write_all(&payload).unwrap();
            return;
        }
    }
    eprintln!("no Texture2D");
    std::process::exit(1);
}
