use abgen::pathids;
use std::collections::HashSet;

fn main() {
    let mut a = std::env::args().skip(1);
    let hash = a.next().expect("glb content hash");
    let prefix = a.next().expect("path prefix (may be empty)");
    let segs: Vec<String> = a
        .next()
        .expect("comma-separated path segments")
        .split(',')
        .map(|s| s.to_string())
        .collect();
    let depth: usize = a.next().expect("depth").parse().unwrap();
    let targets: HashSet<i64> = a.map(|s| s.parse().expect("pid i64")).collect();
    let guid = pathids::asset_guid(&hash);
    let comp_types = [
        "Transform",
        "MeshFilter",
        "MeshRenderer",
        "MeshCollider",
        "SkinnedMeshRenderer",
        "Animation",
        "Animator",
        "BoxCollider",
    ];

    let mut paths: Vec<String> = Vec::new();
    let mut layer: Vec<String> = segs.iter().map(|s| format!("{prefix}{s}")).collect();
    paths.extend(layer.iter().cloned());
    for _ in 1..depth {
        let mut next = Vec::with_capacity(layer.len() * segs.len());
        for p in &layer {
            for s in &segs {
                next.push(format!("{p}/{s}"));
            }
        }
        paths.extend(next.iter().cloned());
        layer = next;
    }
    eprintln!("paths: {}", paths.len());

    let mut check = |t: &str, name: &str| {
        for idx in 0u32..4 {
            let lid = pathids::local_id_for_recycle_name_indexed(t, name, idx);
            for &ft in &[2i32, 3i32] {
                let pid = pathids::prefab_packed_path_id(&guid, lid, ft);
                if targets.contains(&pid) {
                    println!("{pid}  = ft={ft} type={t} name={name:?} idx={idx}");
                }
            }
        }
    };
    for p in &paths {
        check("GameObject", p);
        for t in &comp_types {
            check(t, &format!("{p}/{t}"));
        }
    }
}
