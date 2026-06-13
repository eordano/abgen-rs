use std::collections::BTreeMap;

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

fn accumulate(c: &MeshCase) -> (Vec<[f64; 3]>, Vec<[f64; 3]>, Vec<u32>) {
    let n = c.pos.len();
    let mut tan1 = vec![[0.0f64; 3]; n];
    let mut tan2 = vec![[0.0f64; 3]; n];
    let mut tris = vec![0u32; n];
    let m = (c.idx.len() / 3) * 3;
    let mut k = 0;
    while k < m {
        let i1 = c.idx[k] as usize;
        let i2 = c.idx[k + 1] as usize;
        let i3 = c.idx[k + 2] as usize;
        k += 3;
        tris[i1] += 1;
        tris[i2] += 1;
        tris[i3] += 1;
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
        let r = 1.0 / den;
        let mut sx = (t2 * x1 - t1 * x2) * r;
        let mut sy = (t2 * y1 - t1 * y2) * r;
        let mut sz = (t2 * z1 - t1 * z2) * r;
        let mut tx = (s1 * x2 - s2 * x1) * r;
        let mut ty = (s1 * y2 - s2 * y1) * r;
        let mut tz = (s1 * z2 - s2 * z1) * r;
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
                let d = (dot / (l1sq * l2sq).sqrt()).clamp(-1.0, 1.0);
                d.acos() * absden
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
    (tan1, tan2, tris)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct F {
    f32mode: bool,
    n_renorm: bool,
    recip: bool,
    gate_pow: i32,
    gate_sq: bool,
    tan2_norm: bool,
}

fn finalize(nn0: [f64; 3], t: [f64; 3], tb: [f64; 3], f: F) -> [u32; 4] {
    macro_rules! o {
        ($a:expr, $op:tt, $b:expr) => { if f.f32mode { (($a as f32) $op ($b as f32)) as f64 } else { $a $op $b } };
    }
    let sq = |x: f64| {
        if f.f32mode {
            (x as f32).sqrt() as f64
        } else {
            x.sqrt()
        }
    };
    let mut nn = nn0;
    if f.n_renorm {
        let nm = sq(o!(o!(o!(nn[0], *, nn[0]), +, o!(nn[1], *, nn[1])), +, o!(nn[2], *, nn[2])));
        if nm > 0.0 {
            let inv = o!(1.0f64, /, nm);
            nn = [o!(nn[0], *, inv), o!(nn[1], *, inv), o!(nn[2], *, inv)];
        }
    }
    let d = o!(o!(o!(nn[0], *, t[0]), +, o!(nn[1], *, t[1])), +, o!(nn[2], *, t[2]));
    let ox = o!(t[0], -, o!(nn[0], *, d));
    let oy = o!(t[1], -, o!(nn[1], *, d));
    let oz = o!(t[2], -, o!(nn[2], *, d));
    let magsq = o!(o!(o!(ox, *, ox), +, o!(oy, *, oy)), +, o!(oz, *, oz));
    let mag = sq(magsq);
    let gate = 10f64.powi(-f.gate_pow);
    let degenerate = if f.gate_sq {
        !(magsq > gate)
    } else {
        !(mag > gate)
    };
    let (fb, b2) = fb_axes(nn);
    let (tgx, tgy, tgz);
    if !degenerate {
        if f.recip {
            let inv = o!(1.0f64, /, mag);
            tgx = o!(ox, *, inv);
            tgy = o!(oy, *, inv);
            tgz = o!(oz, *, inv);
        } else {
            tgx = o!(ox, /, mag);
            tgy = o!(oy, /, mag);
            tgz = o!(oz, /, mag);
        }
    } else {
        let dd = o!(o!(o!(nn[0], *, fb[0]), +, o!(nn[1], *, fb[1])), +, o!(nn[2], *, fb[2]));
        let ox2 = o!(fb[0], -, o!(nn[0], *, dd));
        let oy2 = o!(fb[1], -, o!(nn[1], *, dd));
        let oz2 = o!(fb[2], -, o!(nn[2], *, dd));
        let mag2 = sq(o!(o!(o!(ox2, *, ox2), +, o!(oy2, *, oy2)), +, o!(oz2, *, oz2)));
        if mag2 > 0.0 {
            if f.recip {
                let inv = o!(1.0f64, /, mag2);
                tgx = o!(ox2, *, inv);
                tgy = o!(oy2, *, inv);
                tgz = o!(oz2, *, inv);
            } else {
                tgx = o!(ox2, /, mag2);
                tgy = o!(oy2, /, mag2);
                tgz = o!(oz2, /, mag2);
            }
        } else {
            tgx = fb[0];
            tgy = fb[1];
            tgz = fb[2];
        }
    }
    let cx = o!(o!(nn[1], *, tgz), -, o!(nn[2], *, tgy));
    let cy = o!(o!(nn[2], *, tgx), -, o!(nn[0], *, tgz));
    let cz = o!(o!(nn[0], *, tgy), -, o!(nn[1], *, tgx));
    let mut tbv = tb;
    if f.tan2_norm {
        let d1 = tbv[0] * nn[0] + tbv[1] * nn[1] + tbv[2] * nn[2];
        tbv = [
            tbv[0] - nn[0] * d1,
            tbv[1] - nn[1] * d1,
            tbv[2] - nn[2] * d1,
        ];
        let d2 = tbv[0] * tgx + tbv[1] * tgy + tbv[2] * tgz;
        tbv = [tbv[0] - tgx * d2, tbv[1] - tgy * d2, tbv[2] - tgz * d2];
        let m2 = (tbv[0] * tbv[0] + tbv[1] * tbv[1] + tbv[2] * tbv[2]).sqrt();
        if m2 > 0.0 {
            tbv = [tbv[0] / m2, tbv[1] / m2, tbv[2] / m2];
        }
    }
    let w = if degenerate {
        let h = cx * b2[0] + cy * b2[1] + cz * b2[2];
        if h > 0.0 {
            1.0f64
        } else {
            -1.0
        }
    } else {
        let h = cx * tbv[0] + cy * tbv[1] + cz * tbv[2];
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

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();

    let mut variants = vec![];
    for &f32mode in &[false, true] {
        for &n_renorm in &[false, true] {
            for &recip in &[false, true] {
                for &gate_pow in &[6, 12] {
                    for &gate_sq in &[false, true] {
                        for &tan2_norm in &[false, true] {
                            variants.push(F {
                                f32mode,
                                n_renorm,
                                recip,
                                gate_pow,
                                gate_sq,
                                tan2_norm,
                            });
                        }
                    }
                }
            }
        }
    }

    let mut fixed: BTreeMap<usize, usize> = BTreeMap::new();
    let mut broken: BTreeMap<usize, usize> = BTreeMap::new();
    let mut tot_bad = 0usize;
    let mut tot_ok = 0usize;
    let base = F {
        f32mode: false,
        n_renorm: false,
        recip: false,
        gate_pow: 6,
        gate_sq: false,
        tan2_norm: false,
    };
    let mut bad_by_tris: BTreeMap<u32, usize> = BTreeMap::new();
    let mut all_by_tris: BTreeMap<u32, usize> = BTreeMap::new();
    for fp in &files {
        let c = load(fp);
        let (tan1, tan2, tris) = accumulate(&c);
        for i in 0..c.pos.len() {
            let basev = finalize(c.nrm[i], tan1[i], tan2[i], base);
            let was_ok = basev == c.refr[i];
            *all_by_tris.entry(tris[i].min(12)).or_default() += 1;
            if !was_ok {
                *bad_by_tris.entry(tris[i].min(12)).or_default() += 1;
            }
            if was_ok {
                tot_ok += 1
            } else {
                tot_bad += 1
            }
            for (vi, v) in variants.iter().enumerate() {
                let got = finalize(c.nrm[i], tan1[i], tan2[i], *v);
                let ok = got == c.refr[i];
                if !was_ok && ok {
                    *fixed.entry(vi).or_default() += 1;
                }
                if was_ok && !ok {
                    *broken.entry(vi).or_default() += 1;
                }
            }
        }
        let _ = c.name;
    }
    println!("base-bad verts {}   base-ok verts {}", tot_bad, tot_ok);
    println!("bad/all by incident tris (12=12+):");
    for (k, all) in &all_by_tris {
        let bad = bad_by_tris.get(k).copied().unwrap_or(0);
        println!("  tris={:2}  bad {:6} / {:7}", k, bad, all);
    }
    let mut rows: Vec<(usize, i64)> = (0..variants.len())
        .map(|vi| {
            let f = *fixed.get(&vi).unwrap_or(&0) as i64;
            let b = *broken.get(&vi).unwrap_or(&0) as i64;
            (vi, f - b)
        })
        .collect();
    rows.sort_by_key(|(_, net)| -net);
    for (vi, net) in rows.iter().take(20) {
        println!(
            "net {:6} fixed {:6} broke {:6} :: {:?}",
            net,
            fixed.get(vi).unwrap_or(&0),
            broken.get(vi).unwrap_or(&0),
            variants[*vi]
        );
    }
}
