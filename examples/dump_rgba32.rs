use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(-1)
}

fn main() {
    let path = std::env::args().nth(1).expect("bundle");
    let b = Bundle::load(std::path::Path::new(&path)).unwrap();
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
            if fmt != 4 && fmt != 5 {
                continue;
            }
            let bytes: Vec<u8> = match v.get("image data") {
                Some(Value::Bytes(bts)) if !bts.is_empty() => bts.clone(),
                _ => {
                    eprintln!("no inline image data");
                    return;
                }
            };

            let stride = w * 4;
            for y in 0..h {
                let row = h - 1 - y;
                let off = row * stride;
                let mut s = String::new();
                for x in 0..w {
                    let p = off + x * 4;
                    s += &format!(
                        "{:3},{:3},{:3},{:3} | ",
                        bytes[p],
                        bytes[p + 1],
                        bytes[p + 2],
                        bytes[p + 3]
                    );
                }
                println!("y{y:3} {s}");
            }
            return;
        }
    }
}
