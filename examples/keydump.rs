use abgen::animation_mecanim::binding_key_dump;
use abgen::hashes::crc32;

fn main() {
    let glb = std::env::args().nth(1).unwrap();
    let clip_want = std::env::args().nth(3).unwrap();
    let crc_want: i64 = std::env::args().nth(4).unwrap().parse().unwrap();
    let glb_bytes = std::fs::read(&glb).unwrap();
    for (clip, rows) in binding_key_dump(&glb_bytes) {
        if clip != clip_want {
            continue;
        }
        for (path, attr, is_step, comps) in rows {
            let pcrc = crc32(path.as_bytes()) as i64;
            if pcrc != crc_want {
                continue;
            }
            println!("clip={clip} path={path} crc={pcrc} attr={attr} step={is_step}");
            for (c, vals) in comps.iter().enumerate() {
                let v0 = vals[0] as f32;

                let mut a = 0f32;

                let mut mn = vals[0] as f32;
                let mut mx = vals[0] as f32;

                let mut cmet = 0f64;
                for &v in vals {
                    let vf = v as f32;
                    a = a.max((vf - v0).abs());
                    mn = mn.min(vf);
                    mx = mx.max(vf);
                    cmet = cmet.max((v - vals[0]).abs());
                }
                println!(
                    "  comp{c} n={} v0={:.10e}  A(max|v-v0|f32)={:#010x}  B(range f32)={:#010x}  C(max|v-v0|f64)={:.6e}",
                    vals.len(), vals[0], a.to_bits(), (mx - mn).to_bits(), cmet
                );

                let mut idxmax = 0usize;
                for (i, &v) in vals.iter().enumerate() {
                    if ((v as f32) - v0).abs() == a {
                        idxmax = i;
                    }
                }
                println!("    v0bits={:#010x} vmaxbits={:#010x} (key {idxmax}) raw v0={:.17} vmax={:.17}",
                    v0.to_bits(), (vals[idxmax] as f32).to_bits(), vals[0], vals[idxmax]);
            }
        }
    }
}
