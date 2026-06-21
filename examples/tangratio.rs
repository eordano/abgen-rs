struct MeshCase {
    name: String,
    pos: Vec<[f64; 3]>,
    nrm: Vec<[f64; 3]>,
    uv: Vec<[f64; 2]>,
    idx: Vec<u32>,
    refr: Vec<[u32; 4]>,
}

fn load(path: &str) -> MeshCase {
    let mut c = MeshCase {
        name: path.into(),
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

fn contrib(c: &MeshCase, tri: [usize; 3], corner: usize) -> Option<([f64; 3], f64, f64)> {
    let (i1, i2, i3) = (tri[0], tri[1], tri[2]);
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
        return None;
    }
    let r = 1.0 / den;
    let mut s = [
        (t2 * x1 - t1 * x2) * r,
        (t2 * y1 - t1 * y2) * r,
        (t2 * z1 - t1 * z2) * r,
    ];
    let sl = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
    if sl > 0.0 {
        s = [s[0] / sl, s[1] / sl, s[2] / sl];
    }
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
    if l1sq <= 0.0 || l2sq <= 0.0 {
        return Some((s, 0.0, den));
    }
    let d = ((e1x * e2x + e1y * e2y + e1z * e2z) / (l1sq * l2sq).sqrt()).clamp(-1.0, 1.0);
    Some((s, d.acos() * den.abs(), den))
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let max_rows: usize = std::env::args()
        .nth(2)
        .map(|s| s.parse().unwrap())
        .unwrap_or(40);
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();
    let mut shown = 0usize;
    for fp in &files {
        if shown >= max_rows {
            break;
        }
        let c = load(fp);
        let n = c.pos.len();
        let mut inc: Vec<Vec<([usize; 3], usize)>> = vec![vec![]; n];
        for t in c.idx.chunks_exact(3) {
            let tri = [t[0] as usize, t[1] as usize, t[2] as usize];
            for ci in 0..3 {
                inc[tri[ci]].push((tri, ci));
            }
        }
        // `i` indexes inc at multiple positions; range loop is clearest.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            if shown >= max_rows {
                break;
            }
            if inc[i].len() != 2 {
                continue;
            }
            let (Some((s_a, w_a, den_a)), Some((s_b, w_b, den_b))) = (
                contrib(&c, inc[i][0].0, inc[i][0].1),
                contrib(&c, inc[i][1].0, inc[i][1].1),
            ) else {
                continue;
            };
            if w_a == 0.0 || w_b == 0.0 {
                continue;
            }

            let nn = c.nrm[i];
            let t1 = [
                w_a * s_a[0] + w_b * s_b[0],
                w_a * s_a[1] + w_b * s_b[1],
                w_a * s_a[2] + w_b * s_b[2],
            ];
            let proj = |t: [f64; 3]| -> [f64; 3] {
                let d = nn[0] * t[0] + nn[1] * t[1] + nn[2] * t[2];
                let o = [t[0] - nn[0] * d, t[1] - nn[1] * d, t[2] - nn[2] * d];
                let m = (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt();
                if m > 1e-6 {
                    [o[0] / m, o[1] / m, o[2] / m]
                } else {
                    [0.0; 3]
                }
            };
            let ours = proj(t1);
            let ours_b = [
                (ours[0] as f32).to_bits(),
                (ours[1] as f32).to_bits(),
                (ours[2] as f32).to_bits(),
            ];
            if ours_b == [c.refr[i][0], c.refr[i][1], c.refr[i][2]] {
                continue;
            }

            let mag = (t1[0] * t1[0] + t1[1] * t1[1] + t1[2] * t1[2]).sqrt();
            if mag > 1e-3 {
                continue;
            }

            let rho0 = w_a / w_b;
            let refd = [
                f32::from_bits(c.refr[i][0]) as f64,
                f32::from_bits(c.refr[i][1]) as f64,
                f32::from_bits(c.refr[i][2]) as f64,
            ];
            let err = |rho: f64| -> f64 {
                let t = [
                    rho * s_a[0] + s_b[0],
                    rho * s_a[1] + s_b[1],
                    rho * s_a[2] + s_b[2],
                ];
                let p = proj(t);
                let dx = p[0] - refd[0];
                let dy = p[1] - refd[1];
                let dz = p[2] - refd[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };

            let mut best = (err(rho0), rho0);
            for &sgn in &[1.0f64, -1.0] {
                for k in -6000..=6000 {
                    let rho = sgn * rho0 * (k as f64 * 1e-3).exp();
                    let e = err(rho);
                    if e < best.0 {
                        best = (e, rho);
                    }
                }
            }
            for round in 0..6 {
                let span = 1e-3 / 10f64.powi(round);
                let center = best.1;
                for k in -100..=100 {
                    let rho = center * (1.0 + k as f64 * span);
                    let e = err(rho);
                    if e < best.0 {
                        best = (e, rho);
                    }
                }
            }
            println!(
                "{} v{} |tan1|={:.2e} dens=({:.2e},{:.2e}) ratio_ours={:.12e} ratio_best={:.12e} rel_dev={:+.3e} resid={:.2e}",
                c.name.rsplit('/').next().unwrap(), i, mag, den_a, den_b,
                rho0, best.1, (best.1 - rho0) / rho0, best.0
            );
            shown += 1;
        }
    }
}
