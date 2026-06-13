use abgen::unity::bundle_file::Bundle;
use std::path::PathBuf;
fn main() {
    let p = PathBuf::from(std::env::args().nth(1).unwrap());
    let b = Bundle::load(&p).expect("load");
    let sf = b.serialized().expect("sf");
    for (i, o) in sf.objects.iter().enumerate() {
        println!("[{}] class={} pid={}", i, o.class_id, o.path_id);
    }
}
