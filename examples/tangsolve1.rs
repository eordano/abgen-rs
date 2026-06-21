
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

fn finalize(nn: [f64; 3], t: [f64; 3], tb: [f64; 3]) -> [u32; 4] {
    let d = nn[0] * t[0] + nn[1] * t[1] + nn[2] * t[2];
    let ox = t[0] - nn[0] * d;
    let oy = t[1] - nn[1] * d;
    let oz = t[2] - nn[2] * d;
    let mag = (ox * ox + oy * oy + oz * oz).sqrt();
    let (fb, b2) = fb_axes(nn);
    // NaN or magnitude <= 1e-6 both count as degenerate; the negated
    // compare is deliberate (`mag <= 1e-6` would let NaN slip through).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let degenerate = !(mag > 1e-6);
    let (tgx, tgy, tgz);
    if !degenerate {
        tgx = ox / mag;
        tgy = oy / mag;
        tgz = oz / mag;
    } else {
        let dd = nn[0] * fb[0] + nn[1] * fb[1] + nn[2] * fb[2];
        let ox2 = fb[0] - nn[0] * dd;
        let oy2 = fb[1] - nn[1] * dd;
        let oz2 = fb[2] - nn[2] * dd;
        let mag2 = (ox2 * ox2 + oy2 * oy2 + oz2 * oz2).sqrt();
        if mag2 > 0.0 {
            tgx = ox2 / mag2;
            tgy = oy2 / mag2;
            tgz = oz2 / mag2;
        } else {
            tgx = fb[0];
            tgy = fb[1];
            tgz = fb[2];
        }
    }
    let cx = nn[1] * tgz - nn[2] * tgy;
    let cy = nn[2] * tgx - nn[0] * tgz;
    let cz = nn[0] * tgy - nn[1] * tgx;
    let w = if degenerate {
        let h = cx * b2[0] + cy * b2[1] + cz * b2[2];
        if h > 0.0 {
            1.0f64
        } else {
            -1.0
        }
    } else {
        let h = cx * tb[0] + cy * tb[1] + cz * tb[2];
        if h != 0.0 {
            if h > 0.0 {
                1.0
            } else {
                -1.0
            }
        } else {
            let h2 = cx * b2[0] + cy * b2[1] + cz * b2[2];
            if h2 > 0.0 {
                1.0
            } else {
                -1.0
            }
        }
    };
    [
        (tgx as f32).to_bits(),
        (tgy as f32).to_bits(),
        (tgz as f32).to_bits(),
        (w as f32).to_bits(),
    ]
}

#[derive(Clone, Copy, Debug)]
struct SV {
    sdir_mode: u8,
    edges_f64: bool,
    norm_recip: bool,
    sum_assoc_right: bool,
    wgt_mode: u8,
}

