//! tplcheck — dump the fields that decide whether abgen output renders:
//! Mesh m_IsReadable, AssetBundle preload/container sizes, Material texenv PPtrs.
use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::path::Path;

fn pptr(v: &Value) -> String {
    let f = v.get("m_FileID").and_then(|x| x.as_i64()).unwrap_or(-99);
    let p = v.get("m_PathID").and_then(|x| x.as_i64()).unwrap_or(-99);
    format!("(file={f},path={p})")
}

fn main() {
    let p = std::env::args().nth(1).expect("bundle");
    let b = Bundle::load(Path::new(&p)).unwrap();
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else { continue };
        if !sf.externals.is_empty() {
            println!("externals: {}", sf.externals.len());
        }
        for obj in &sf.objects {
            let Ok(v) = sf.read_typetree(obj) else { continue };
            match obj.type_name.as_str() {
                "Mesh" => {
                    let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
                    let rd = v.get("m_IsReadable").and_then(|x| x.as_bool());
                    let kv = v.get("m_KeepVertices").and_then(|x| x.as_bool());
                    println!("Mesh '{name}' path={} m_IsReadable={rd:?} m_KeepVertices={kv:?}", obj.path_id);
                }
                "AssetBundle" => {
                    let pre = v.get("m_PreloadTable").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
                    let cont = v.get("m_Container").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0);
                    println!("AssetBundle preload={pre} container={cont}");
                    if let Some(pt) = v.get("m_PreloadTable").and_then(|x| x.as_array()) {
                        for (i, e) in pt.iter().enumerate().take(6) { println!("  preload[{i}] {}", pptr(e)); }
                    }
                }
                "Material" => {
                    let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
                    println!("Material '{name}' path={}", obj.path_id);
                    if let Some(tx) = v.get("m_SavedProperties").and_then(|s| s.get("m_TexEnvs")).and_then(|x| x.as_array()) {
                        for e in tx.iter() {
                            // each entry: [name, {m_Texture: PPtr, ...}] or {first,second}
                            let tex = e.get("second").and_then(|s| s.get("m_Texture"))
                                .or_else(|| e.get("m_Texture"));
                            if let Some(t) = tex { println!("  texenv tex {}", pptr(t)); }
                        }
                    }
                }
                "SkinnedMeshRenderer" => {
                    let m = v.get("m_Mesh").map(pptr).unwrap_or_default();
                    println!("SkinnedMeshRenderer m_Mesh={m}");
                }
                _ => {}
            }
        }
    }
}
