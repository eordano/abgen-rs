use std::collections::BTreeMap;

struct MeshCase {
    name: String,
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
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
    let f = |s: &str| f32::from_bits(u32::from_str_radix(s, 16).unwrap());
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

fn ulp_dist(a: u32, b: u32) -> i64 {
    fn key(x: u32) -> i64 {
        let s = x >> 31;
        let mag = (x & 0x7fff_ffff) as i64;
        if s == 1 {
            -mag
        } else {
            mag
        }
    }
    (key(a) - key(b)).abs()
}

fn fb_axis(nn: [f64; 3]) -> ([f64; 3], [f64; 3]) {
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

const fn f32r(x: f64) -> f64 {
    x as f32 as f64
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let detail = std::env::args().nth(2).is_some();
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();
    let mut hist: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut detail_rows = 0usize;
    for fpath in &files {
        let c = load(fpath);
        let pos: Vec<[f64; 3]> = c
            .pos
            .iter()
            .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
            .collect();
        let nrm: Vec<[f64; 3]> = c
            .nrm
            .iter()
            .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
            .collect();
        let uv: Vec<[f64; 2]> = c.uv.iter().map(|p| [p[0] as f64, p[1] as f64]).collect();
        let got = abgen::tangents::calculate_tangents(&pos, &nrm, &uv, &c.idx);

        let n = pos.len();
        let mut tan1 = vec![[0.0f64; 3]; n];
        let mut tan2 = vec![[0.0f64; 3]; n];
        let mut zero_area_hits = vec![0u32; n];
        let m = (c.idx.len() / 3) * 3;
        let mut k = 0;
        while k < m {
            let i1 = c.idx[k] as usize;
            let i2 = c.idx[k + 1] as usize;
            let i3 = c.idx[k + 2] as usize;
            k += 3;
            let v1 = pos[i1];
            let v2 = pos[i2];
            let v3 = pos[i3];
            let w1 = uv[i1];
            let w2 = uv[i2];
            let w3 = uv[i3];
            let x1 = f32r(v2[0] - v1[0]);
            let x2 = f32r(v3[0] - v1[0]);
            let y1 = f32r(v2[1] - v1[1]);
            let y2 = f32r(v3[1] - v1[1]);
            let z1 = f32r(v2[2] - v1[2]);
            let z2 = f32r(v3[2] - v1[2]);
            let s1 = f32r(w2[0] - w1[0]);
            let s2 = f32r(w3[0] - w1[0]);
            let t1 = f32r(w2[1] - w1[1]);
            let t2 = f32r(w3[1] - w1[1]);
            let den = s1 * t2 - s2 * t1;
            if den == 0.0 {
                zero_area_hits[i1] += 1;
                zero_area_hits[i2] += 1;
                zero_area_hits[i3] += 1;
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
                let e1 = [
                    f32r(pa[0] - p0[0]),
                    f32r(pa[1] - p0[1]),
                    f32r(pa[2] - p0[2]),
                ];
                let e2 = [
                    f32r(pb[0] - p0[0]),
                    f32r(pb[1] - p0[1]),
                    f32r(pb[2] - p0[2]),
                ];
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

        for i in 0..n {
            let g = [
                (got[i][0] as f32).to_bits(),
                (got[i][1] as f32).to_bits(),
                (got[i][2] as f32).to_bits(),
                (got[i][3] as f32).to_bits(),
            ];
            let r = c.refr[i];
            if g == r {
                continue;
            }
            let max_ulp = (0..3).map(|k| ulp_dist(g[k], r[k])).max().unwrap();
            let w_flip = g[3] != r[3];

            let nn = nrm[i];
            let (fb, _b2) = fb_axis(nn);
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
            let ref_is_fb = (0..3).all(|k| ulp_dist(fbt[k].to_bits(), r[k]) <= 1);
            let our_is_fb = (0..3).all(|k| ulp_dist(fbt[k].to_bits(), g[k]) <= 1);

            let t = tan1[i];
            let d = nn[0] * t[0] + nn[1] * t[1] + nn[2] * t[2];
            let ox = t[0] - nn[0] * d;
            let oy = t[1] - nn[1] * d;
            let oz = t[2] - nn[2] * d;
            let mag = (ox * ox + oy * oy + oz * oz).sqrt();
            let t2m = (tan2[i][0] * tan2[i][0] + tan2[i][1] * tan2[i][1] + tan2[i][2] * tan2[i][2])
                .sqrt();

            let gv = [
                f32::from_bits(g[0]) as f64,
                f32::from_bits(g[1]) as f64,
                f32::from_bits(g[2]) as f64,
            ];
            let rv = [
                f32::from_bits(r[0]) as f64,
                f32::from_bits(r[1]) as f64,
                f32::from_bits(r[2]) as f64,
            ];
            let gn = (gv[0] * gv[0] + gv[1] * gv[1] + gv[2] * gv[2]).sqrt();
            let rn = (rv[0] * rv[0] + rv[1] * rv[1] + rv[2] * rv[2]).sqrt();
            let ang = if gn > 0.0 && rn > 0.0 {
                ((gv[0] * rv[0] + gv[1] * rv[1] + gv[2] * rv[2]) / (gn * rn))
                    .clamp(-1.0, 1.0)
                    .acos()
                    .to_degrees()
            } else {
                0.0
            };
            let class = if max_ulp <= 4 && !w_flip {
                "ulp<=4"
            } else if ref_is_fb && !our_is_fb {
                "ref-fallback-we-real"
            } else if our_is_fb && !ref_is_fb {
                "we-fallback-ref-real"
            } else if max_ulp <= 4 && w_flip {
                "w-flip-only"
            } else if ang < 0.5 {
                "big-ULPnoise(<0.5deg)"
            } else {
                "big-DIRFLIP(>=0.5deg)"
            };
            *hist.entry(class).or_default() += 1;
            if detail && class != "ulp<=4" && detail_rows < 200 {
                println!(
                    "{} v{} {} ulp={} wflip={} |tan1|={:.3e} |ortho|={:.3e} |tan2|={:.3e} zerohits={} g=({:08x},{:08x},{:08x},{:08x}) r=({:08x},{:08x},{:08x},{:08x})",
                    c.name.rsplit('/').next().unwrap(), i, class, max_ulp, w_flip,
                    (t[0]*t[0]+t[1]*t[1]+t[2]*t[2]).sqrt(), mag, t2m, zero_area_hits[i],
                    g[0], g[1], g[2], g[3], r[0], r[1], r[2], r[3]
                );
                detail_rows += 1;
            }
        }
    }
    println!("CLASS HISTOGRAM:");
    for (k, v) in &hist {
        println!("  {:24} {}", k, v);
    }
}
