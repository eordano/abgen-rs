
use abgen::unity::bundle_file::Bundle;
use abgen::validate::{bundle_cab_names, validate_bundle, Severity, ValidateCtx};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn out_root() -> PathBuf {
    PathBuf::from(
        std::env::var("ABGEN_VAL300_OUT").unwrap_or_else(|_| "/tmp/abgen-val300-out".to_string()),
    )
}

fn sample_bundles(root: &Path, max: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with("_windows") || n.ends_with("_mac"))
                .unwrap_or(false)
            {
                found.push(p);
                if found.len() >= max {
                    return found;
                }
            }
        }
    }
    found
}

fn global_ctx(root: &Path) -> ValidateCtx {

    let mut cabs = HashSet::new();
    for b in sample_bundles(root, 5000) {
        if let Ok(data) = std::fs::read(&b) {
            for c in bundle_cab_names(&data) {
                cabs.insert(c);
            }
        }
    }
    ValidateCtx::with_global_cabs(cabs)
}

#[test]
fn real_bundles_validate_clean() {
    let root = out_root();
    if !root.exists() {
        eprintln!("SKIP real_bundles_validate_clean: {root:?} not present");
        return;
    }
    let samples = sample_bundles(&root, 20);
    if samples.is_empty() {
        eprintln!("SKIP real_bundles_validate_clean: no bundles under {root:?}");
        return;
    }
    let ctx = global_ctx(&root);
    let mut total_err = 0;
    for b in &samples {
        let data = std::fs::read(b).unwrap();
        let findings = validate_bundle(&data, &b.display().to_string(), &ctx);
        for f in &findings {
            if f.severity == Severity::Error {
                eprintln!("ERR {} {}: {}", f.code, f.bundle, f.msg);
                total_err += 1;
            }
        }
    }
    assert_eq!(total_err, 0, "real emitted bundles should have zero ERR findings");
}

#[test]
fn corrupted_material_binding_is_caught() {
    let root = out_root();
    if !root.exists() {
        eprintln!("SKIP corrupted_material_binding_is_caught: {root:?} not present");
        return;
    }

    let ctx = ValidateCtx::single_file();
    for b in sample_bundles(&root, 200) {
        let Ok(data) = std::fs::read(&b) else { continue };
        let Ok(mut bundle) = Bundle::load_bytes(&data) else {
            continue;
        };
        if !corrupt_first_material_texture(&mut bundle) {
            continue;
        }

        let Ok(bytes) = bundle.save_lz4() else { continue };
        let findings = validate_bundle(&bytes, "corrupted", &ctx);
        let caught = findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.code == "E2");
        assert!(
            caught,
            "validator must catch the dangling Material->Texture binding (findings: {:?})",
            findings
                .iter()
                .map(|f| format!("{} {}", f.code, f.msg))
                .collect::<Vec<_>>()
        );
        return;
    }
    eprintln!("SKIP corrupted_material_binding_is_caught: no suitable Material bundle found");
}

fn corrupt_first_material_texture(bundle: &mut Bundle) -> bool {
    use abgen::unity::typetree;
    use abgen::value::Value;
    let Some(sf) = bundle.serialized_mut() else {
        return false;
    };
    let big_endian = sf.big_endian;

    let existing: HashSet<i64> = sf.objects.iter().map(|o| o.path_id).collect();
    let bogus = existing.iter().copied().min().unwrap_or(-1) - 7777;

    for i in 0..sf.objects.len() {
        if sf.objects[i].class_id != 21 {
            continue;
        }

        let obj = sf.objects[i].clone();
        let Ok(mut v) = sf.read_typetree(&obj) else {
            continue;
        };
        let mut changed = false;
        if let Some(envs) = v
            .get_mut("m_SavedProperties")
            .and_then(|s| s.get_mut("m_TexEnvs"))
            .and_then(|x| x.as_array_mut())
        {
            for e in envs.iter_mut() {
                let Some(pair) = e.as_array_mut() else { continue };
                let Some(tex) = pair.get_mut(1).and_then(|p| p.get_mut("m_Texture")) else {
                    continue;
                };
                let Some(tm) = tex.as_map_mut() else { continue };
                let fid = tm.get("m_FileID").and_then(|x| x.as_i64()).unwrap_or(0);
                let pid = tm.get("m_PathID").and_then(|x| x.as_i64()).unwrap_or(0);
                if fid == 0 && pid != 0 {
                    if let Some(slot) = tm.get_mut("m_PathID") {
                        *slot = Value::Int(bogus);
                        changed = true;
                        break;
                    }
                }
            }
        }
        if changed {

            let node = sf.types[obj.type_id as usize]
                .node
                .as_ref()
                .expect("material type has a type tree");
            let bytes = typetree::write_typetree(&v, node, big_endian);
            sf.objects[i].data = bytes;
            return true;
        }
    }
    false
}
