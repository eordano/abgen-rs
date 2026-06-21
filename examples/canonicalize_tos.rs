use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::unity::typetree::write_typetree;
use abgen::value::Value;
use std::path::Path;

fn tos_crc(entry: &Value) -> Option<u32> {
    if let Some(a) = entry.as_array() {
        return a.first().and_then(|x| x.as_i64()).map(|v| v as u32);
    }
    entry
        .get("first")
        .and_then(|x| x.as_i64())
        .map(|v| v as u32)
}

fn canonicalize_one(path: &Path) -> anyhow::Result<bool> {
    let mut bundle = Bundle::load(path)?;
    let mut changed = false;

    for entry in &mut bundle.files {
        let FileContent::Serialized(sf) = &mut entry.content else {
            continue;
        };

        for oi in 0..sf.objects.len() {
            if sf.objects[oi].class_id != 91 {
                continue;
            }

            let mut value = sf.read_typetree(&sf.objects[oi])?;

            let Some(tos) = value.get_mut("m_TOS").and_then(|v| v.as_array_mut()) else {
                continue;
            };

            let before: Vec<u32> = tos.iter().filter_map(tos_crc).collect();
            tos.sort_by_key(|e| tos_crc(e).unwrap_or(0));
            let after: Vec<u32> = tos.iter().filter_map(tos_crc).collect();

            if before == after {
                continue;
            }

            let type_id = sf.objects[oi].type_id as usize;
            let node = sf.types[type_id]
                .node
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("AnimatorController type has no type tree"))?;
            let new_data = write_typetree(&value, node, sf.big_endian);

            if new_data.len() != sf.objects[oi].data.len() {
                return Err(anyhow::anyhow!(
                    "TOS reorder changed object size {} -> {} (not size-preserving)",
                    sf.objects[oi].data.len(),
                    new_data.len()
                ));
            }

            sf.objects[oi].data = new_data;
            changed = true;
        }
    }

    if changed {
        let bytes = bundle.save_lz4()?;
        std::fs::write(path, bytes)?;
    }

    Ok(changed)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: canonicalize_tos <bundle> [<bundle> ...]");
        std::process::exit(2);
    }

    let mut any_changed = false;
    let mut had_error = false;
    for p in &args {
        match canonicalize_one(Path::new(p)) {
            Ok(true) => {
                any_changed = true;
                println!("CANON {p}");
            }
            Ok(false) => {
                println!("SKIP  {p}");
            }
            Err(e) => {
                had_error = true;
                eprintln!("ERROR {p}: {e:#}");
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }
    let _ = any_changed;
}
