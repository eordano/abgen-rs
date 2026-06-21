
use abgen::unity::bundle_file::{Bundle, FileContent};
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let inp = env::args().nth(1).expect("input bundle");
    let outp = env::args().nth(2).expect("output bundle");
    let mut bundle = Bundle::load(Path::new(&inp)).expect("load");
    let mut done = false;
    for f in &mut bundle.files {
        if let FileContent::Serialized(sf) = &mut f.content {
            for o in &mut sf.objects {

                if o.class_id == 43 && o.data.len() > 100 {
                    let mid = o.data.len() / 2;
                    let old = o.data[mid];
                    o.data[mid] ^= 0xFF;
                    println!(
                        "corrupted Mesh path_id={} at byte {} ({}->{})",
                        o.path_id, mid, old, o.data[mid]
                    );
                    done = true;
                    break;
                }
            }
        }
        if done {
            break;
        }
    }
    if !done {
        eprintln!("no Mesh object found to corrupt");
        std::process::exit(2);
    }
    let bytes = bundle.save_lz4().expect("save_lz4");
    fs::write(&outp, bytes).expect("write");
    println!("wrote corrupted bundle -> {outp}");
}
