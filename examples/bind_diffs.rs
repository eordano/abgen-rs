use abgen::animation_mecanim::binding_max_diffs;
fn main() {
    for p in std::env::args().skip(1) {
        let bytes = std::fs::read(&p).unwrap();
        for (clip, rows) in binding_max_diffs(&bytes) {
            println!("== {clip}");
            for (path, attr, step, dim, md) in rows {
                println!("{path}\t{attr}\t{}\t{dim}\t{md:e}", step as u8);
            }
        }
    }
}
