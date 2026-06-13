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
fn ulp_up(x: f64) -> f64 {
    f64::from_bits(x.to_bits() + 1)
}
fn ulp_dn(x: f64) -> f64 {
    f64::from_bits(x.to_bits() - 1)
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

fn contrib(c: &MeshCase, tri: [usize; 3], corner: usize) -> Option<([f64; 3], [f64; 3], f64)> {
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
    let mut t = [
        (s1 * x2 - s2 * x1) * r,
        (s1 * y2 - s2 * y1) * r,
        (s1 * z2 - s2 * z1) * r,
    ];
    let sl = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
    if sl > 0.0 {
        s = [s[0] / sl, s[1] / sl, s[2] / sl];
    }
    let tl = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
    if tl > 0.0 {
        t = [t[0] / tl, t[1] / tl, t[2] / tl];
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
    let wgt = if l1sq > 0.0 && l2sq > 0.0 {
        let d = (e1x * e2x + e1y * e2y + e1z * e2z) / (l1sq * l2sq).sqrt();
        d.clamp(-1.0, 1.0).acos() * den.abs()
    } else {
        0.0
    };
    Some((s, t, wgt))
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let max_tris: usize = std::env::args()
        .nth(2)
        .map(|s| s.parse().unwrap())
        .unwrap_or(5);
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();
    let mut tested = 0usize;
    let mut solved = 0usize;
    let mut unsolved = 0usize;
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
        for i in 0..n {
            let k = inc[i].len();
            if k == 0 || k > max_tris {
                continue;
            }
            let parts: Vec<([f64; 3], [f64; 3], f64)> = inc[i]
                .iter()
                .filter_map(|(tri, ci)| contrib(&c, *tri, *ci))
                .collect();
            let base_sum = |ws: &[f64]| -> ([f64; 3], [f64; 3]) {
                let mut t1 = [0.0f64; 3];
                let mut t2 = [0.0f64; 3];
                for (j, (s, t, _)) in parts.iter().enumerate() {
                    let w = ws[j];
                    t1[0] += w * s[0];
                    t1[1] += w * s[1];
                    t1[2] += w * s[2];
                    t2[0] += w * t[0];
                    t2[1] += w * t[1];
                    t2[2] += w * t[2];
                }
                (t1, t2)
            };
            let w0: Vec<f64> = parts.iter().map(|(_, _, w)| *w).collect();
            let (t1, t2) = base_sum(&w0);
            if finalize(c.nrm[i], t1, t2) == c.refr[i] {
                continue;
            }
            tested += 1;

            let np = parts.len();
            let mut hit = false;
            'outer: for mask in 0..3usize.pow(np as u32) {
                let mut m = mask;
                let mut ws = w0.clone();
                for j in 0..np {
                    match m % 3 {
                        1 => ws[j] = ulp_up(ws[j]),
                        2 => ws[j] = ulp_dn(ws[j]),
                        _ => {}
                    }
                    m /= 3;
                }
                let (t1, t2) = base_sum(&ws);
                if finalize(c.nrm[i], t1, t2) == c.refr[i] {
                    hit = true;
                    break 'outer;
                }
            }
            if hit {
                solved += 1
            } else {
                unsolved += 1
            }
        }
    }
    println!(
        "bad verts (<= {} tris): {}   ulp-weight-solvable {}   unsolvable {}",
        max_tris, tested, solved, unsolved
    );
}
