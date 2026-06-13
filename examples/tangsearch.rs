use std::cmp::Reverse;

#[derive(Clone)]
struct MeshCase {
    pos: Vec<[f64; 3]>,
    nrm: Vec<[f64; 3]>,
    uv: Vec<[f64; 2]>,
    idx: Vec<u32>,
    ours: Vec<[u32; 4]>,
    refr: Vec<[u32; 4]>,
}

fn load(path: &str) -> MeshCase {
    let mut c = MeshCase {
        pos: vec![],
        nrm: vec![],
        uv: vec![],
        idx: vec![],
        ours: vec![],
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
            Some("O") => c.ours.push([
                b(it.next().unwrap()),
                b(it.next().unwrap()),
                b(it.next().unwrap()),
                b(it.next().unwrap()),
            ]),
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct V {
    st_f32: bool,
    dirnorm_f32: bool,
    wgt_f32: bool,
    acc_f32: bool,
    fin_f32: bool,
    norm_recip: bool,
    fin_recip: bool,
    gate: f64,
    corner_round: bool,
    dir_store_f32: bool,
    wgt_store_f32: bool,
    lsq_combined: bool,
    raw_store_f32: bool,
    edge_chain: bool,
}

macro_rules! op {
    ($f32:expr, $a:expr, $op:tt, $b:expr) => {
        if $f32 { (($a as f32) $op ($b as f32)) as f64 } else { $a $op $b }
    };
}
fn fsqrt(f32m: bool, x: f64) -> f64 {
    if f32m {
        (x as f32).sqrt() as f64
    } else {
        x.sqrt()
    }
}
fn facos(f32m: bool, x: f64) -> f64 {
    if f32m {
        (x as f32).acos() as f64
    } else {
        x.acos()
    }
}

fn tangents(c: &MeshCase, v: V) -> Vec<[u32; 4]> {
    let n = c.pos.len();
    if c.uv.is_empty() || c.idx.is_empty() {
        return vec![[0x3f800000, 0, 0, 0x3f800000]; n];
    }
    let r32 = |x: f64| x as f32 as f64;
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
        let g = v.st_f32;
        let den = op!(g, op!(g, s1, *, t2), -, op!(g, s2, *, t1));
        if den == 0.0 {
            continue;
        }
        let r = op!(g, 1.0f64, /, den);
        let mut sx = op!(g, op!(g, op!(g, t2, *, x1), -, op!(g, t1, *, x2)), *, r);
        let mut sy = op!(g, op!(g, op!(g, t2, *, y1), -, op!(g, t1, *, y2)), *, r);
        let mut sz = op!(g, op!(g, op!(g, t2, *, z1), -, op!(g, t1, *, z2)), *, r);
        let mut tx = op!(g, op!(g, op!(g, s1, *, x2), -, op!(g, s2, *, x1)), *, r);
        let mut ty = op!(g, op!(g, op!(g, s1, *, y2), -, op!(g, s2, *, y1)), *, r);
        let mut tz = op!(g, op!(g, op!(g, s1, *, z2), -, op!(g, s2, *, z1)), *, r);
        if v.raw_store_f32 {
            sx = r32(sx);
            sy = r32(sy);
            sz = r32(sz);
            tx = r32(tx);
            ty = r32(ty);
            tz = r32(tz);
        }
        let dn = v.dirnorm_f32;
        let sl = fsqrt(
            dn,
            op!(dn, op!(dn, op!(dn, sx, *, sx), +, op!(dn, sy, *, sy)), +, op!(dn, sz, *, sz)),
        );
        if sl > 0.0 {
            if v.norm_recip {
                let i = op!(dn, 1.0f64, /, sl);
                sx = op!(dn, sx, *, i);
                sy = op!(dn, sy, *, i);
                sz = op!(dn, sz, *, i);
            } else {
                sx = op!(dn, sx, /, sl);
                sy = op!(dn, sy, /, sl);
                sz = op!(dn, sz, /, sl);
            }
        }
        let tl = fsqrt(
            dn,
            op!(dn, op!(dn, op!(dn, tx, *, tx), +, op!(dn, ty, *, ty)), +, op!(dn, tz, *, tz)),
        );
        if tl > 0.0 {
            if v.norm_recip {
                let i = op!(dn, 1.0f64, /, tl);
                tx = op!(dn, tx, *, i);
                ty = op!(dn, ty, *, i);
                tz = op!(dn, tz, *, i);
            } else {
                tx = op!(dn, tx, /, tl);
                ty = op!(dn, ty, /, tl);
                tz = op!(dn, tz, /, tl);
            }
        }
        if v.dir_store_f32 {
            sx = r32(sx);
            sy = r32(sy);
            sz = r32(sz);
            tx = r32(tx);
            ty = r32(ty);
            tz = r32(tz);
        }
        let absden = den.abs();
        let tri = [i1, i2, i3];
        let pv = [v1, v2, v3];
        let wf = v.wgt_f32;
        for ci in 0..3 {
            let (mut e1x, mut e1y, mut e1z, mut e2x, mut e2y, mut e2z);
            if v.edge_chain {
                let cx = r32(x2 - x1);
                let cy = r32(y2 - y1);
                let cz = r32(z2 - z1);
                match ci {
                    0 => {
                        e1x = x1;
                        e1y = y1;
                        e1z = z1;
                        e2x = x2;
                        e2y = y2;
                        e2z = z2;
                    }
                    1 => {
                        e1x = cx;
                        e1y = cy;
                        e1z = cz;
                        e2x = -x1;
                        e2y = -y1;
                        e2z = -z1;
                    }
                    _ => {
                        e1x = -x2;
                        e1y = -y2;
                        e1z = -z2;
                        e2x = -cx;
                        e2y = -cy;
                        e2z = -cz;
                    }
                }
            } else {
                let p0 = pv[ci];
                let pa = pv[(ci + 1) % 3];
                let pb = pv[(ci + 2) % 3];
                e1x = pa[0] - p0[0];
                e1y = pa[1] - p0[1];
                e1z = pa[2] - p0[2];
                e2x = pb[0] - p0[0];
                e2y = pb[1] - p0[1];
                e2z = pb[2] - p0[2];
                if v.corner_round {
                    e1x = r32(e1x);
                    e1y = r32(e1y);
                    e1z = r32(e1z);
                    e2x = r32(e2x);
                    e2y = r32(e2y);
                    e2z = r32(e2z);
                }
            }
            let l1sq = op!(wf, op!(wf, op!(wf, e1x, *, e1x), +, op!(wf, e1y, *, e1y)), +, op!(wf, e1z, *, e1z));
            let l2sq = op!(wf, op!(wf, op!(wf, e2x, *, e2x), +, op!(wf, e2y, *, e2y)), +, op!(wf, e2z, *, e2z));
            let l1 = fsqrt(wf, l1sq);
            let l2 = fsqrt(wf, l2sq);
            let mut wgt = if l1 > 0.0 && l2 > 0.0 {
                let dot = op!(wf, op!(wf, op!(wf, e1x, *, e2x), +, op!(wf, e1y, *, e2y)), +, op!(wf, e1z, *, e2z));
                let d = if v.lsq_combined {
                    op!(wf, dot, /, fsqrt(wf, op!(wf, l1sq, *, l2sq))).clamp(-1.0, 1.0)
                } else {
                    op!(wf, dot, /, op!(wf, l1, *, l2)).clamp(-1.0, 1.0)
                };
                op!(wf, facos(wf, d), *, absden)
            } else {
                0.0
            };
            if v.wgt_store_f32 {
                wgt = r32(wgt);
            }
            let vi = tri[ci];
            let af = v.acc_f32;
            tan1[vi][0] = op!(af, tan1[vi][0], +, op!(af, wgt, *, sx));
            tan1[vi][1] = op!(af, tan1[vi][1], +, op!(af, wgt, *, sy));
            tan1[vi][2] = op!(af, tan1[vi][2], +, op!(af, wgt, *, sz));
            tan2[vi][0] = op!(af, tan2[vi][0], +, op!(af, wgt, *, tx));
            tan2[vi][1] = op!(af, tan2[vi][1], +, op!(af, wgt, *, ty));
            tan2[vi][2] = op!(af, tan2[vi][2], +, op!(af, wgt, *, tz));
        }
    }
    let ff = v.fin_f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let nn = c.nrm[i];
        let t = tan1[i];
        let d = op!(ff, op!(ff, op!(ff, nn[0], *, t[0]), +, op!(ff, nn[1], *, t[1])), +, op!(ff, nn[2], *, t[2]));
        let ox = op!(ff, t[0], -, op!(ff, nn[0], *, d));
        let oy = op!(ff, t[1], -, op!(ff, nn[1], *, d));
        let oz = op!(ff, t[2], -, op!(ff, nn[2], *, d));
        let mag = fsqrt(
            ff,
            op!(ff, op!(ff, op!(ff, ox, *, ox), +, op!(ff, oy, *, oy)), +, op!(ff, oz, *, oz)),
        );
        let ax = nn[0].abs();
        let ay = nn[1].abs();
        let az = nn[2].abs();
        let (fbx, fby, fbz, bx, by, bz) = if ax <= ay && ax <= az {
            if ay <= az {
                (1.0f64, 0.0, 0.0, 0.0, 1.0, 0.0)
            } else {
                (1.0, 0.0, 0.0, 0.0, 0.0, 1.0)
            }
        } else if ay <= az {
            if ax <= az {
                (0.0, 1.0, 0.0, 1.0, 0.0, 0.0)
            } else {
                (0.0, 1.0, 0.0, 0.0, 0.0, 1.0)
            }
        } else if ax <= ay {
            (0.0, 0.0, 1.0, 1.0, 0.0, 0.0)
        } else {
            (0.0, 0.0, 1.0, 0.0, 1.0, 0.0)
        };
        let degenerate = !(mag > v.gate);
        let (tgx, tgy, tgz);
        if !degenerate {
            if v.fin_recip {
                let inv = op!(ff, 1.0f64, /, mag);
                tgx = op!(ff, ox, *, inv);
                tgy = op!(ff, oy, *, inv);
                tgz = op!(ff, oz, *, inv);
            } else {
                tgx = op!(ff, ox, /, mag);
                tgy = op!(ff, oy, /, mag);
                tgz = op!(ff, oz, /, mag);
            }
        } else {
            let dd = op!(ff, op!(ff, op!(ff, nn[0], *, fbx), +, op!(ff, nn[1], *, fby)), +, op!(ff, nn[2], *, fbz));
            let ox2 = op!(ff, fbx, -, op!(ff, nn[0], *, dd));
            let oy2 = op!(ff, fby, -, op!(ff, nn[1], *, dd));
            let oz2 = op!(ff, fbz, -, op!(ff, nn[2], *, dd));
            let mag2 = fsqrt(
                ff,
                op!(ff, op!(ff, op!(ff, ox2, *, ox2), +, op!(ff, oy2, *, oy2)), +, op!(ff, oz2, *, oz2)),
            );
            if mag2 > 0.0 {
                if v.fin_recip {
                    let inv = op!(ff, 1.0f64, /, mag2);
                    tgx = op!(ff, ox2, *, inv);
                    tgy = op!(ff, oy2, *, inv);
                    tgz = op!(ff, oz2, *, inv);
                } else {
                    tgx = op!(ff, ox2, /, mag2);
                    tgy = op!(ff, oy2, /, mag2);
                    tgz = op!(ff, oz2, /, mag2);
                }
            } else {
                tgx = fbx;
                tgy = fby;
                tgz = fbz;
            }
        }
        let cx = op!(ff, op!(ff, nn[1], *, tgz), -, op!(ff, nn[2], *, tgy));
        let cy = op!(ff, op!(ff, nn[2], *, tgx), -, op!(ff, nn[0], *, tgz));
        let cz = op!(ff, op!(ff, nn[0], *, tgy), -, op!(ff, nn[1], *, tgx));
        let tb = tan2[i];
        let w = if degenerate {
            let h =
                op!(ff, op!(ff, op!(ff, cx, *, bx), +, op!(ff, cy, *, by)), +, op!(ff, cz, *, bz));
            if h > 0.0 {
                1.0f64
            } else {
                -1.0
            }
        } else {
            let h = op!(ff, op!(ff, op!(ff, cx, *, tb[0]), +, op!(ff, cy, *, tb[1])), +, op!(ff, cz, *, tb[2]));
            if h != 0.0 {
                if h > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            } else {
                let h2 = op!(ff, op!(ff, op!(ff, cx, *, bx), +, op!(ff, cy, *, by)), +, op!(ff, cz, *, bz));
                if h2 > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
        };
        out.push([
            (tgx as f32).to_bits(),
            (tgy as f32).to_bits(),
            (tgz as f32).to_bits(),
            (w as f32).to_bits(),
        ]);
    }
    out
}

fn score(cases: &[MeshCase], v: V) -> (usize, usize, usize, usize) {
    let mut me = 0;
    let mut ve = 0;
    let mut ce = 0;
    let mut tc = 0;
    for c in cases {
        let got = tangents(c, v);
        let mut ok = true;
        for (g, r) in got.iter().zip(c.refr.iter()) {
            let mut vok = true;
            for k in 0..4 {
                tc += 1;
                if g[k] == r[k] {
                    ce += 1
                } else {
                    vok = false
                }
            }
            if vok {
                ve += 1
            } else {
                ok = false
            }
        }
        if ok {
            me += 1
        }
    }
    (me, ve, ce, tc)
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
    println!("loaded {} meshes", cases.len());

    let base = V {
        st_f32: false,
        dirnorm_f32: false,
        wgt_f32: false,
        acc_f32: false,
        fin_f32: false,
        norm_recip: false,
        fin_recip: false,
        gate: 1e-6,
        corner_round: false,
        dir_store_f32: false,
        wgt_store_f32: false,
        lsq_combined: false,
        raw_store_f32: false,
        edge_chain: false,
    };
    let s = score(&cases, base);
    println!(
        "BASE (current shipped arithmetic): meshes {:3} verts {:6} comps {:7}/{}",
        s.0, s.1, s.2, s.3
    );

    let mut results: Vec<(String, (usize, usize, usize, usize))> = vec![];

    for bits in 0..256u32 {
        let v = V {
            norm_recip: bits & 1 != 0,
            fin_recip: bits & 2 != 0,
            dir_store_f32: bits & 4 != 0,
            wgt_store_f32: bits & 8 != 0,
            lsq_combined: bits & 16 != 0,
            raw_store_f32: bits & 32 != 0,
            fin_f32: bits & 64 != 0,
            edge_chain: bits & 128 != 0,
            corner_round: true,
            ..base
        };
        results.push((format!("{:?}", v), score(&cases, v)));
    }
    results.sort_by_key(|(_, s)| Reverse(s.2));
    for (k, s) in results.iter().take(15) {
        println!(
            "meshes {:3} verts {:6} comps {:7}/{} :: {}",
            s.0, s.1, s.2, s.3, k
        );
    }
}
