
use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::unity::serialized_file::{Object, SerializedType};
use std::collections::HashSet;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: make_template <out.bundle> <Type1,Type2,...> <src-bundle>...");
        std::process::exit(2);
    }
    let out = &args[0];
    let want: Vec<String> = args[1]
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let srcs = &args[2..];

    let mut types: Vec<SerializedType> = Vec::new();
    let mut objects: Vec<Object> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut next_pid: i64 = 1;

    for src in srcs {
        if want.iter().all(|w| seen.contains(w)) {
            break;
        }
        let b = match Bundle::load(Path::new(src)) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skip {src}: {e}");
                continue;
            }
        };
        let Some(sf) = b.serialized() else { continue };
        for obj in &sf.objects {
            if want.contains(&obj.type_name) && !seen.contains(&obj.type_name) {
                let st = sf.types[obj.type_id as usize].clone();
                let mut o = obj.clone();
                o.path_id = next_pid;
                next_pid += 1;
                o.type_id = types.len() as i32;
                types.push(st);
                objects.push(o);
                seen.insert(obj.type_name.clone());
                let from = Path::new(src)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                eprintln!("  + {:<22} from {from}", obj.type_name);
            }
        }
    }

    let missing: Vec<&String> = want.iter().filter(|w| !seen.contains(*w)).collect();
    if !missing.is_empty() {
        eprintln!("MISSING (not found in any source): {missing:?}");
    }

    let mut scaffold = Bundle::load(Path::new(&srcs[0])).expect("load scaffold");
    {
        let sf = scaffold
            .serialized_mut()
            .expect("scaffold has no serialized file");
        sf.types = types;
        sf.objects = objects;
        sf.script_types = Vec::new();
    }

    scaffold
        .files
        .retain(|f| matches!(f.content, FileContent::Serialized(_)));

    let bytes = abgen::bundle::save_bundle(&scaffold).expect("save_bundle");
    std::fs::write(out, &bytes).expect("write output");
    eprintln!(
        "wrote {out}: {} types, {} bytes",
        seen.len(),
        bytes.len()
    );
    if !missing.is_empty() {
        std::process::exit(1);
    }
}
