use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let b = Bundle::load(std::path::Path::new(&p)).unwrap();
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };

        let mut texinfo = std::collections::HashMap::new();
        for o in &sf.objects {
            if o.class_id == 28 {
                if let Ok(v) = sf.read_typetree(o) {
                    let n = v
                        .get("m_Name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let fmt = v
                        .get("m_TextureFormat")
                        .and_then(|x| x.as_i64())
                        .unwrap_or(0);
                    texinfo.insert(o.path_id, (n, fmt));
                }
            }
        }
        for o in &sf.objects {
            if o.class_id != 21 {
                continue;
            }
            let v = match sf.read_typetree(o) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
            println!("Material pid={} '{}'", o.path_id, name);
            let Some(envs) = v.get("m_SavedProperties").and_then(|s| s.get("m_TexEnvs")) else {
                continue;
            };
            if let Value::Array(arr) = envs {
                for e in arr {
                    let pair = match e {
                        Value::Array(p) if p.len() == 2 => p,
                        _ => continue,
                    };
                    let slot = pair[0].as_str().unwrap_or("?");
                    let tex = pair[1].get("m_Texture");
                    let pid = tex
                        .and_then(|t| t.get("m_PathID"))
                        .and_then(|x| x.as_i64())
                        .unwrap_or(0);
                    let fid = tex
                        .and_then(|t| t.get("m_FileID"))
                        .and_then(|x| x.as_i64())
                        .unwrap_or(0);
                    if pid != 0 {
                        let info = if fid == 0 {
                            texinfo
                                .get(&pid)
                                .map(|(n, f)| format!("-> INTERNAL '{n}' fmt={f}"))
                                .unwrap_or_else(|| "-> internal pid not found!".into())
                        } else {
                            format!("-> EXTERNAL file#{fid}")
                        };
                        println!("  {slot}: fileID={fid} pid={pid} {info}");
                    }
                }
            }
        }
    }
}
