use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::path::PathBuf;

fn walk(v: &Value, refs: &mut Vec<(i64, i64)>) {
    match v {
        Value::Map(m) => {
            let fid = m.get("m_FileID");
            let pid = m.get("m_PathID");
            if let (Some(Value::Int(f)), Some(Value::Int(p))) = (fid, pid) {
                if m.len() == 2 {
                    refs.push((*f, *p));
                    return;
                }
            }
            for (_, vv) in m.iter() {
                walk(vv, refs);
            }
        }
        Value::Array(a) => {
            for vv in a {
                walk(vv, refs);
            }
        }
        _ => {}
    }
}

fn main() {
    let p = PathBuf::from(std::env::args().nth(1).expect("bundle"));
    let b = Bundle::load(&p).expect("load");
    let sf = b.serialized().expect("sf");
    let mut seen: Vec<i64> = Vec::new();
    for o in &sf.objects {
        let v = match sf.read_typetree(o) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mut refs = Vec::new();
        walk(&v, &mut refs);
        let mut firsts = Vec::new();
        for (f, _) in &refs {
            if *f >= 1 && !seen.contains(f) {
                seen.push(*f);
                firsts.push(*f);
            }
        }
        if !firsts.is_empty() {
            println!(
                "obj class={} pid={} first_use_fids={:?}",
                o.class_id, o.path_id, firsts
            );
        }
    }
    println!("OVERALL first-use fid order: {:?}", seen);
}
