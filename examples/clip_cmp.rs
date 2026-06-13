use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;
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
fn clips(b: &Bundle) -> Vec<(String, Value)> {
    let mut v = Vec::new();
    for f in &b.files {
        if let FileContent::Serialized(sf) = &f.content {
            for o in &sf.objects {
                if o.class_id == 74 {
                    let t = sf.read_typetree(o).unwrap();
                    let n = t
                        .get("m_Name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    v.push((n, t));
                }
            }
        }
    }
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}
fn main() {
    let mut args = std::env::args().skip(1);
    let load = |p: String| Bundle::load(std::path::Path::new(&p)).unwrap();
    let bo = load(args.next().unwrap());
    let br = load(args.next().unwrap());
    let max: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(40);
    let co = clips(&bo);
    let cr = clips(&br);
    if co.len() != cr.len() {
        println!("clip count {} vs {}", co.len(), cr.len());
    }
    for ((no, vo), (nr, vr)) in co.iter().zip(cr.iter()) {
        println!("== clip {no} vs {nr}");
        let mut out = Vec::new();
        walk(vo, vr, "", &mut out);

        let mut groups: BTreeMap<String, usize> = BTreeMap::new();
        for l in &out {
            let p = l.split(&[':', ' '][..]).next().unwrap_or(l);
            let mut segs: Vec<&str> = Vec::new();
            for s in p.split('.') {
                let s = s.split('[').next().unwrap_or(s);
                if !s.is_empty() {
                    segs.push(s);
                }
                if segs.len() >= 4 {
                    break;
                }
            }
            *groups.entry(segs.join(".")).or_default() += 1;
        }
        for (g, n) in &groups {
            println!("  {n:6}  {g}");
        }
        for l in out.iter().take(max) {
            println!("  {l}");
        }
        println!("  total {} diffs", out.len());
    }
}
