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

fn ulp(a: u32, b: u32) -> i64 {
    fn key(x: u32) -> i64 {
        let s = x >> 31;
        let m = (x & 0x7fff_ffff) as i64;
        if s == 1 {
            -m
        } else {
            m
        }
    }
    (key(a) - key(b)).abs()
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
    let mut tot1 = 0usize;
    let mut bad1 = 0usize;

    let mut canc = 0usize;
    let mut small_ulp = 0usize;
    let mut big_other = 0usize;
    let mut dets = 0usize;
    for fp in &files {
        let c = load(fp);
        let n = c.pos.len();
        let got = abgen::tangents::calculate_tangents(&c.pos, &c.nrm, &c.uv, &c.idx);
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
            tot1 += 1;
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
            bad1 += 1;

            let (tri, corner) = inc[i][0];
            let r32 = |x: f64| x as f32 as f64;
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
            let (tan1, _tan2) = if den == 0.0 {
                ([0.0; 3], [0.0; 3])
            } else {
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
                let pv = [v1, v2, v3];
                let p0 = pv[corner];
                let pa = pv[(corner + 1) % 3];
                let pb = pv[(corner + 2) % 3];
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
                (
                    [wgt * sx, wgt * sy, wgt * sz],
                    [wgt * tx, wgt * ty, wgt * tz],
                )
            };
            let nn = c.nrm[i];
            let d = nn[0] * tan1[0] + nn[1] * tan1[1] + nn[2] * tan1[2];
            let ox = tan1[0] - nn[0] * d;
            let oy = tan1[1] - nn[1] * d;
            let oz = tan1[2] - nn[2] * d;
            let t1mag = (tan1[0] * tan1[0] + tan1[1] * tan1[1] + tan1[2] * tan1[2]).sqrt();
            let omag = (ox * ox + oy * oy + oz * oz).sqrt();
            let ratio = if t1mag > 0.0 { omag / t1mag } else { 1.0 };
            let max_ulp = (0..3).map(|k| ulp(g[k], r[k])).max().unwrap();
            let class = if ratio < 0.1 {
                canc += 1;
                "cancel"
            } else if max_ulp <= 2 {
                small_ulp += 1;
                "ulp<=2"
            } else {
                big_other += 1;
                "big"
            };
            if detail && dets < 60 && (class == "cancel" || class == "big") {
                println!("{} v{} {} ulp={} ratio={:.3e} |tan1|={:.2e} |ortho|={:.2e} g=({:08x},{:08x},{:08x}) r=({:08x},{:08x},{:08x})",
                    c.name.rsplit('/').next().unwrap(), i, class, max_ulp, ratio, t1mag, omag, g[0],g[1],g[2], r[0],r[1],r[2]);
                dets += 1;
            }
        }
    }
    println!("single-tri: total {} bad {}", tot1, bad1);
    println!("  cancel(|ortho|/|tan1|<0.1): {}", canc);
    println!("  ulp<=2 (xyz)             : {}", small_ulp);
    println!("  big-other               : {}", big_other);
}
