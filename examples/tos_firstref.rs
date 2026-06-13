use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::HashSet;

fn walk_leaves(v: &Value, out: &mut Vec<u32>) {
    match v {
        Value::Map(m) => {
            for (_, vv) in m.iter() {
                walk_leaves(vv, out);
            }
        }
        Value::Array(a) => {
            for vv in a.iter() {
                walk_leaves(vv, out);
            }
        }
        Value::Int(i) => {
            let u = *i as u64;
            if u <= u32::MAX as u64 {
                out.push(u as u32);
            }
        }
        _ => {}
    }
}

fn main() {
    let p = std::env::args().nth(1).unwrap();
    let b = Bundle::load(std::path::Path::new(&p)).unwrap();
    for f in &b.files {
        if let FileContent::Serialized(sf) = &f.content {
            for o in &sf.objects {
                if o.class_id != 91 {
                    continue;
                }
                let v = sf.read_typetree(o).unwrap();
                let tos = v.get("m_TOS").and_then(|x| x.as_array()).unwrap();
                let mut tos_order: Vec<(u32, String)> = Vec::new();
                for e in tos {
                    let (h, n) = if let Some(a) = e.as_array() {
                        (
                            a[0].as_i64().unwrap() as u32,
                            a[1].as_str().unwrap_or("").to_string(),
                        )
                    } else {
                        (
                            e.get("first").and_then(|x| x.as_i64()).unwrap() as u32,
                            e.get("second")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                        )
                    };
                    tos_order.push((h, n));
                }
                let keys: HashSet<u32> = tos_order.iter().map(|(h, _)| *h).collect();
                let ctrl = v.get("m_Controller").unwrap();
                let mut leaves = Vec::new();
                walk_leaves(ctrl, &mut leaves);
                let mut seen = HashSet::new();
                let mut first_order: Vec<u32> = Vec::new();
                for u in leaves {
                    if keys.contains(&u) && seen.insert(u) {
                        first_order.push(u);
                    }
                }
                let tos_keys: Vec<u32> = tos_order.iter().map(|(h, _)| *h).collect();
                println!("tos  : {:?}", tos_keys);
                println!("first: {:?}", first_order);
                let matches = tos_keys == first_order;
                let rev: Vec<u32> = first_order.iter().rev().cloned().collect();
                println!(
                    "match={} rev_match={} missing_from_ctrl={}",
                    matches,
                    tos_keys == rev,
                    tos_keys.len() - first_order.len()
                );
            }
        }
    }
}
