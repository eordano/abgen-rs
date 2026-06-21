use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn decode_mip0(b: &Bundle) -> Option<(usize, usize, Vec<u8>)> {
    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data));
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
            let v = match sf.read_typetree(obj) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let fmt = gi(&v, "m_TextureFormat");
            if w == 0 || h == 0 {
                continue;
            }
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
            let mut px = vec![0u32; w * h];
            let ok = match fmt {
                25 => texture2ddecoder::decode_bc7(payload, w, h, &mut px).is_ok(),
                _ => false,
            };
            if !ok {
                continue;
            }
            let mut rgba = vec![0u8; w * h * 4];
            for i in 0..w * h {
                let p = px[i];
                rgba[i * 4] = ((p >> 16) & 0xff) as u8;
                rgba[i * 4 + 1] = ((p >> 8) & 0xff) as u8;
                rgba[i * 4 + 2] = (p & 0xff) as u8;
                rgba[i * 4 + 3] = ((p >> 24) & 0xff) as u8;
            }
            return Some((w, h, rgba));
        }
    }
    None
}

fn main() {
    let ours = std::env::args().nth(1).expect("ours");
    let refp = std::env::args().nth(2).expect("ref");
    let bo = Bundle::load(std::path::Path::new(&ours)).expect("load ours");
    let br = Bundle::load(std::path::Path::new(&refp)).expect("load ref");
    let (w, h, oo) = decode_mip0(&bo).expect("ours mip0");
    let (w2, h2, ro) = decode_mip0(&br).expect("ref mip0");
    assert_eq!((w, h), (w2, h2), "dim mismatch");
    let n = w * h;

    let mut buckets: Vec<Vec<u16>> = vec![Vec::new(); 256];
    for i in 0..n {
        let ar = ro[i * 4 + 3];
        let ao = oo[i * 4 + 3];
        if ar < 250 || ao < 250 {
            continue;
        }
        for c in 0..3 {
            let inv = oo[i * 4 + c] as usize;
            buckets[inv].push(ro[i * 4 + c] as u16);
        }
    }
    println!("# in  count  median  mean  min  max");
    for (inv, b) in buckets.iter_mut().enumerate() {
        if b.is_empty() {
            continue;
        }
        b.sort_unstable();
        let med = b[b.len() / 2];
        let mean: f64 = b.iter().map(|&x| x as f64).sum::<f64>() / b.len() as f64;
        let min = b[0];
        let max = b[b.len() - 1];
        println!("{inv} {} {med} {mean:.2} {min} {max}", b.len());
    }
}
