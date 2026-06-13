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
            let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
            let w = gi(&v, "m_Width");
            let h = gi(&v, "m_Height");
            let fmt = gi(&v, "m_TextureFormat");
            let mips = gi(&v, "m_MipCount");
            let cis = gi(&v, "m_CompleteImageSize");
            let sd = v.get("m_StreamData");
            let sdsize = sd.map(|s| gi(s, "size")).unwrap_or(-1);
            println!(
                "pid={:>22} {:>5}x{:<5} fmt={:<3} mips={:<3} completeImg={:>9} streamSize={:>9} {}",
                obj.path_id, w, h, fmt, mips, cis, sdsize, name
            );
        }
    }
}
