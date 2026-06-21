use abgen::unity::bundle_file::Bundle;
use abgen::unity::serialized_file::class_name;
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
    println!("OURS header platform={} REF platform={}", osf.target_platform, rsf.target_platform);
    let rmap: BTreeMap<i64, _> = rsf.objects.iter().map(|o| (o.path_id, o)).collect();
    for o in &osf.objects {
        let r = match rmap.get(&o.path_id) { Some(r) => r, None => continue };
        if o.data == r.data { continue; }
        let n = o.data.len().min(r.data.len());
        let mut first = None;
        let mut ndiff = 0;
        for i in 0..n {
            if o.data[i] != r.data[i] {
                if first.is_none() { first = Some(i); }
                ndiff += 1;
            }
        }
        println!("class={} pid={} olen={} rlen={} first_diff={:?} ndiff={} ({:.1}%)",
            class_name(o.class_id), o.path_id, o.data.len(), r.data.len(),
            first, ndiff, 100.0*ndiff as f64/n as f64);
        if let Some(f) = first {
            let s = f.saturating_sub(8);
            let e = (f+24).min(n);
            print!("  ours @{}: ", s); for i in s..e { print!("{:02x} ", o.data[i]); } println!();
            print!("  ref  @{}: ", s); for i in s..e { print!("{:02x} ", r.data[i]); } println!();
        }
    }
}
