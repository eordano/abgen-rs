use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn flat(prefix: &str, v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Map(m) => {
            for (k, x) in m.iter() {
                flat(&format!("{prefix}.{k}"), x, out);
            }
        }
        Value::Array(a) if a.len() <= 8 => {
            for (i, x) in a.iter().enumerate() {
                flat(&format!("{prefix}[{i}]"), x, out);
            }
        }
        Value::Array(a) => out.push((prefix.to_string(), format!("array[{}]", a.len()))),
        Value::Bytes(b) => out.push((prefix.to_string(), format!("bytes[{}]", b.len()))),
        other => out.push((prefix.to_string(), format!("{other:?}"))),
    }
}

fn texes(p: &str) -> Vec<(String, Vec<(String, String)>)> {
    let b = Bundle::load(std::path::Path::new(p)).unwrap();
    let mut out = Vec::new();
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for o in &sf.objects {
            if o.class_id != 28 {
                continue;
            }
            let v = sf.read_typetree(o).unwrap();
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let mut fields = Vec::new();
            flat("", &v, &mut fields);
            out.push((format!("{name}/{}", o.path_id), fields));
        }
    }
    out
}

fn main() {
    let a = std::env::args().nth(1).unwrap();
    let b = std::env::args().nth(2).unwrap();
    let oa = texes(&a);
    let ob = texes(&b);
    for ((n1, f1), (n2, f2)) in oa.iter().zip(ob.iter()) {
        assert_eq!(n1, n2);
        for ((k1, v1), (_k2, v2)) in f1.iter().zip(f2.iter()) {
            if v1 != v2 {
                println!("{n1}: {k1}  ours={v1}  ref={v2}");
            }
        }
    }
}
