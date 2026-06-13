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

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let detail = std::env::args().nth(2).is_some();
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();
    let mut hist: BTreeMap<String, usize> = BTreeMap::new();
    let mut shown = 0usize;
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
            if inc[i].len() != 1 {
                continue;
            }
            let (tri, _ci) = inc[i][0];
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
            let den64 = s1 * t2 - s2 * t1;
            let den32 = (s1 as f32) * (t2 as f32) - (s2 as f32) * (t1 as f32);
            let raw_s = [t2 * x1 - t1 * x2, t2 * y1 - t1 * y2, t2 * z1 - t1 * z2];
            let raw_t = [s1 * x2 - s2 * x1, s1 * y2 - s2 * y1, s1 * z2 - s2 * z1];

            let mut cands: Vec<(&str, [u32; 4])> = vec![];
            cands.push(("skip", finalize(c.nrm[i], [0.0; 3], [0.0; 3])));
            {
                let norm = |v: [f64; 3]| {
                    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    if l > 0.0 {
                        [v[0] / l, v[1] / l, v[2] / l]
                    } else {
                        v
                    }
                };
                cands.push(("rawnorm", finalize(c.nrm[i], norm(raw_s), norm(raw_t))));
                let s = if den64 < 0.0 { -1.0 } else { 1.0 };
                let ns = norm(raw_s);
                let nt = norm(raw_t);
                cands.push((
                    "rawnorm*sign",
                    finalize(
                        c.nrm[i],
                        [ns[0] * s, ns[1] * s, ns[2] * s],
                        [nt[0] * s, nt[1] * s, nt[2] * s],
                    ),
                ));
                if den64 != 0.0 {
                    let r = 1.0 / den64;
                    let sc = norm([raw_s[0] * r, raw_s[1] * r, raw_s[2] * r]);
                    let tc = norm([raw_t[0] * r, raw_t[1] * r, raw_t[2] * r]);
                    cands.push(("current", finalize(c.nrm[i], sc, tc)));
                }
            }
            let base = cands
                .iter()
                .find(|(n2, _)| *n2 == if den64 == 0.0 { "skip" } else { "current" })
                .unwrap()
                .1;
            if base == c.refr[i] {
                continue;
            }
            let denclass = if den64 == 0.0 {
                "den64==0"
            } else if den32 == 0.0 {
                "den32==0,den64!=0"
            } else if den64.abs() < 1e-8 {
                "|den|<1e-8"
            } else if den64.abs() < 1e-4 {
                "|den|<1e-4"
            } else {
                "den-normal"
            };
            let mhit = cands
                .iter()
                .filter(|(_, v)| *v == c.refr[i])
                .map(|(n2, _)| *n2)
                .collect::<Vec<_>>()
                .join("+");
            let key = format!(
                "{} -> {}",
                denclass,
                if mhit.is_empty() {
                    "NONE".to_string()
                } else {
                    mhit
                }
            );
            *hist.entry(key).or_default() += 1;
            if detail && shown < 30 {
                println!(
                    "{} v{} den64={:.6e} den32={:.6e} |raw_s|={:.3e} ref=({:08x},{:08x},{:08x},{:08x})",
                    c.name.rsplit('/').next().unwrap(), i, den64, den32,
                    (raw_s[0] * raw_s[0] + raw_s[1] * raw_s[1] + raw_s[2] * raw_s[2]).sqrt(),
                    c.refr[i][0], c.refr[i][1], c.refr[i][2], c.refr[i][3]
                );
                shown += 1;
            }
        }
    }
    println!("BAD 1-TRI VERTS by den class and matching candidate:");
    let mut rows: Vec<_> = hist.into_iter().collect();
    rows.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
    for (k, v) in rows {
        println!("  {:6} {}", v, k);
    }
}
