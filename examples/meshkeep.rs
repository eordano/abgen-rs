use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let p = PathBuf::from(std::env::args().nth(1).expect("bundle"));
    let b = Bundle::load(&p).expect("load");
    let sf = b.serialized().expect("sf");
    let mut refs: HashMap<i64, Vec<i32>> = HashMap::new();
    for o in &sf.objects {
        if !matches!(o.class_id, 33 | 64 | 137) {
            continue;
        }
        let v = match sf.read_typetree(o) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(m) = v.as_map() {
            if let Some(mesh) = m.get("m_Mesh").and_then(|x| x.as_map()) {
                if let Some(pid) = mesh.get("m_PathID").and_then(|x| x.as_i64()) {
                    refs.entry(pid).or_default().push(o.class_id);
                }
            }
        }
    }
    for o in &sf.objects {
        if o.class_id != 43 {
            continue;
        }
        let v = match sf.read_typetree(o) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let m = v.as_map().unwrap();
        let name = m.get("m_Name").and_then(|x| x.as_str()).unwrap_or("");
        let usage = m
            .get("m_MeshUsageFlags")
            .and_then(|x| x.as_i64())
            .unwrap_or(-1);
        let kv = m
            .get("m_KeepVertices")
            .map(|x| format!("{:?}", x))
            .unwrap_or_default();
        let ki = m
            .get("m_KeepIndices")
            .map(|x| format!("{:?}", x))
            .unwrap_or_default();
        let r = m
            .get("m_IsReadable")
            .map(|x| format!("{:?}", x))
            .unwrap_or_default();
        let bk = m
            .get("m_BakedConvexCollisionMesh")
            .and_then(|x| {
                if let Value::Bytes(b) = x {
                    Some(b.len())
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let bt = m
            .get("m_BakedTriangleCollisionMesh")
            .and_then(|x| {
                if let Value::Bytes(b) = x {
                    Some(b.len())
                } else {
                    None
                }
            })
            .unwrap_or(0);
        println!(
            "Mesh pid={} usage={} keepV={} keepI={} readable={} bakedConvex={} bakedTri={} refs={:?} name={}",
            o.path_id,
            usage,
            kv,
            ki,
            r,
            bk,
            bt,
            refs.get(&o.path_id).map(|v| v.as_slice()).unwrap_or(&[]),
            name
        );
    }
}
