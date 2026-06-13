use abgen::unity::bundle_file::{Bundle, FileContent};
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let b = Bundle::load(std::path::Path::new(&p)).unwrap();
    for f in &b.files {
        if let FileContent::Serialized(sf) = &f.content {
            for o in &sf.objects {
                if o.class_id == 74 {
                    let v = sf.read_typetree(o).unwrap();
                    let n = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
                    println!("{}\t{}", o.path_id, n);
                }
            }
        }
    }
}