fn contribution(c: &MeshCase, tri: [usize; 3], corner: usize, v: SV) -> ([f64; 3], [f64; 3]) {
    let (i1, i2, i3) = (tri[0], tri[1], tri[2]);
    let v1 = c.pos[i1];
    let v2 = c.pos[i2];
    let v3 = c.pos[i3];
    let w1 = c.uv[i1];
    let w2 = c.uv[i2];
    let w3 = c.uv[i3];
    let rr = |x: f64| if v.edges_f64 { x } else { r32(x) };
    let x1 = rr(v2[0] - v1[0]);
    let x2 = rr(v3[0] - v1[0]);
    let y1 = rr(v2[1] - v1[1]);
    let y2 = rr(v3[1] - v1[1]);
    let z1 = rr(v2[2] - v1[2]);
    let z2 = rr(v3[2] - v1[2]);
    let s1 = r32(w2[0] - w1[0]);
    let s2 = r32(w3[0] - w1[0]);
    let t1 = r32(w2[1] - w1[1]);
    let t2 = r32(w3[1] - w1[1]);
    let den = s1 * t2 - s2 * t1;
    if den == 0.0 {
        return ([0.0; 3], [0.0; 3]);
    }
    let r = 1.0 / den;
    let raw_s = [t2 * x1 - t1 * x2, t2 * y1 - t1 * y2, t2 * z1 - t1 * z2];
    let raw_t = [s1 * x2 - s2 * x1, s1 * y2 - s2 * y1, s1 * z2 - s2 * z1];
    let apply = |raw: [f64; 3]| -> [f64; 3] {
        match v.sdir_mode {
            0 => [raw[0] * r, raw[1] * r, raw[2] * r],
            1 => [raw[0] / den, raw[1] / den, raw[2] / den],
            _ => raw,
        }
    };
    let mut s = apply(raw_s);
    let mut t = apply(raw_t);
    let norm = |x: &mut [f64; 3], neg: bool| {
        let sumsq = if v.sum_assoc_right {
            x[0] * x[0] + (x[1] * x[1] + x[2] * x[2])
        } else {
            x[0] * x[0] + x[1] * x[1] + x[2] * x[2]
        };
        let l = sumsq.sqrt();
        if l > 0.0 {
            if v.norm_recip {
                let i = 1.0 / l;
                x[0] *= i;
                x[1] *= i;
                x[2] *= i;
            } else {
                x[0] /= l;
                x[1] /= l;
                x[2] /= l;
            }
        }
        if neg {
            x[0] = -x[0];
            x[1] = -x[1];
            x[2] = -x[2];
        }
    };
    let neg = v.sdir_mode == 2 && den < 0.0;
    norm(&mut s, neg);
    norm(&mut t, neg);
    let absden = den.abs();
    let pv = [v1, v2, v3];
    let p0 = pv[corner];
    let pa = pv[(corner + 1) % 3];
    let pb = pv[(corner + 2) % 3];
    let e1x = r32(pa[0] - p0[0]);
    let e1y = r32(pa[1] - p0[1]);
    let e1z = r32(pa[2] - p0[2]);
    let e2x = r32(pb[0] - p0[0]);
    let e2y = r32(pb[1] - p0[1]);
    let e2z = r32(pb[2] - p0[2]);
    let l1sq = e1x * e1x + e1y * e1y + e1z * e1z;
    let l2sq = e2x * e2x + e2y * e2y + e2z * e2z;
    let wgt = if l1sq > 0.0 && l2sq > 0.0 {
        let dot = e1x * e2x + e1y * e2y + e1z * e2z;
        match v.wgt_mode {
            0 => (dot / (l1sq * l2sq).sqrt()).clamp(-1.0, 1.0).acos() * absden,
            1 => (dot / (l1sq.sqrt() * l2sq.sqrt())).clamp(-1.0, 1.0).acos() * absden,
            2 => (dot / (l1sq * l2sq).sqrt()).clamp(-1.0, 1.0).acos(),
            3 => absden,
            _ => ((dot / (l1sq * l2sq).sqrt()).clamp(-1.0, 1.0) as f32).acos() as f64 * absden,
        }
    } else {
        0.0
    };
    (
        [wgt * s[0], wgt * s[1], wgt * s[2]],
        [wgt * t[0], wgt * t[1], wgt * t[2]],
    )
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();

    let mut variants = vec![];
    for sdir_mode in 0..3u8 {
        for &edges_f64 in &[false, true] {
            for &norm_recip in &[false, true] {
                for &sum_assoc_right in &[false, true] {
                    for wgt_mode in 0..5u8 {
                        variants.push(SV {
                            sdir_mode,
                            edges_f64,
                            norm_recip,
                            sum_assoc_right,
                            wgt_mode,
                        });
                    }
                }
            }
        }
    }
    let nv = variants.len();
    let mut fixed = vec![0usize; nv];
    let mut broke = vec![0usize; nv];
    let mut tot_bad = 0usize;
    let mut tot_ok = 0usize;

    for fp in &files {
        let c = load(fp);
        let n = c.pos.len();

        let mut inc: Vec<Vec<([usize; 3], usize)>> = vec![vec![]; n];
        for t in c.idx.chunks_exact(3) {
            let tri = [t[0] as usize, t[1] as usize, t[2] as usize];
            for ci in 0..3 {
                inc[tri[ci]].push((tri, ci));
            }
        }
        let basev = SV {
            sdir_mode: 0,
            edges_f64: false,
            norm_recip: false,
            sum_assoc_right: false,
            wgt_mode: 0,
        };
        // `i` indexes inc/c.nrm/c.refr at the same position; range loop is clearest.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            if inc[i].len() != 1 {
                continue;
            }
            let (tri, ci) = inc[i][0];
            let (t1c, t2c) = contribution(&c, tri, ci, basev);
            let got = finalize(c.nrm[i], t1c, t2c);
            let was_ok = got == c.refr[i];
            if was_ok {
                tot_ok += 1
            } else {
                tot_bad += 1
            }
            for (vi, v) in variants.iter().enumerate() {
                let (t1v, t2v) = contribution(&c, tri, ci, *v);
                let gv = finalize(c.nrm[i], t1v, t2v);
                let ok = gv == c.refr[i];
                if !was_ok && ok {
                    fixed[vi] += 1;
                }
                if was_ok && !ok {
                    broke[vi] += 1;
                }
            }
        }
    }
    println!("1-tri verts: bad {}  ok {}", tot_bad, tot_ok);
    let mut rows: Vec<usize> = (0..nv).collect();
    rows.sort_by_key(|&vi| (broke[vi] as i64) - (fixed[vi] as i64));
    for &vi in rows.iter().take(20) {
        println!(
            "fixed {:5} broke {:5} :: {:?}",
            fixed[vi], broke[vi], variants[vi]
        );
    }
}
