use abgen::unity::bundle_file::{Bundle, FileContent};
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let b = Bundle::load(std::path::Path::new(&p)).unwrap();
    for f in &b.files {
        if let FileContent::Serialized(sf) = &f.content {
            for o in &sf.objects {
                if o.class_id != 74 {
                    continue;
                }
                let v = sf.read_typetree(o).unwrap();
                let n = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
                let stored = v
                    .get("m_MuscleClipSize")
                    .and_then(|x| x.as_i64())
                    .unwrap_or(-1);
                let node = sf.types[o.type_id as usize].node.as_ref().unwrap();
                let mc_node = node
                    .m_Children
                    .iter()
                    .find(|c| c.m_Name == "m_MuscleClip")
                    .unwrap();
                let mc_val = v.get("m_MuscleClip").unwrap();
                let bytes = abgen::unity::write_typetree(mc_val, mc_node, sf.big_endian);
                println!(
                    "{}\t{}\tstored={}\treser={}\tobj={}",
                    o.path_id,
                    n,
                    stored,
                    bytes.len(),
                    o.data.len()
                );
            }
        }
    }
}
