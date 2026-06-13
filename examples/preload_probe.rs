use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::env;
use std::path::PathBuf;

fn main() {
    let p = PathBuf::from(env::args().nth(1).expect("bundle path"));
    let b = Bundle::load(&p).expect("load");
    for f in &b.files {
        println!("FILE\t{}", f.name);
    }
    let sf = b.serialized().expect("sf");
    for (i, ext) in sf.externals.iter().enumerate() {
        println!("EXT\t{}\t{}", i + 1, ext.path);
    }
    for (i, o) in sf.objects.iter().enumerate() {
        println!(
            "OBJ\t{}\t{}\t{}\t{}",
            i,
            o.path_id,
            o.class_id,
            abgen::unity::serialized_file::class_name(o.class_id)
        );
    }
    for o in &sf.objects {
        if o.class_id != 142 {
            continue;
        }
        let v = sf.read_typetree(o).expect("tt");
        let m = v.as_map().unwrap();
        if let Some(deps) = m.get("m_Dependencies").and_then(|x| x.as_array()) {
            for (i, d) in deps.iter().enumerate() {
                println!("DEP\t{}\t{}", i, d.as_str().unwrap_or(""));
            }
        }
        if let Some(cont) = m.get("m_Container").and_then(|x| x.as_array()) {
            for (i, e) in cont.iter().enumerate() {
                if let Some(arr) = e.as_array() {
                    let k = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(slot) = arr.get(1).and_then(|v| v.as_map()) {
                        let gi = |key: &str| -> i64 {
                            slot.get(key)
                                .and_then(|v| {
                                    if let Value::Int(i) = v {
                                        Some(*i)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0)
                        };
                        let (fid, pid) = slot
                            .get("asset")
                            .and_then(|v| v.as_map())
                            .map(|am| {
                                let g = |key: &str| -> i64 {
                                    am.get(key)
                                        .and_then(|v| {
                                            if let Value::Int(i) = v {
                                                Some(*i)
                                            } else {
                                                None
                                            }
                                        })
                                        .unwrap_or(0)
                                };
                                (g("m_FileID"), g("m_PathID"))
                            })
                            .unwrap_or((0, 0));
                        println!(
                            "CONT\t{}\t{}\t{}\t{}\t{}\t{}",
                            i,
                            k,
                            gi("preloadIndex"),
                            gi("preloadSize"),
                            fid,
                            pid
                        );
                    }
                }
            }
        }
        if let Some(pre) = m.get("m_PreloadTable").and_then(|x| x.as_array()) {
            for (i, pv) in pre.iter().enumerate() {
                if let Some(pm) = pv.as_map() {
                    let g = |key: &str| -> i64 {
                        pm.get(key)
                            .and_then(|v| {
                                if let Value::Int(i) = v {
                                    Some(*i)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0)
                    };
                    println!("PRE\t{}\t{}\t{}", i, g("m_FileID"), g("m_PathID"));
                }
            }
        }
    }
}
