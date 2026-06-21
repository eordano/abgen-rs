struct MeshCase {
    pos: Vec<[f64; 3]>,
    nrm: Vec<[f64; 3]>,
    uv: Vec<[f64; 2]>,
    idx: Vec<u32>,
    refr: Vec<[u32; 4]>,
}

fn load(path: &str) -> MeshCase {
    let mut c = MeshCase {
        pos: vec![],
        nrm: vec![],
        uv: vec![],
        idx: vec![],
        refr: vec![],
    };
    let f = |s: &str| f32::from_bits(u32::from_str_radix(s, 16).unwrap()) as f64;
    let b = |s: &str| u32::from_str_radix(s, 16).unwrap();
    for line in std::fs::read_to_string(path).unwrap().lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("P") => c.pos.push([
                f(it.next().unwrap()),
                f(it.next().unwrap()),
                f(it.next().unwrap()),
            ]),
            Some("N") => c.nrm.push([
                f(it.next().unwrap()),
                f(it.next().unwrap()),
                f(it.next().unwrap()),
            ]),
            Some("U") => c.uv.push([f(it.next().unwrap()), f(it.next().unwrap())]),
            Some("I") => {
                for _ in 0..3 {
                    c.idx.push(it.next().unwrap().parse().unwrap());
                }
            }
            Some("T") => c.refr.push([
                b(it.next().unwrap()),
                b(it.next().unwrap()),
                b(it.next().unwrap()),
                b(it.next().unwrap()),
            ]),
            _ => {}
        }
    }
    c
}

const fn r32(x: f64) -> f64 {
    x as f32 as f64
}

fn fb_axes(nn: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let ax = nn[0].abs();
    let ay = nn[1].abs();
    let az = nn[2].abs();
    if ax <= ay && ax <= az {
        if ay <= az {
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        } else {
            ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
        }
    } else if ay <= az {
        if ax <= az {
            ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0])
        } else {
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0])
        }
    } else if ax <= ay {
        ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0])
    } else {
        ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0])
    }
}

