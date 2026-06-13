use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::path::PathBuf;
fn main() {
    let p = PathBuf::from(std::env::args().nth(1).unwrap());
    let pid: i64 = std::env::args().nth(2).unwrap().parse().unwrap();
    let out = PathBuf::from(std::env::args().nth(3).unwrap());
    let b = Bundle::load(&p).expect("load");
    let sf = b.serialized().expect("sf");
    let o = sf.objects.iter().find(|o| o.path_id == pid).expect("pid");
    let v = sf.read_typetree(o).unwrap();
    let m = v.as_map().unwrap();
    if let Some(Value::Bytes(d)) = m.get("image data") {
        if !d.is_empty() {
            std::fs::write(&out, d).unwrap();
            println!("inline {} bytes", d.len());
            return;
        }
    }
    let sd = m
        .get("m_StreamData")
        .and_then(|x| x.as_map())
        .expect("stream");
    let off = sd.get("offset").and_then(|x| x.as_i64()).unwrap() as usize;
    let size = sd.get("size").and_then(|x| x.as_i64()).unwrap() as usize;
    let path = sd.get("path").and_then(|x| x.as_str()).unwrap();
    let name = path.rsplit('/').next().unwrap();
    let e = b.files.iter().find(|e| e.name == name).expect("ress");
    let data = match &e.content {
        FileContent::Raw(d) => d.clone(),
        _ => panic!("non-raw ress"),
    };
    std::fs::write(&out, &data[off..off + size]).unwrap();
    println!("ress {} bytes from {}", size, name);
}
