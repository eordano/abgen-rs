use abgen::unity::bundle_file::Bundle;
use std::path::PathBuf;
fn main() {
    let p = PathBuf::from(std::env::args().nth(1).unwrap());
    let pid: i64 = std::env::args().nth(2).unwrap().parse().unwrap();
    let b = Bundle::load(&p).expect("load");
    let sf = b.serialized().expect("sf");
    let o = sf.objects.iter().find(|o| o.path_id == pid).unwrap();
    let v = sf.read_typetree(o).unwrap();
    println!("{:#?}", v);
}
