//! make_template — assemble an abgen typetree-template bundle from real bundles.
//!
//! abgen serializes against `<ABGEN_ROOT>/template/all-types.windows.bundle`
//! (and optional `animated-/emote-/skinned-types.windows.bundle`), which supply,
//! per Unity type, the typetree definition + a base object value to clone. Those
//! reference bundles aren't redistributable, but every type abgen needs already
//! exists in real Decentraland bundles (e.g. the explorer's on-disk AB cache).
//!
//! This tool harvests one object per requested type across a set of source
//! bundles and writes them into a single typetree-enabled bundle abgen can load.
//! Typetree node layouts are platform-independent, so mac-sourced objects are
//! fine for a `.windows` template (abgen overrides the per-platform object data
//! when it builds).
//!
//!   make_template <out.bundle> <Type1,Type2,...> <src-bundle>...
//!
//! Exits non-zero if any requested type was not found in any source.

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

    // Reuse the first source as a structural scaffold (valid UnityFS container),
    // then swap in the harvested type/object tables.
    let mut scaffold = Bundle::load(Path::new(&srcs[0])).expect("load scaffold");
    {
        let sf = scaffold
            .serialized_mut()
            .expect("scaffold has no serialized file");
        sf.types = types;
        sf.objects = objects;
        sf.script_types = Vec::new();
    }
    // Base values are read from the typetree only, never from a resource stream,
    // so drop any .resS node — its stale stream offsets are harmless.
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
