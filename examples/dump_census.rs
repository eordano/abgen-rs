use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}
fn gf(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

fn main() {
    let p = std::env::args().nth(1).expect("bundle");
    let b = Bundle::load(std::path::Path::new(&p)).unwrap();
    let mut classes: BTreeMap<String, usize> = BTreeMap::new();
    let mut meshes: Vec<serde_json::Value> = Vec::new();
    let mut clips: Vec<serde_json::Value> = Vec::new();

    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for obj in &sf.objects {
            let cname = abgen::unity::serialized_file::class_name(obj.class_id).to_string();
            *classes.entry(cname).or_insert(0) += 1;

            if obj.class_id == 43 {
                if let Ok(v) = sf.read_typetree(obj) {
                    let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
                    let subs = v
                        .get("m_SubMeshes")
                        .and_then(|s| s.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    let idx_count: i64 = v
                        .get("m_SubMeshes")
                        .and_then(|s| s.as_array())
                        .map(|a| a.iter().map(|sm| gi(sm, "indexCount")).sum())
                        .unwrap_or(0);
                    let vcount = v
                        .get("m_VertexData")
                        .map(|vd| gi(vd, "m_VertexCount"))
                        .unwrap_or(0);
                    let bc = v.get("m_LocalAABB").and_then(|a| a.get("m_Center"));
                    let be = v.get("m_LocalAABB").and_then(|a| a.get("m_Extent"));
                    let bones = v
                        .get("m_BindPose")
                        .and_then(|s| s.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);
                    meshes.push(serde_json::json!({
                        "name": name, "verts": vcount, "submeshes": subs,
                        "indices": idx_count, "bindposes": bones,
                        "center": bc.map(|c| [gf(c,"x"), gf(c,"y"), gf(c,"z")]),
                        "extent": be.map(|c| [gf(c,"x"), gf(c,"y"), gf(c,"z")]),
                    }));
                }
            } else if obj.class_id == 74 {
                if let Ok(v) = sf.read_typetree(obj) {
                    let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
                    let legacy = v.get("m_Legacy").and_then(|x| x.as_bool()).unwrap_or(false);
                    let mc = v.get("m_MuscleClip");
                    let stop = mc.map(|m| gf(m, "m_StopTime")).unwrap_or(0.0);
                    let bindings = v
                        .get("m_ClipBindingConstant")
                        .and_then(|c| c.get("genericBindings"))
                        .and_then(|g| g.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);

                    let curves: usize = [
                        "m_RotationCurves",
                        "m_PositionCurves",
                        "m_ScaleCurves",
                        "m_EulerCurves",
                        "m_FloatCurves",
                    ]
                    .iter()
                    .map(|k| {
                        v.get(k)
                            .and_then(|a| a.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0)
                    })
                    .sum();
                    let sample_rate = gf(&v, "m_SampleRate");
                    clips.push(serde_json::json!({
                        "name": name, "legacy": legacy, "stop_time": stop,
                        "bindings": bindings, "legacy_curves": curves,
                        "sample_rate": sample_rate,
                    }));
                }
            }
        }
    }

    meshes.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    clips.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    println!(
        "{}",
        serde_json::json!({"classes": classes, "meshes": meshes, "clips": clips})
    );
}
