use abgen::pathids;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed = format!("{}/animatorController", args[0]);
    let guid = pathids::asset_guid(&seed);
    let targets: Vec<i64> = args[1..].iter().map(|s| s.parse().unwrap()).collect();
    for idx in 0..40usize {
        let fid = pathids::deterministic_sub_asset_path_id(&seed, idx);
        let pid = pathids::prefab_packed_path_id(&guid, fid, pathids::FILE_TYPE_SERIALIZED_ASSET);
        let hit: Vec<usize> = targets
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == pid)
            .map(|(i, _)| i)
            .collect();
        if !hit.is_empty() {
            println!("idx={idx} -> pid={pid}  MATCHES target#{:?}", hit);
        }
    }

    eprintln!("seed={seed} guid={guid}");
}
