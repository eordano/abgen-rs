use abgen::unity::bundle_file::Bundle;
use std::env;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let p = PathBuf::from(env::args().nth(1).unwrap());
    let out = PathBuf::from(env::args().nth(2).unwrap());
    let pid: i64 = env::args().nth(3).unwrap().parse().unwrap();
    let b = Bundle::load(&p).unwrap();
    let sf = b.serialized().unwrap();
    let o = sf.objects.iter().find(|o| o.path_id == pid).unwrap();
    let mut f = std::fs::File::create(&out).unwrap();
    f.write_all(&o.data).unwrap();
    println!(
        "Wrote {} bytes (class {}) to {}",
        o.data.len(),
        o.class_id,
        out.display()
    );
}
