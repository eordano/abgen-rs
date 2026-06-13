use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::env;
use std::path::PathBuf;

fn main() {
    let mut args = env::args().skip(1);
    let ours = PathBuf::from(args.next().unwrap());
    let refb = PathBuf::from(args.next().unwrap());
    for (label, p) in [("OURS", &ours), ("REF ", &refb)] {
        let b = Bundle::load(p).expect("load");
        let sf = b.serialized().expect("sf");
        for o in &sf.objects {
            if o.class_id != 142 {
                continue;
            }
            let v = sf.read_typetree(o).expect("tt");
            let m = v.as_map().unwrap();
            println!("=== {} ===", label);
            if let Some(deps) = m.get("m_Dependencies").and_then(|x| x.as_array()) {
                println!("  m_Dependencies (count={})", deps.len());
                for (i, d) in deps.iter().enumerate() {
                    println!("    [{}] {}", i, d.as_str().unwrap_or(""));
                }
            }
            if let Some(cont) = m.get("m_Container").and_then(|x| x.as_array()) {
                println!("  m_Container (count={})", cont.len());
                for (i, e) in cont.iter().enumerate() {
                    if let Some(arr) = e.as_array() {
                        let k = arr.first().and_then(|v| v.as_str()).unwrap_or("");
                        if let Some(slot) = arr.get(1).and_then(|v| v.as_map()) {
                            let pi = slot
                                .get("preloadIndex")
                                .and_then(|v| {
                                    if let Value::Int(i) = v {
                                        Some(*i)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                            let ps = slot
                                .get("preloadSize")
                                .and_then(|v| {
                                    if let Value::Int(i) = v {
                                        Some(*i)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0);
                            let asset = slot.get("asset").and_then(|v| v.as_map());
                            let (fid, pid) = if let Some(am) = asset {
                                let f = am
                                    .get("m_FileID")
                                    .and_then(|v| {
                                        if let Value::Int(i) = v {
                                            Some(*i)
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or(0);
                                let p = am
                                    .get("m_PathID")
                                    .and_then(|v| {
                                        if let Value::Int(i) = v {
                                            Some(*i)
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or(0);
                                (f, p)
                            } else {
                                (0, 0)
                            };
                            println!(
                                "    [{}] {:<50} pi={} ps={} asset=(fid={}, pid={})",
                                i, k, pi, ps, fid, pid
                            );
                        }
                    }
                }
            }
            if let Some(pre) = m.get("m_PreloadTable").and_then(|x| x.as_array()) {
                println!("  m_PreloadTable (count={})", pre.len());
                for (i, p) in pre.iter().enumerate() {
                    if let Some(pm) = p.as_map() {
                        let f = pm
                            .get("m_FileID")
                            .and_then(|v| {
                                if let Value::Int(i) = v {
                                    Some(*i)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        let pid = pm
                            .get("m_PathID")
                            .and_then(|v| {
                                if let Value::Int(i) = v {
                                    Some(*i)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        println!("    [{:>2}] fid={} pid={}", i, f, pid);
                    }
                }
            }
        }
    }
}
