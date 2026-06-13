use abgen::animation_mecanim::binding_tie_audit;
use abgen::hashes::crc32;
use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::HashSet;

fn g<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for k in path {
        cur = cur.get(k)?;
    }
    Some(cur)
}

fn main() {
    let glb = std::env::args().nth(1).unwrap();
    let refp = std::env::args().nth(2).unwrap();
    let glb_bytes = std::fs::read(&glb).unwrap();
    let rb = Bundle::load(std::path::Path::new(&refp)).unwrap();

    let mut ref_collapsed: std::collections::HashMap<String, HashSet<(i64, i64)>> =
        std::collections::HashMap::new();
    for f in &rb.files {
        if let FileContent::Serialized(sf) = &f.content {
            for o in &sf.objects {
                if o.class_id != 74 {
                    continue;
                }
                let v = sf.read_typetree(o).unwrap();
                let name = v
                    .get("m_Name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let streamed_cc = g(
                    &v,
                    &[
                        "m_MuscleClip",
                        "m_Clip",
                        "data",
                        "m_StreamedClip",
                        "curveCount",
                    ],
                )
                .and_then(|x| x.as_i64())
                .unwrap_or(0);
                let gb = match g(&v, &["m_ClipBindingConstant", "genericBindings"])
                    .and_then(|x| x.as_array())
                {
                    Some(a) => a,
                    None => continue,
                };
                let mut set = HashSet::new();
                let mut ci: i64 = 0;
                for gbi in gb {
                    let attr = gbi.get("attribute").and_then(|x| x.as_i64()).unwrap_or(-1);
                    let pcrc = gbi.get("path").and_then(|x| x.as_i64()).unwrap_or(-1);
                    let dim = if attr == 2 { 4 } else { 3 };

                    if ci >= streamed_cc {
                        set.insert((pcrc, attr));
                    }
                    ci += dim;
                }
                ref_collapsed.insert(name, set);
            }
        }
    }

    for (clip, rows) in binding_tie_audit(&glb_bytes) {
        let refset = ref_collapsed.get(&clip);
        for (path, attr, is_step, _dim, our, vbits, sbits) in rows {
            let pcrc = crc32(path.as_bytes()) as i64;
            let ref_collapse = refset.map(|s| s.contains(&(pcrc, attr))).unwrap_or(false);
            let tie = vbits == 0x35800000;
            if our != ref_collapse || tie {
                println!(
                    "{clip}\tattr={attr}\tstep={}\tours={}\tref={}\tvbits={:#010x}\tsbits={:#010x}\t{}{}",
                    is_step as u8, our, ref_collapse, vbits, sbits,
                    if our != ref_collapse { "DISAGREE " } else { "" },
                    if tie { "TIE" } else { "" },
                );
            }
        }
    }
}
