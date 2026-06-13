use abgen::unity::bundle_file::{Bundle, FileContent};
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let b = Bundle::load(std::path::Path::new(&p)).unwrap();
    for f in &b.files {
        if let FileContent::Serialized(sf) = &f.content {
            for o in &sf.objects {
                if o.class_id == 91 {
                    let v = sf.read_typetree(o).unwrap();
                    let tos = v.get("m_TOS").and_then(|x| x.as_array()).unwrap();
                    for e in tos {
                        if let Some(a) = e.as_array() {
                            let h = a[0].as_i64().unwrap();
                            let n = a[1].as_str().unwrap_or("");
                            println!("{}\t{}", h as u32, n);
                        } else {
                            let h = e.get("first").and_then(|x| x.as_i64()).unwrap();
                            let n = e.get("second").and_then(|x| x.as_str()).unwrap_or("");
                            println!("{}\t{}", h as u32, n);
                        }
                    }
                }
            }
        }
    }
}
