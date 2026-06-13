use abgen::pathids;
fn main() {
    let mut args = std::env::args().skip(1);
    let hash = args.next().unwrap();
    let seed = format!("{hash}/animatorController");
    let guid = pathids::asset_guid(&seed);
    let pids: Vec<i64> = args.map(|a| a.parse().unwrap()).collect();
    for pid in pids {
        let mut found = None;
        for idx in 0..64usize {
            let fid = pathids::deterministic_sub_asset_path_id(&seed, idx);
            let p = pathids::prefab_packed_path_id(&guid, fid, 2);
            if p == pid {
                found = Some(idx);
                break;
            }
        }
        match found {
            Some(i) => println!("{hash}\t{pid}\tidx={i}"),
            None => println!("{hash}\t{pid}\tNOT-IN-FAMILY"),
        }
    }
}
