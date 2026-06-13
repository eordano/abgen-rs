use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
fn walk(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Map(ma), Value::Map(mb)) => {
            for (k, va) in ma.iter() {
                let ks: &str = k.as_ref();
                match mb.get(ks) {
                    Some(vb) => walk(va, vb, &format!("{path}.{ks}"), out),
                    None => out.push(format!("{path}.{ks} only-ours")),
                }
            }
        }
        (Value::Array(aa), Value::Array(ab)) => {
            if aa.len() != ab.len() {
                out.push(format!("{path} len {} vs {}", aa.len(), ab.len()));
            }
            for (i, (va, vb)) in aa.iter().zip(ab.iter()).enumerate() {
                walk(va, vb, &format!("{path}[{i}]"), out);
            }
        }
        _ => {
            let sa = format!("{a:?}");
            let sb = format!("{b:?}");
            if sa != sb {
                out.push(format!("{path}: {sa} vs {sb}"));
            }
        }
    }
}
fn main() {
    let mut args = std::env::args().skip(1);
    let load = |p: String| Bundle::load(std::path::Path::new(&p)).unwrap();
    let bo = load(args.next().unwrap());
    let br = load(args.next().unwrap());
    let get = |b: &Bundle| -> Value {
        for f in &b.files {
            if let FileContent::Serialized(sf) = &f.content {
                for o in &sf.objects {
                    if o.class_id == 91 {
                        return sf.read_typetree(o).unwrap();
                    }
                }
            }
        }
        panic!("no 91")
    };
    let vo = get(&bo);
    let vr = get(&br);
    let mut out = Vec::new();
    walk(&vo, &vr, "", &mut out);
    for l in &out {
        println!("{l}");
    }
    eprintln!("total {} diffs", out.len());
}
