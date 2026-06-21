use std::collections::{BTreeMap, HashMap};

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

fn accumulate(c: &MeshCase, remap: &[usize]) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let nslots = remap.iter().max().map(|m| m + 1).unwrap_or(0);
    let mut tan1 = vec![[0.0f64; 3]; nslots];
    let mut tan2 = vec![[0.0f64; 3]; nslots];
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
                (dot / (l1sq * l2sq).sqrt()).clamp(-1.0, 1.0).acos() * absden
            } else {
                0.0
            };
            let s = remap[tri[ci]];
            tan1[s][0] += wgt * sx;
            tan1[s][1] += wgt * sy;
            tan1[s][2] += wgt * sz;
            tan2[s][0] += wgt * tx;
            tan2[s][1] += wgt * ty;
            tan2[s][2] += wgt * tz;
        }
    }
    (tan1, tan2)
}

fn key_pos(c: &MeshCase, i: usize) -> Vec<u32> {
    c.pos[i].iter().map(|x| (*x as f32).to_bits()).collect()
}
fn key_pn(c: &MeshCase, i: usize) -> Vec<u32> {
    let mut k = key_pos(c, i);
    k.extend(c.nrm[i].iter().map(|x| (*x as f32).to_bits()));
    k
}
fn key_pnu(c: &MeshCase, i: usize) -> Vec<u32> {
    let mut k = key_pn(c, i);
    k.extend(c.uv[i].iter().map(|x| (*x as f32).to_bits()));
    k
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();
    let modes = ["identity", "weld-pos", "weld-pos+nrm", "weld-pos+nrm+uv"];
    let mut score: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
    let mut fixes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut breaks: BTreeMap<&str, usize> = BTreeMap::new();
    for fp in &files {
        let c = load(fp);
        let n = c.pos.len();
        let mut id_ok = vec![false; n];
        for (mi, mode) in modes.iter().enumerate() {
            let remap: Vec<usize> = match mi {
                0 => (0..n).collect(),
                _ => {
                    let mut map: HashMap<Vec<u32>, usize> = HashMap::new();
                    let mut rm = vec![0usize; n];
                    // `i` is passed by value to key_* and indexes rm; range loop is clearest.
                    #[allow(clippy::needless_range_loop)]
                    for i in 0..n {
                        let k = match mi {
                            1 => key_pos(&c, i),
                            2 => key_pn(&c, i),
                            _ => key_pnu(&c, i),
                        };
                        let next = map.len();
                        let slot = *map.entry(k).or_insert(next);
                        rm[i] = slot;
                    }
                    rm
                }
            };
            let (tan1, tan2) = accumulate(&c, &remap);
            let mut ok = 0usize;
            for i in 0..n {
                let got = finalize(c.nrm[i], tan1[remap[i]], tan2[remap[i]]);
                let good = got == c.refr[i];
                if good {
                    ok += 1;
                }
                if mi == 0 {
                    id_ok[i] = good;
                } else {
                    if good && !id_ok[i] {
                        *fixes.entry(mode).or_default() += 1;
                    }
                    if !good && id_ok[i] {
                        *breaks.entry(mode).or_default() += 1;
                    }
                }
            }
            let e = score.entry(mode).or_default();
            e.0 += ok;
            e.1 += n;
            if ok == n {
                e.2 += 1;
            }
        }
    }
    for (m, (ok, tot, me)) in &score {
        println!(
            "{:16} verts {:6}/{}  meshes-exact {}  fixed {} broke {}",
            m,
            ok,
            tot,
            me,
            fixes.get(m).copied().unwrap_or(0),
            breaks.get(m).copied().unwrap_or(0)
        );
    }
}
