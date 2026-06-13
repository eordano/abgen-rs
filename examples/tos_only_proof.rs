use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::unity::serialized_file::{class_name, Object, SerializedFile};
use abgen::unity::typetree::{read_typetree, write_typetree};
use abgen::value::Value;
use std::collections::BTreeMap;

fn sf_of(b: &Bundle) -> &SerializedFile {
    for f in &b.files {
        if let FileContent::Serialized(sf) = &f.content {
            return sf;
        }
    }
    panic!("no serialized file");
}

fn pair(e: &Value) -> (i64, String) {
    if let Some(a) = e.as_array() {
        (
            a[0].as_i64().unwrap_or(0),
            a[1].as_str().unwrap_or("").to_string(),
        )
    } else {
        (
            e.get("first").and_then(|x| x.as_i64()).unwrap_or(0),
            e.get("second")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        )
    }
}

fn tos_only_match(
    osf: &SerializedFile,
    oo: &Object,
    rsf: &SerializedFile,
    ro: &Object,
) -> (bool, String) {
    let mut ov = match osf.read_typetree(oo) {
        Ok(v) => v,
        Err(e) => return (false, format!("read ours: {e}")),
    };
    let rv = match rsf.read_typetree(ro) {
        Ok(v) => v,
        Err(e) => return (false, format!("read ref: {e}")),
    };
    let ref_order: Vec<(i64, String)> = match rv.get("m_TOS").and_then(|x| x.as_array()) {
        Some(a) => a.iter().map(pair).collect(),
        None => return (false, "ref has no m_TOS".into()),
    };
    let our_entries: Vec<Value> = match ov.get("m_TOS").and_then(|x| x.as_array()) {
        Some(a) => a.to_vec(),
        None => return (false, "ours has no m_TOS".into()),
    };
    let mut idx: BTreeMap<(i64, String), Vec<Value>> = BTreeMap::new();
    for e in &our_entries {
        idx.entry(pair(e)).or_default().push(e.clone());
    }
    let mut reordered: Vec<Value> = Vec::with_capacity(ref_order.len());
    for key in &ref_order {
        match idx.get_mut(key).and_then(|v| v.pop()) {
            Some(e) => reordered.push(e),
            None => return (false, format!("ref TOS entry not in ours: {key:?}")),
        }
    }
    if reordered.len() != our_entries.len() {
        return (false, "TOS length mismatch".into());
    }
    if let Some(m) = ov.as_map_mut() {
        m.insert("m_TOS", Value::Array(reordered));
    }
    let node = osf.types[oo.type_id as usize].node.as_ref().unwrap();
    let reser = write_typetree(&ov, node, osf.big_endian);
    if reser == ro.data {
        (true, String::new())
    } else {
        let orig = read_typetree(&oo.data, node, osf.big_endian).unwrap();
        let roundtrip = write_typetree(&orig, node, osf.big_endian);
        let rt = if roundtrip == oo.data {
            "roundtrip-ok"
        } else {
            "ROUNDTRIP-BROKEN"
        };
        (
            false,
            format!(
                "reordered controller != ref ({} vs {} bytes; {rt})",
                reser.len(),
                ro.data.len()
            ),
        )
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let ours = a.next().unwrap();
    let refp = a.next().unwrap();
    let ob = Bundle::load(std::path::Path::new(&ours)).unwrap();
    let rb = Bundle::load(std::path::Path::new(&refp)).unwrap();
    let osf = sf_of(&ob);
    let rsf = sf_of(&rb);
    let omap: BTreeMap<i64, &Object> = osf.objects.iter().map(|o| (o.path_id, o)).collect();
    let rmap: BTreeMap<i64, &Object> = rsf.objects.iter().map(|o| (o.path_id, o)).collect();
    let mut residues: Vec<String> = Vec::new();
    let mut all: Vec<i64> = omap.keys().chain(rmap.keys()).cloned().collect();
    all.sort();
    all.dedup();
    for pid in all {
        match (omap.get(&pid), rmap.get(&pid)) {
            (Some(o), Some(r)) => {
                if o.data == r.data {
                    continue;
                }
                if o.class_id == 91 {
                    let (ok, msg) = tos_only_match(osf, o, rsf, r);
                    if !ok {
                        residues.push(format!("AnimatorController {pid}: {msg}"));
                    }
                } else {
                    let kind = if o.data.len() == r.data.len() {
                        "DIFF"
                    } else {
                        "SIZE"
                    };
                    residues.push(format!(
                        "{} {pid} {kind} ({} vs {} B)",
                        class_name(o.class_id),
                        o.data.len(),
                        r.data.len()
                    ));
                }
            }
            (Some(o), None) => residues.push(format!("{} {pid} ONLY-OURS", class_name(o.class_id))),
            (None, Some(r)) => residues.push(format!("{} {pid} ONLY-REF", class_name(r.class_id))),
            (None, None) => unreachable!(),
        }
    }
    if residues.is_empty() {
        println!("TOS-ONLY");
    } else {
        println!("RESIDUE:");
        for r in &residues {
            println!("  {r}");
        }
        std::process::exit(1);
    }
}
