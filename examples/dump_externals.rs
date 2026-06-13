use abgen::unity::bundle_file::Bundle;
use std::env;
use std::path::PathBuf;

fn hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{:02x}", byte));
    }
    s
}

fn main() {
    let mut args = env::args().skip(1);
    let ours = PathBuf::from(args.next().unwrap());
    let refb = PathBuf::from(args.next().unwrap());
    for (label, p) in [("OURS", &ours), ("REF ", &refb)] {
        let b = Bundle::load(p).expect("load");
        let sf = b.serialized().expect("sf");
        println!("=== {} {} ===", label, p.display());
        println!("externals_count={}", sf.externals.len());
        for (i, e) in sf.externals.iter().enumerate() {
            println!(
                "  external[{}] (m_FileID={}) guid={} type={} path={}",
                i,
                i + 1,
                hex(&e.guid),
                e.r#type,
                e.path
            );
        }
    }
}
