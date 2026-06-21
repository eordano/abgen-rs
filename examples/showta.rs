use abgen::unity::bundle_file::Bundle;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let mut a = std::env::args().skip(1);
    let ours = PathBuf::from(a.next().unwrap());
    let refb = PathBuf::from(a.next().unwrap());
    let ob = Bundle::load(&ours).expect("load ours");
    let rb = Bundle::load(&refb).expect("load ref");
    let osf = ob.serialized().expect("osf");
    let rsf = rb.serialized().expect("rsf");
    let omap: BTreeMap<i64, _> = osf.objects.iter().map(|o| (o.path_id, o)).collect();
    let rmap: BTreeMap<i64, _> = rsf.objects.iter().map(|o| (o.path_id, o)).collect();
    for (pid, o) in &omap {
        let r = rmap.get(pid).unwrap();
        if o.class_id == 142 || o.class_id == 49 {
            println!("=== class {} pid {} ===", o.class_id, pid);
            println!("OURS: {:?}", String::from_utf8_lossy(&o.data));
            println!("REF : {:?}", String::from_utf8_lossy(&r.data));
        }
        if o.class_id == 28 {
            println!("=== Texture2D header first 120 bytes ===");
            print!("OURS:");
            for i in 0..120 { print!(" {:02x}", o.data[i]); }
            println!();
            print!("REF :");
            for i in 0..120 { print!(" {:02x}", r.data[i]); }
            println!();
        }
    }
}
