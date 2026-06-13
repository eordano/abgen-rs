use abgen::unity::bundle_file::Bundle;
use std::path::PathBuf;
fn main() {
    let p = PathBuf::from(std::env::args().nth(1).unwrap());
    let b = Bundle::load(&p).expect("load");
    let sf = b.serialized().expect("sf");
    for o in &sf.objects {
        if o.class_id != 28 {
            continue;
        }
        let v = sf.read_typetree(o).unwrap();
        let m = v.as_map().unwrap();
        let g = |k: &str| m.get(k).and_then(|x| x.as_i64()).unwrap_or(-1);
        println!(
            "name={:<12} fmt={:<3} mips={:<3} cs={} w={} h={} pid={}",
            m.get("m_Name").and_then(|x| x.as_str()).unwrap_or(""),
            g("m_TextureFormat"),
            g("m_MipCount"),
            g("m_ColorSpace"),
            g("m_Width"),
            g("m_Height"),
            o.path_id
        );
    }
}
