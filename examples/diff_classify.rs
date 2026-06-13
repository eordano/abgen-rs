use abgen::unity::bundle_file::Bundle;
use abgen::unity::serialized_file::Object;
use abgen::value::Value;
use std::collections::BTreeMap;
use std::io::{self, BufRead};
use std::path::PathBuf;

#[derive(Default, Debug)]
struct PatternHits {
    transform_neg_zero: u32,
    assetbundle_preload_swap: u32,
    material_shader_fid_swap: u32,
    material_texenv_fid_swap: u32,
    transform_other_field: u32,
    other_class_diffs: u32,
    bytes_diff: usize,
    objs_diff: usize,
    externals_swapped: bool,
}

fn map_to_lines(prefix: &str, v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Map(m) => {
            for (k, c) in m.iter() {
                map_to_lines(&format!("{}.{}", prefix, k), c, out);
            }
        }
        Value::Array(a) => {
            out.push((format!("{}[len]", prefix), format!("{}", a.len())));
            for (i, e) in a.iter().enumerate() {
                map_to_lines(&format!("{}[{}]", prefix, i), e, out);
            }
        }
        Value::Bytes(b) => out.push((prefix.into(), format!("bytes(len={})", b.len()))),
        Value::Int(i) => out.push((prefix.into(), format!("Int({})", i))),
        Value::Float(f) => out.push((prefix.into(), format!("Float({})", f))),
        Value::Str(s) => out.push((prefix.into(), format!("Str(\"{}\")", s))),
        Value::Bool(b) => out.push((prefix.into(), format!("Bool({})", b))),
        Value::Null => out.push((prefix.into(), "Null".into())),
    }
}

fn classify_one(ours_path: &str, ref_path: &str) -> Option<PatternHits> {
    let ob = Bundle::load(&PathBuf::from(ours_path)).ok()?;
    let rb = Bundle::load(&PathBuf::from(ref_path)).ok()?;
    let osf = ob.serialized()?;
    let rsf = rb.serialized()?;
    let mut hits = PatternHits::default();

    let oext: Vec<_> = osf.externals.iter().map(|e| e.path.clone()).collect();
    let rext: Vec<_> = rsf.externals.iter().map(|e| e.path.clone()).collect();
    if oext != rext {
        let mut oo = oext.clone();
        oo.sort();
        let mut rr = rext.clone();
        rr.sort();
        if oo == rr {
            hits.externals_swapped = true;
        }
    }

    let omap: BTreeMap<i64, &Object> = osf.objects.iter().map(|o| (o.path_id, o)).collect();
    let rmap: BTreeMap<i64, &Object> = rsf.objects.iter().map(|o| (o.path_id, o)).collect();
    for (pid, oo) in &omap {
        if let Some(ro) = rmap.get(pid) {
            if oo.data == ro.data {
                continue;
            }
            hits.objs_diff += 1;
            let diff_bytes = oo
                .data
                .iter()
                .zip(ro.data.iter())
                .filter(|(a, b)| a != b)
                .count();
            hits.bytes_diff += diff_bytes;
            let ov = match osf.read_typetree(oo) {
                Ok(v) => v,
                _ => continue,
            };
            let rv = match rsf.read_typetree(ro) {
                Ok(v) => v,
                _ => continue,
            };
            let mut o = Vec::new();
            map_to_lines("", &ov, &mut o);
            let mut r = Vec::new();
            map_to_lines("", &rv, &mut r);
            let omap2: BTreeMap<_, _> = o.into_iter().collect();
            let rmap2: BTreeMap<_, _> = r.into_iter().collect();
            for (k, v) in &omap2 {
                let rv2 = rmap2.get(k);
                if rv2 != Some(v) {
                    match oo.class_id {
                        4 if k == ".m_LocalPosition.x"
                            || k == ".m_LocalPosition.y"
                            || k == ".m_LocalPosition.z"
                            || k == ".m_LocalEulerAnglesHint.x"
                            || k == ".m_LocalEulerAnglesHint.y"
                            || k == ".m_LocalEulerAnglesHint.z" =>
                        {
                            if (v.contains("Float(-0)")
                                && rv2.is_some_and(|r| r.contains("Float(0)")))
                                || (v.contains("Float(0)")
                                    && rv2.is_some_and(|r| r.contains("Float(-0)")))
                            {
                                hits.transform_neg_zero += 1;
                            } else {
                                hits.transform_other_field += 1;
                            }
                        }
                        4 => {
                            hits.transform_other_field += 1;
                        }
                        142 if k.starts_with(".m_PreloadTable") => {
                            hits.assetbundle_preload_swap += 1;
                        }
                        21 if k.contains(".m_Shader.m_FileID") => {
                            hits.material_shader_fid_swap += 1;
                        }
                        21 if k.contains(".m_TexEnvs") && k.contains(".m_FileID") => {
                            hits.material_texenv_fid_swap += 1;
                        }
                        _ => {
                            hits.other_class_diffs += 1;
                        }
                    }
                }
            }
        }
    }
    Some(hits)
}

fn main() {
    let stdin = io::stdin();
    println!("bundle,objs_diff,bytes_diff,externals_swapped,tx_neg0,tx_other,ab_preload,mat_shader_fid,mat_texenv_fid,other_cls");
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 2 {
            continue;
        }
        let name = PathBuf::from(parts[0])
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let h = match classify_one(parts[0], parts[1]) {
            Some(h) => h,
            None => {
                eprintln!("skip {}", name);
                continue;
            }
        };
        println!(
            "{},{},{},{},{},{},{},{},{},{}",
            name,
            h.objs_diff,
            h.bytes_diff,
            h.externals_swapped,
            h.transform_neg_zero,
            h.transform_other_field,
            h.assetbundle_preload_swap,
            h.material_shader_fid_swap,
            h.material_texenv_fid_swap,
            h.other_class_diffs
        );
    }
}
