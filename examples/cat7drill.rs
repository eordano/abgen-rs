use abgen::unity::bundle_file::Bundle;
use abgen::unity::serialized_file::{class_name, Object, SerializedFile};
use abgen::value::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn flatten(prefix: &str, v: &Value, out: &mut Vec<(String, Value)>) {
    match v {
        Value::Map(m) => {
            for (k, c) in m.iter() {
                flatten(&format!("{}.{}", prefix, k), c, out);
            }
        }
        Value::Array(a) => {
            out.push((format!("{}[len]", prefix), Value::Int(a.len() as i64)));
            for (i, e) in a.iter().enumerate() {
                flatten(&format!("{}[{}]", prefix, i), e, out);
            }
        }
        other => out.push((prefix.into(), other.clone())),
    }
}

fn fmt_val(v: &Value) -> String {
    match v {
        Value::Float(f) => {
            let f32v = *f as f32;
            format!("f{:.9e} bits=0x{:08x}", f, (f32v).to_bits())
        }
        Value::Int(i) => format!("i{}", i),
        Value::Str(s) => format!("\"{}\"", s),
        Value::Bool(b) => format!("{}", b),
        Value::Bytes(b) => format!("bytes(len={})", b.len()),
        Value::Null => "null".into(),
        Value::Map(_) | Value::Array(_) => "<agg>".into(),
    }
}

fn bytes_diff_desc(a: &[u8], b: &[u8]) -> String {
    if a.len() != b.len() {
        return format!("len {} vs {}", a.len(), b.len());
    }
    let offs: Vec<usize> = a
        .iter()
        .zip(b.iter())
        .enumerate()
        .filter(|(_, (x, y))| x != y)
        .map(|(i, _)| i)
        .collect();
    let n = offs.len();
    let head: Vec<String> = offs.iter().take(8).map(|o| format!("{}", o)).collect();
    format!("{} bytes differ at [{}]", n, head.join(","))
}

fn go_name(sf: &SerializedFile, o: &Object) -> String {
    if let Ok(v) = sf.read_typetree(o) {
        if let Some(n) = v
            .as_map()
            .and_then(|m| m.get("m_Name"))
            .and_then(|x| x.as_str())
        {
            return n.to_string();
        }
    }
    String::new()
}

fn main() {
    let mut a = std::env::args().skip(1);
    let ours = PathBuf::from(a.next().expect("ours"));
    let refb = PathBuf::from(a.next().expect("ref"));
    let max_fields: usize = a.next().map(|s| s.parse().unwrap()).unwrap_or(40);
    let ob = Bundle::load(&ours).expect("load ours");
    let rb = Bundle::load(&refb).expect("load ref");
    let osf = ob.serialized().expect("osf");
    let rsf = rb.serialized().expect("rsf");

    let oext: Vec<_> = osf.externals.iter().map(|e| e.path.clone()).collect();
    let rext: Vec<_> = rsf.externals.iter().map(|e| e.path.clone()).collect();
    if oext != rext {
        println!("EXTERNALS DIFFER:");
        println!("  ours: {:?}", oext);
        println!("  ref:  {:?}", rext);
    }

    let omap: BTreeMap<i64, &Object> = osf.objects.iter().map(|o| (o.path_id, o)).collect();
    let rmap: BTreeMap<i64, &Object> = rsf.objects.iter().map(|o| (o.path_id, o)).collect();
    for (pid, oo) in &omap {
        let Some(ro) = rmap.get(pid) else {
            println!(
                "OBJ {} pid={} OURS-ONLY size={}",
                class_name(oo.class_id),
                pid,
                oo.data.len()
            );
            continue;
        };
        if oo.data == ro.data {
            continue;
        }
        println!(
            "OBJ {} pid={} name='{}' sizes {}/{}  raw: {}",
            class_name(oo.class_id),
            pid,
            go_name(osf, oo),
            oo.data.len(),
            ro.data.len(),
            bytes_diff_desc(&oo.data, &ro.data)
        );
        let (Ok(ov), Ok(rv)) = (osf.read_typetree(oo), rsf.read_typetree(ro)) else {
            println!("  <typetree read failed>");
            continue;
        };
        let mut of = Vec::new();
        flatten("", &ov, &mut of);
        let mut rf = Vec::new();
        flatten("", &rv, &mut rf);
        let om: BTreeMap<String, Value> = of.into_iter().collect();
        let rm: BTreeMap<String, Value> = rf.into_iter().collect();
        let mut shown = 0usize;
        let mut total = 0usize;
        let mut keys: Vec<&String> = om.keys().chain(rm.keys()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            let o = om.get(k);
            let r = rm.get(k);
            let same = match (o, r) {
                (Some(Value::Bytes(x)), Some(Value::Bytes(y))) => x == y,
                (Some(x), Some(y)) => fmt_val(x) == fmt_val(y),
                _ => false,
            };
            if same {
                continue;
            }
            total += 1;
            if shown < max_fields {
                let extra = match (o, r) {
                    (Some(Value::Bytes(x)), Some(Value::Bytes(y))) => {
                        format!("  [{}]", bytes_diff_desc(x, y))
                    }
                    _ => String::new(),
                };
                println!(
                    "  {} : {} -> {}{}",
                    k,
                    o.map(fmt_val).unwrap_or("<absent>".into()),
                    r.map(fmt_val).unwrap_or("<absent>".into()),
                    extra
                );
                shown += 1;
            }
        }
        if total > shown {
            println!("  ... {} more differing fields", total - shown);
        }
    }
    for (pid, ro) in &rmap {
        if !omap.contains_key(pid) {
            println!(
                "OBJ {} pid={} REF-ONLY size={}",
                class_name(ro.class_id),
                pid,
                ro.data.len()
            );
        }
    }
}