fn accumulate(c: &MeshCase) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let n = c.pos.len();
    let mut tan1 = vec![[0.0f64; 3]; n];
    let mut tan2 = vec![[0.0f64; 3]; n];
    let m = (c.idx.len() / 3) * 3;
    let mut k = 0;
    while k < m {
        let i1 = c.idx[k] as usize;
        let i2 = c.idx[k + 1] as usize;
        let i3 = c.idx[k + 2] as usize;
        k += 3;
        let v1 = c.pos[i1];
        let v2 = c.pos[i2];
        let v3 = c.pos[i3];
        let w1 = c.uv[i1];
        let w2 = c.uv[i2];
        let w3 = c.uv[i3];
        let x1 = r32(v2[0] - v1[0]);
        let x2 = r32(v3[0] - v1[0]);
        let y1 = r32(v2[1] - v1[1]);
        let y2 = r32(v3[1] - v1[1]);
        let z1 = r32(v2[2] - v1[2]);
        let z2 = r32(v3[2] - v1[2]);
        let s1 = r32(w2[0] - w1[0]);
        let s2 = r32(w3[0] - w1[0]);
        let t1 = r32(w2[1] - w1[1]);
        let t2 = r32(w3[1] - w1[1]);
        let den = s1 * t2 - s2 * t1;
        if den == 0.0 {
            continue;
        }
        let rr = 1.0 / den;
        let mut sx = (t2 * x1 - t1 * x2) * rr;
        let mut sy = (t2 * y1 - t1 * y2) * rr;
        let mut sz = (t2 * z1 - t1 * z2) * rr;
        let mut tx = (s1 * x2 - s2 * x1) * rr;
        let mut ty = (s1 * y2 - s2 * y1) * rr;
        let mut tz = (s1 * z2 - s2 * z1) * rr;
        let sl = (sx * sx + sy * sy + sz * sz).sqrt();
        if sl > 0.0 {
            sx /= sl;
            sy /= sl;
            sz /= sl;
        }
        let tl = (tx * tx + ty * ty + tz * tz).sqrt();
        if tl > 0.0 {
            tx /= tl;
            ty /= tl;
            tz /= tl;
        }
        let absden = den.abs();
        let tri = [i1, i2, i3];
        let pv = [v1, v2, v3];
        for ci in 0..3 {
            let p0 = pv[ci];
            let pa = pv[(ci + 1) % 3];
            let pb = pv[(ci + 2) % 3];
            let e1 = [r32(pa[0] - p0[0]), r32(pa[1] - p0[1]), r32(pa[2] - p0[2])];
            let e2 = [r32(pb[0] - p0[0]), r32(pb[1] - p0[1]), r32(pb[2] - p0[2])];
            let l1sq = e1[0] * e1[0] + e1[1] * e1[1] + e1[2] * e1[2];
            let l2sq = e2[0] * e2[0] + e2[1] * e2[1] + e2[2] * e2[2];
            let wgt = if l1sq > 0.0 && l2sq > 0.0 {
                ((e1[0] * e2[0] + e1[1] * e2[1] + e1[2] * e2[2]) / (l1sq * l2sq).sqrt())
                    .clamp(-1.0, 1.0)
                    .acos()
                    * absden
            } else {
                0.0
            };
            let vi = tri[ci];
            tan1[vi][0] += wgt * sx;
            tan1[vi][1] += wgt * sy;
            tan1[vi][2] += wgt * sz;
            tan2[vi][0] += wgt * tx;
            tan2[vi][1] += wgt * ty;
            tan2[vi][2] += wgt * tz;
        }
    }
    (tan1, tan2)
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();

    let mut fb_pts: Vec<(f64, f64, f64)> = vec![];
    let mut real_pts: Vec<(f64, f64, f64)> = vec![];
    let mut samples = 0;
    for fp in &files {
        let c = load(fp);
        let (tan1, _tan2) = accumulate(&c);
        let n = c.pos.len();
        // `i` indexes c.nrm and tan1 at the same position; range loop is clearest.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            let nn = c.nrm[i];
            let t = tan1[i];
            let d = nn[0] * t[0] + nn[1] * t[1] + nn[2] * t[2];
            let ox = t[0] - nn[0] * d;
            let oy = t[1] - nn[1] * d;
            let oz = t[2] - nn[2] * d;
            let t1mag = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
            let omag = (ox * ox + oy * oy + oz * oz).sqrt();

            let (fb, _b2) = fb_axes(nn);
            let dd = nn[0] * fb[0] + nn[1] * fb[1] + nn[2] * fb[2];
            let o2 = [fb[0] - nn[0] * dd, fb[1] - nn[1] * dd, fb[2] - nn[2] * dd];
            let m2 = (o2[0] * o2[0] + o2[1] * o2[1] + o2[2] * o2[2]).sqrt();
            let fbt = if m2 > 0.0 {
                [
                    (o2[0] / m2) as f32,
                    (o2[1] / m2) as f32,
                    (o2[2] / m2) as f32,
                ]
            } else {
                [fb[0] as f32, fb[1] as f32, fb[2] as f32]
            };
            let r = c.refr[i];

            let our_real = if omag > 0.0 {
                [(ox / omag) as f32, (oy / omag) as f32, (oz / omag) as f32]
            } else {
                fbt
            };

            let ref_is_fb = (0..3).all(|k| fbt[k].to_bits() == r[k]);
            let ref_is_real = (0..3).all(|k| our_real[k].to_bits() == r[k]);

            let fb_eq_real = (0..3).all(|k| fbt[k].to_bits() == our_real[k].to_bits());
            if fb_eq_real {
                continue;
            }

            if omag < 1e-1 {
                samples += 1;
                if ref_is_fb && !ref_is_real {
                    fb_pts.push((t1mag, omag, d.abs()));
                } else if ref_is_real {
                    real_pts.push((t1mag, omag, d.abs()));
                }
            }
        }
    }
    println!("examined {} verts with |ortho|<1e-3", samples);
    println!(
        "ref==fallback: {}   ref==real: {}",
        fb_pts.len(),
        real_pts.len()
    );

    let fn_cases: Vec<&(f64, f64, f64)> = fb_pts.iter().filter(|p| p.1 > 1e-6).collect();
    let fp_cases: Vec<&(f64, f64, f64)> = real_pts
        .iter()
        .filter(|p| p.1 <= 1e-6 && p.1 > 0.0)
        .collect();
    println!("FALSE NEG (ref=fb, our|ortho|>1e-6): {}", fn_cases.len());
    println!("FALSE POS (ref=real, our|ortho|<=1e-6): {}", fp_cases.len());

    let mut fnv = fn_cases.clone();
    fnv.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    println!("--- top FALSE NEG by |ortho| (|tan1|, |ortho|, |dot|) ---");
    for p in fnv.iter().take(20) {
        println!("  |tan1|={:.4e} |ortho|={:.4e} |dot|={:.4e}", p.0, p.1, p.2);
    }

    let mut fp2 = fp_cases.clone();
    fp2.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    println!("--- bottom FALSE POS by |ortho| ---");
    for p in fp2.iter().take(20) {
        println!("  |tan1|={:.4e} |ortho|={:.4e} |dot|={:.4e}", p.0, p.1, p.2);
    }

    println!("=== BOUNDARY ANALYSIS (non-coincidental verts) ===");
    let fb_t1max = fb_pts.iter().map(|p| p.0).fold(0.0, f64::max);
    let fb_omax = fb_pts.iter().map(|p| p.1).fold(0.0, f64::max);
    let re_t1min = real_pts.iter().map(|p| p.0).fold(f64::MAX, f64::min);
    let re_omin = real_pts.iter().map(|p| p.1).fold(f64::MAX, f64::min);
    println!(
        "ref-FALLBACK: |tan1| max {:.4e}  |ortho| max {:.4e}  (n={})",
        fb_t1max,
        fb_omax,
        fb_pts.len()
    );
    println!(
        "ref-REAL    : |tan1| min {:.4e}  |ortho| min {:.4e}  (n={})",
        re_t1min,
        re_omin,
        real_pts.len()
    );

    let re_below = real_pts.iter().filter(|p| p.1 <= fb_omax).count();
    let fb_above = fb_pts.iter().filter(|p| p.1 >= re_omin).count();
    println!(
        "ref-real with |ortho|<=fb_omax: {}   ref-fb with |ortho|>=re_omin: {}",
        re_below, fb_above
    );

    let fb_rmin = fb_pts
        .iter()
        .map(|p| if p.0 > 0.0 { p.1 / p.0 } else { 1.0 })
        .fold(f64::MAX, f64::min);
    let re_rmin = real_pts
        .iter()
        .map(|p| if p.0 > 0.0 { p.1 / p.0 } else { 1.0 })
        .fold(f64::MAX, f64::min);
    println!(
        "ratio |ortho|/|tan1|: fb min {:.4e}  real min {:.4e}",
        fb_rmin, re_rmin
    );

    let mut re_near: Vec<f64> = real_pts
        .iter()
        .map(|p| p.1)
        .filter(|&o| (1e-6..1e-5).contains(&o))
        .collect();
    re_near.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut fb_near: Vec<f64> = fb_pts.iter().map(|p| p.1).filter(|&o| o >= 1e-6).collect();
    fb_near.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "ref-REAL lowest |ortho| in [1e-6,1e-5]: {:?}",
        &re_near[..re_near.len().min(12)]
    );
    println!(
        "ref-FALLBACK |ortho| >= 1e-6 (sorted):  {:?}",
        &fb_near[..fb_near.len().min(12)]
    );
    println!(
        "ref-FALLBACK >=1e-6 max: {:.5e}",
        fb_near.last().copied().unwrap_or(0.0)
    );

    for g in [1.2e-6, 1.4e-6, 2e-6, 5e-6] {
        let fixes = fb_pts.iter().filter(|p| p.1 > 1e-6 && p.1 <= g).count();
        let breaks = real_pts.iter().filter(|p| p.1 > 1e-6 && p.1 <= g).count();
        println!(
            "gate->{:.1e}: fixes {} fallback, BREAKS {} real  (net {})",
            g,
            fixes,
            breaks,
            fixes as i64 - breaks as i64
        );
    }
}
