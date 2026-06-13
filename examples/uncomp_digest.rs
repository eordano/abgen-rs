use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(-1)
}

fn main() {
    let path = std::env::args().nth(1).expect("bundle");
    let b = match Bundle::load(std::path::Path::new(&path)) {
        Ok(b) => b,
        Err(_) => return,
    };
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
            if fmt != 3 && fmt != 4 && fmt != 5 {
                continue;
            }
            let w = gi(&v, "m_Width");
            let h = gi(&v, "m_Height");
            let mips = gi(&v, "m_MipCount");
            let bytes: &[u8] = match v.get("image data") {
                Some(Value::Bytes(bts)) => bts,
                _ => &[],
            };
            let mut hsh = DefaultHasher::new();
            bytes.hash(&mut hsh);
            let digest = hsh.finish();
            println!(
                "{} {} {} {} {} {} {:016x}",
                obj.path_id,
                w,
                h,
                fmt,
                mips,
                bytes.len(),
                digest
            );
        }
    }
}
