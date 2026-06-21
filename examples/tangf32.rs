struct MeshCase {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
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

#[derive(Clone, Copy, Debug)]
struct V {
    normalize_dir: bool,
    wgt_mode: u8,
    recip_det: bool,
    recip_final: bool,
    lsq_combined: bool,
}

#[inline(always)]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn fb_axes(nn: [f32; 3]) -> ([f32; 3], [f32; 3]) {
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

fn finalize_f32(nn: [f32; 3], t: [f32; 3], tb: [f32; 3], v: V) -> [u32; 4] {
    let d = dot3(nn, t);
    let ox = t[0] - nn[0] * d;
    let oy = t[1] - nn[1] * d;
    let oz = t[2] - nn[2] * d;
    let mag = (ox * ox + oy * oy + oz * oz).sqrt();
    let (fb, b2) = fb_axes(nn);
    // NaN or magnitude <= 1e-6 both count as degenerate; the negated
    // compare is deliberate (`mag <= 1e-6` would let NaN slip through).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    let degenerate = !(mag > 1e-6f32);
    let (tgx, tgy, tgz);
    if !degenerate {
        if v.recip_final {
            let inv = 1.0f32 / mag;
            tgx = ox * inv;
            tgy = oy * inv;
            tgz = oz * inv;
        } else {
            tgx = ox / mag;
            tgy = oy / mag;
            tgz = oz / mag;
        }
    } else {
        let dd = dot3(nn, fb);
        let ox2 = fb[0] - nn[0] * dd;
        let oy2 = fb[1] - nn[1] * dd;
        let oz2 = fb[2] - nn[2] * dd;
        let mag2 = (ox2 * ox2 + oy2 * oy2 + oz2 * oz2).sqrt();
        if mag2 > 0.0 {
            if v.recip_final {
                let inv = 1.0f32 / mag2;
                tgx = ox2 * inv;
                tgy = oy2 * inv;
                tgz = oz2 * inv;
            } else {
                tgx = ox2 / mag2;
                tgy = oy2 / mag2;
                tgz = oz2 / mag2;
            }
        } else {
            tgx = fb[0];
            tgy = fb[1];
            tgz = fb[2];
        }
    }
    let cx = nn[1] * tgz - nn[2] * tgy;
    let cy = nn[2] * tgx - nn[0] * tgz;
    let cz = nn[0] * tgy - nn[1] * tgx;
    let cr = [cx, cy, cz];
    let w = if degenerate {
        if dot3(cr, b2) > 0.0 {
            1.0f32
        } else {
            -1.0
        }
    } else {
        let h = dot3(cr, tb);
        if h != 0.0 {
            if h > 0.0 {
                1.0
            } else {
                -1.0
            }
        } else if dot3(cr, b2) > 0.0 {
            1.0
        } else {
            -1.0
        }
    };
    [tgx.to_bits(), tgy.to_bits(), tgz.to_bits(), w.to_bits()]
}

fn calc_one(c: &MeshCase, i: usize, tri: [usize; 3], corner: usize, v: V) -> [u32; 4] {
    let (i1, i2, i3) = (tri[0], tri[1], tri[2]);
    let v1 = c.pos[i1];
    let v2 = c.pos[i2];
    let v3 = c.pos[i3];
    let w1 = c.uv[i1];
    let w2 = c.uv[i2];
    let w3 = c.uv[i3];
    let x1 = v2[0] - v1[0];
    let x2 = v3[0] - v1[0];
    let y1 = v2[1] - v1[1];
    let y2 = v3[1] - v1[1];
    let z1 = v2[2] - v1[2];
    let z2 = v3[2] - v1[2];
    let s1 = w2[0] - w1[0];
    let s2 = w3[0] - w1[0];
    let t1 = w2[1] - w1[1];
    let t2 = w3[1] - w1[1];
    let den: f32 = s1 * t2 - s2 * t1;
    if den == 0.0 {
        return [0x3f800000, 0, 0, 0x3f800000];
    }
    let r: f32 = if v.recip_det { 1.0f32 / den } else { 1.0 };
    let ap = |a: f32| if v.recip_det { a * r } else { a / den };
    let mut sx = ap(t2 * x1 - t1 * x2);
    let mut sy = ap(t2 * y1 - t1 * y2);
    let mut sz = ap(t2 * z1 - t1 * z2);
    let mut tx = ap(s1 * x2 - s2 * x1);
    let mut ty = ap(s1 * y2 - s2 * y1);
    let mut tz = ap(s1 * z2 - s2 * z1);
    if v.normalize_dir {
        let sl = (sx * sx + sy * sy + sz * sz).sqrt();
        if sl > 0.0 {
            if v.recip_final {
                let inv = 1.0f32 / sl;
                sx *= inv;
                sy *= inv;
                sz *= inv;
            } else {
                sx /= sl;
                sy /= sl;
                sz /= sl;
            }
        }
        let tl = (tx * tx + ty * ty + tz * tz).sqrt();
        if tl > 0.0 {
            if v.recip_final {
                let inv = 1.0f32 / tl;
                tx *= inv;
                ty *= inv;
                tz *= inv;
            } else {
                tx /= tl;
                ty /= tl;
                tz /= tl;
            }
        }
    }
    let absden = den.abs();
    let pv = [v1, v2, v3];
    let p0 = pv[corner];
    let pa = pv[(corner + 1) % 3];
    let pb = pv[(corner + 2) % 3];
    let e1 = [pa[0] - p0[0], pa[1] - p0[1], pa[2] - p0[2]];
    let e2 = [pb[0] - p0[0], pb[1] - p0[1], pb[2] - p0[2]];
    let l1sq = dot3(e1, e1);
    let l2sq = dot3(e2, e2);
    let wgt: f32 = match v.wgt_mode {
        0 => {
            if l1sq > 0.0 && l2sq > 0.0 {
                let dd = dot3(e1, e2);
                let cosv = if v.lsq_combined {
                    dd / (l1sq * l2sq).sqrt()
                } else {
                    dd / (l1sq.sqrt() * l2sq.sqrt())
                };
                cosv.clamp(-1.0, 1.0).acos() * absden
            } else {
                0.0
            }
        }
        1 => absden,
        2 => 1.0,
        _ => {
            if l1sq > 0.0 && l2sq > 0.0 {
                let dd = dot3(e1, e2);
                let cosv = dd / (l1sq * l2sq).sqrt();
                cosv.clamp(-1.0, 1.0).acos()
            } else {
                0.0
            }
        }
    };
    let t1c = [wgt * sx, wgt * sy, wgt * sz];
    let t2c = [wgt * tx, wgt * ty, wgt * tz];
    finalize_f32(c.nrm[i], t1c, t2c, v)
}

fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".txt"))
        .collect();
    files.sort();
    let cases: Vec<MeshCase> = files.iter().map(|f| load(f)).collect();

    let mut variants = vec![];
    for normalize_dir in [true, false] {
        for wgt_mode in 0..4u8 {
            for recip_det in [false, true] {
                for recip_final in [false, true] {
                    for lsq_combined in [true, false] {
                        variants.push(V {
                            normalize_dir,
                            wgt_mode,
                            recip_det,
                            recip_final,
                            lsq_combined,
                        });
                    }
                }
            }
        }
    }

    let mut tot1 = 0usize;
    let mut ok: Vec<usize> = vec![0; variants.len()];
    let mut wmatch: Vec<usize> = vec![0; variants.len()];
    for c in &cases {
        let n = c.pos.len();
        let mut inc: Vec<Vec<([usize; 3], usize)>> = vec![vec![]; n];
        for t in c.idx.chunks_exact(3) {
            let tri = [t[0] as usize, t[1] as usize, t[2] as usize];
            for ci in 0..3 {
                inc[tri[ci]].push((tri, ci));
            }
        }
        // `i` is passed by value to calc_one and indexes inc/c.refr; range loop is clearest.
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            if inc[i].len() != 1 {
                continue;
            }
            tot1 += 1;
            let (tri, ci) = inc[i][0];
            for (vi, v) in variants.iter().enumerate() {
                let g = calc_one(c, i, tri, ci, *v);
                if g == c.refr[i] {
                    ok[vi] += 1;
                }
                if g[..3] == c.refr[i][..3] {
                    wmatch[vi] += 1;
                }
            }
        }
    }
    println!("single-tri verts (diverging meshes only): {}", tot1);
    let mut rows: Vec<usize> = (0..variants.len()).collect();
    rows.sort_by_key(|&vi| std::cmp::Reverse(ok[vi]));
    for &vi in rows.iter().take(24) {
        println!(
            "exact4 {:6}  xyz3 {:6}  :: {:?}",
            ok[vi], wmatch[vi], variants[vi]
        );
    }
}
