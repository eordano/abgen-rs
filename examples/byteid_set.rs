use std::path::PathBuf;
fn main() {
    let mut a = std::env::args().skip(1);
    let od = a.next().unwrap();
    let rd = a.next().unwrap();
    for e in std::fs::read_dir(&rd).unwrap().filter_map(|x| x.ok()) {
        let cid = e.file_name().to_string_lossy().to_string();
        if cid.contains('.') {
            continue;
        }
        let odir = PathBuf::from(&od).join(&cid);
        let rdir = e.path();
        if !odir.is_dir() {
            continue;
        }
        for bf in std::fs::read_dir(&rdir)
            .into_iter()
            .flatten()
            .filter_map(|x| x.ok())
        {
            let n = bf.file_name().to_string_lossy().to_string();
            if !(n.ends_with("_windows") || n.ends_with("_mac")) {
                continue;
            }
            let op = odir.join(&n);
            let rp = rdir.join(&n);
            if !op.exists() {
                continue;
            }
            let ob = match std::fs::read(&op) {
                Ok(b) => b,
                _ => continue,
            };
            let rb = match std::fs::read(&rp) {
                Ok(b) => b,
                _ => continue,
            };
            if ob == rb {
                println!("{}/{}", cid, n);
            }
        }
    }
}
