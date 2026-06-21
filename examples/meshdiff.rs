
use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;
use std::path::Path;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(-1)
}

#[derive(Debug, Default)]
struct MeshInfo {
    vcount: i64,
    channels: Vec<(i64, i64, i64, i64)>,
    vdata_len: usize,
    vdata_sum: u64,
    idx_format: i64,
    idx_len: usize,
    idx_sum: u64,
    bindposes: usize,
    submeshes: usize,
    readable: bool,
}

fn collect(path: &str) -> BTreeMap<String, MeshInfo> {
    let b = Bundle::load(Path::new(path)).expect("load");
    let mut out = BTreeMap::new();
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else { continue };
        for obj in &sf.objects {
            if obj.type_name != "Mesh" {
                continue;
            }
            let Ok(v) = sf.read_typetree(obj) else { continue };
            let name = v.get("m_Name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let vd = v.get("m_VertexData");
            let channels = vd
                .and_then(|d| d.get("m_Channels"))
                .and_then(|c| c.as_array())
                .map(|a| {
                    a.iter()
                        .map(|c| (gi(c, "stream"), gi(c, "offset"), gi(c, "format"), gi(c, "dimension")))
                        .collect()
                })
                .unwrap_or_default();
            let vbytes = vd.and_then(|d| d.get("m_DataSize")).and_then(|x| x.as_bytes());
            let ibytes = v.get("m_IndexBuffer").and_then(|x| x.as_bytes());
            let sum = |b: Option<&[u8]>| b.map(|b| b.iter().map(|&x| x as u64).sum()).unwrap_or(0);
            out.insert(
                name,
                MeshInfo {
                    vcount: vd.map(|d| gi(d, "m_VertexCount")).unwrap_or(-1),
                    channels,
                    vdata_len: vbytes.map(|b| b.len()).unwrap_or(0),
                    vdata_sum: sum(vbytes),
                    idx_format: gi(&v, "m_IndexFormat"),
                    idx_len: ibytes.map(|b| b.len()).unwrap_or(0),
                    idx_sum: sum(ibytes),
                    bindposes: v.get("m_BindPose").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
                    submeshes: v.get("m_SubMeshes").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0),
                    readable: v.get("m_IsReadable").and_then(|x| x.as_bool()).unwrap_or(false),
                },
            );
        }
    }
    out
}

fn main() {
    let a = std::env::args().nth(1).expect("bundleA (ours)");
    let b = std::env::args().nth(2).expect("bundleB (ref)");
    let ma = collect(&a);
    let mb = collect(&b);
    println!("A={} meshes  B={} meshes", ma.len(), mb.len());
    let names: std::collections::BTreeSet<&String> = ma.keys().chain(mb.keys()).collect();
    for n in names {
        match (ma.get(n), mb.get(n)) {
            (Some(x), Some(y)) => {
                let mut diffs = Vec::new();
                if x.vcount != y.vcount { diffs.push(format!("vcount {}!={}", x.vcount, y.vcount)); }
                if x.channels != y.channels { diffs.push(format!("channels\n    A={:?}\n    B={:?}", x.channels, y.channels)); }
                if x.vdata_len != y.vdata_len { diffs.push(format!("vdata_len {}!={}", x.vdata_len, y.vdata_len)); }
                else if x.vdata_sum != y.vdata_sum { diffs.push(format!("vdata BYTES differ (len ok, sum {}!={})", x.vdata_sum, y.vdata_sum)); }
                if x.idx_format != y.idx_format { diffs.push(format!("idx_format {}!={}", x.idx_format, y.idx_format)); }
                if x.idx_len != y.idx_len { diffs.push(format!("idx_len {}!={}", x.idx_len, y.idx_len)); }
                else if x.idx_sum != y.idx_sum { diffs.push(format!("index BYTES differ (sum {}!={})", x.idx_sum, y.idx_sum)); }
                if x.bindposes != y.bindposes { diffs.push(format!("bindposes {}!={}", x.bindposes, y.bindposes)); }
                if x.submeshes != y.submeshes { diffs.push(format!("submeshes {}!={}", x.submeshes, y.submeshes)); }
                if x.readable != y.readable { diffs.push(format!("readable {}!={}", x.readable, y.readable)); }
                if diffs.is_empty() {
                    println!("OK   {n} (v={} bp={})", x.vcount, x.bindposes);
                } else {
                    println!("DIFF {n}:\n  - {}", diffs.join("\n  - "));
                }
            }
            (Some(_), None) => println!("ONLY-A {n}"),
            (None, Some(_)) => println!("ONLY-B {n}"),
            _ => {}
        }
    }
}
