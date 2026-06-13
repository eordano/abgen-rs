use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::unity::typetree::write_typetree;
fn main() {
    for p in std::env::args().skip(1) {
        let b = Bundle::load(std::path::Path::new(&p)).unwrap();
        for f in &b.files {
            if let FileContent::Serialized(sf) = &f.content {
                for o in &sf.objects {
                    if o.class_id != 91 {
                        continue;
                    }
                    let st = &sf.types[o.type_id as usize];
                    let node = st.node.as_ref().unwrap();
                    let v = sf.read_typetree(o).unwrap();
                    let stored = v.get("m_ControllerSize").and_then(|x| x.as_i64()).unwrap();
                    let cnode = node
                        .m_Children
                        .iter()
                        .find(|c| c.m_Name == "m_Controller")
                        .unwrap();
                    let cval = v.get("m_Controller").unwrap();
                    let len = write_typetree(cval, cnode, false).len() as i64;
                    println!(
                        "{}\tstored={}\tserialized={}\tdelta={}",
                        p.rsplit('/').next().unwrap(),
                        stored,
                        len,
                        stored - len
                    );
                }
            }
        }
    }
}
