use abgen::unity::bundle_file::Bundle;
use abgen::unity::serialized_file::class_name;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let mut a = std::env::args().skip(1);
    let ours = PathBuf::from(a.next().unwrap());
    let refb = PathBuf::from(a.next().unwrap());
    let ob = Bundle::load(&ours).expect("load ours");
    let rb = Bundle::load(&refb).expect("load ref");
    let osf = ob.serialized().expect("osf");
    let rsf = rb.serialized().expect("rsf");
    let name = |sf: &abgen::unity::serialized_file::SerializedFile,
                o: &abgen::unity::serialized_file::Object|
     -> String {
        if o.class_id == 1 {
            if let Ok(v) = sf.read_typetree(o) {
                if let Some(n) = v
                    .as_map()
                    .and_then(|m| m.get("m_Name"))
                    .and_then(|x| x.as_str())
                {
                    return n.to_string();
                }
            }
        }
        String::new()
    };
    let omap: BTreeMap<i64, _> = osf.objects.iter().map(|o| (o.path_id, o)).collect();
    let rmap: BTreeMap<i64, _> = rsf.objects.iter().map(|o| (o.path_id, o)).collect();
    println!(
        "OURS objects: {}   REF objects: {}",
        osf.objects.len(),
        rsf.objects.len()
    );
    println!("--- ours externals ---");
    for (i, e) in osf.externals.iter().enumerate() {
        println!("  [{i}] {}", e.path);
    }
    println!("--- ref externals ---");
    for (i, e) in rsf.externals.iter().enumerate() {
        println!("  [{i}] {}", e.path);
    }
    println!(
        "{:<22} {:>20} {:>6} {:>6} {:<6} name",
        "class", "path_id", "oSz", "rSz", "flag"
    );
    let mut all: Vec<i64> = omap.keys().chain(rmap.keys()).cloned().collect();
    all.sort();
    all.dedup();
    for pid in all {
        match (omap.get(&pid), rmap.get(&pid)) {
            (Some(o), Some(r)) => {
                let flag = if o.data == r.data {
                    ""
                } else if o.data.len() == r.data.len() {
                    "DIFF"
                } else {
                    "SIZE"
                };
                println!(
                    "{:<22} {:>20} {:>6} {:>6} {:<6} {}",
                    class_name(o.class_id),
                    pid,
                    o.data.len(),
                    r.data.len(),
                    flag,
                    name(osf, o)
                );
            }
            (Some(o), None) => println!(
                "{:<22} {:>20} {:>6} {:>6} {:<6} {}",
                class_name(o.class_id),
                pid,
                o.data.len(),
                0,
                "OURS!",
                name(osf, o)
            ),
            (None, Some(r)) => println!(
                "{:<22} {:>20} {:>6} {:>6} {:<6} {}",
                class_name(r.class_id),
                pid,
                0,
                r.data.len(),
                "REF!",
                name(rsf, r)
            ),
            _ => {}
        }
    }
}
