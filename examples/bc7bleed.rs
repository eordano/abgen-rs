use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::VecDeque;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

const PASSES: u32 = 32;

fn jacobi_fill(rgba: &mut [u8], w: usize, h: usize, order: &[usize], mean: bool, passes: u32) {
    let mut filled: Vec<u8> = (0..w * h).map(|i| u8::from(rgba[i * 4 + 3] > 0)).collect();
    let mut snap = rgba.to_vec();
    for _ in 0..passes {
        let mut any = false;
        let mut nf = filled.clone();
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if filled[idx] != 0 {
                    continue;
                }

                let neigh = |k: usize| -> Option<usize> {
                    match k {
                        0 if y > 0 => Some(idx - w),
                        1 if y + 1 < h => Some(idx + w),
                        2 if x > 0 => Some(idx - 1),
                        3 if x + 1 < w => Some(idx + 1),
                        _ => None,
                    }
                };
                if mean {
                    let (mut sr, mut sg, mut sb, mut cnt) = (0u32, 0u32, 0u32, 0u32);
                    for k in 0..4 {
                        if let Some(p) = neigh(k) {
                            if filled[p] != 0 {
                                sr += snap[p * 4] as u32;
                                sg += snap[p * 4 + 1] as u32;
                                sb += snap[p * 4 + 2] as u32;
                                cnt += 1;
                            }
                        }
                    }
                    if cnt > 0 {
                        let rh = |s: u32| -> u8 {
                            let q = s / cnt;
                            let r = s % cnt;
                            if r * 2 < cnt {
                                q as u8
                            } else if r * 2 > cnt || q & 1 != 0 {
                                (q + 1) as u8
                            } else {
                                q as u8
                            }
                        };
                        rgba[idx * 4] = rh(sr);
                        rgba[idx * 4 + 1] = rh(sg);
                        rgba[idx * 4 + 2] = rh(sb);
                        nf[idx] = 1;
                        any = true;
                    }
                } else {
                    for &k in order {
                        if let Some(p) = neigh(k) {
                            if filled[p] != 0 {
                                rgba[idx * 4] = snap[p * 4];
                                rgba[idx * 4 + 1] = snap[p * 4 + 1];
                                rgba[idx * 4 + 2] = snap[p * 4 + 2];
                                nf[idx] = 1;
                                any = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
        if !any {
            break;
        }
        filled = nf;
        snap.copy_from_slice(rgba);
    }
}

fn voronoi_fill(rgba: &mut [u8], w: usize, h: usize, l2: bool, cap: f64) {
    let n = w * h;
    let mut src: Vec<i32> = vec![-1; n];
    if !l2 {
        let mut dist = vec![u32::MAX; n];
        let mut q = VecDeque::new();
        for i in 0..n {
            if rgba[i * 4 + 3] > 0 {
                dist[i] = 0;
                src[i] = i as i32;
                q.push_back(i);
            }
        }
        while let Some(i) = q.pop_front() {
            let (x, y) = (i % w, i / w);
            let d = dist[i] + 1;
            if d as f64 > cap {
                continue;
            }
            let mut push = |j: usize| {
                if dist[j] > d {
                    dist[j] = d;
                    src[j] = src[i];
                    q.push_back(j);
                }
            };
            if y > 0 {
                push(i - w);
            }
            if y + 1 < h {
                push(i + w);
            }
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < w {
                push(i + 1);
            }
        }
    } else {
        let mut boundary = Vec::new();
        for i in 0..n {
            if rgba[i * 4 + 3] == 0 {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let mut b = false;
            if y > 0 && rgba[(i - w) * 4 + 3] == 0 {
                b = true;
            }
            if y + 1 < h && rgba[(i + w) * 4 + 3] == 0 {
                b = true;
            }
            if x > 0 && rgba[(i - 1) * 4 + 3] == 0 {
                b = true;
            }
            if x + 1 < w && rgba[(i + 1) * 4 + 3] == 0 {
                b = true;
            }
            if b {
                boundary.push(i);
            }
        }
        for i in 0..n {
            if rgba[i * 4 + 3] > 0 {
                continue;
            }
            let (x, y) = (i % w, i / w);
            let mut best = f64::MAX;
            let mut bi = -1i64;
            for &bidx in &boundary {
                let (bx, by) = (bidx % w, bidx / w);
                let dx = x as f64 - bx as f64;
                let dy = y as f64 - by as f64;
                let d = dx * dx + dy * dy;
                if d < best {
                    best = d;
                    bi = bidx as i64;
                }
            }
            if best.sqrt() <= cap {
                src[i] = bi as i32;
            }
        }
    }
    let snap = rgba.to_vec();
    for i in 0..n {
        if rgba[i * 4 + 3] == 0 && src[i] >= 0 {
            let s = src[i] as usize;
            rgba[i * 4] = snap[s * 4];
            rgba[i * 4 + 1] = snap[s * 4 + 1];
            rgba[i * 4 + 2] = snap[s * 4 + 2];
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let src_path = args.next().expect("source image");
    let ref_path = args.next().expect("ref bundle");

    let raw = std::fs::read(&src_path).unwrap();
    let img = image::load_from_memory(&raw).expect("decode").to_rgba8();
    let (sw, sh) = img.dimensions();

    let b = Bundle::load(std::path::Path::new(&ref_path)).unwrap();
    let mut ress: Vec<(String, Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data.clone()));
        }
    }
    let mut found = None;
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for obj in &sf.objects {
            if obj.class_id != 28 {
                continue;
            }
            let Ok(v) = sf.read_typetree(obj) else {
                continue;
            };
            if gi(&v, "m_TextureFormat") != 25 {
                continue;
            }
            let w = gi(&v, "m_Width") as u32;
            let h = gi(&v, "m_Height") as u32;
            let payload: Vec<u8> = match v.get("image data") {
                Some(Value::Bytes(bts)) if !bts.is_empty() => bts.clone(),
                _ => {
                    let sd = v.get("m_StreamData").unwrap();
                    let off = gi(sd, "offset") as usize;
                    let size = gi(sd, "size") as usize;
                    let path = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                    let base = path.rsplit('/').next().unwrap_or(path);
                    let d = &ress.iter().find(|(nm, _)| nm == base).unwrap().1;
                    d[off..off + size].to_vec()
                }
            };
            found = Some((w, h, payload));
        }
    }
    let (tw, th, rpay) = found.expect("no BC7 texture in ref");
    let (w, h) = (tw as usize, th as usize);

    let unbled: Vec<u8> = if (tw, th) != (sw, sh) {
        abgen::resize::box_downscale_rgba(img.as_raw(), sw as usize, sh as usize, w, h)
    } else {
        img.as_raw().clone()
    };

    let mut base = vec![0u8; w * h * 4];
    for y in 0..h {
        base[y * w * 4..(y + 1) * w * 4]
            .copy_from_slice(&unbled[(h - 1 - y) * w * 4..(h - y) * w * 4]);
    }

    let bw = w.div_ceil(4);
    let bh = h.div_ceil(4);
    let mut refpix = vec![0u32; w * h];
    texture2ddecoder::decode_bc7(&rpay[..bw * bh * 16], w, h, &mut refpix).unwrap();
    let mut refrgba = vec![0u8; w * h * 4];
    for (i, p) in refpix.iter().enumerate() {
        refrgba[i * 4] = ((p >> 16) & 0xFF) as u8;
        refrgba[i * 4 + 1] = ((p >> 8) & 0xFF) as u8;
        refrgba[i * 4 + 2] = (p & 0xFF) as u8;
        refrgba[i * 4 + 3] = ((p >> 24) & 0xFF) as u8;
    }

    let mut dist = vec![u32::MAX; w * h];
    let mut q = VecDeque::new();
    for i in 0..w * h {
        if base[i * 4 + 3] > 0 {
            dist[i] = 0;
            q.push_back(i);
        }
    }
    while let Some(i) = q.pop_front() {
        let (x, y) = (i % w, i / w);
        let d = dist[i] + 1;
        let mut push = |j: usize| {
            if dist[j] > d {
                dist[j] = d;
                q.push_back(j);
            }
        };
        if y > 0 {
            push(i - w);
        }
        if y + 1 < h {
            push(i + w);
        }
        if x > 0 {
            push(i - 1);
        }
        if x + 1 < w {
            push(i + 1);
        }
    }

    let score = |buf: &[u8]| -> (usize, usize, f64) {
        let mut m = 0usize;
        let mut t = 0usize;
        let mut s = 0u64;
        for i in 0..w * h {
            if base[i * 4 + 3] != 0 || dist[i] > PASSES {
                continue;
            }
            t += 1;
            let d = (0..3)
                .map(|c| (buf[i * 4 + c] as i32 - refrgba[i * 4 + c] as i32).unsigned_abs())
                .max()
                .unwrap();
            if d == 0 {
                m += 1;
            }
            s += d as u64;
        }
        (m, t, s as f64 / t.max(1) as f64)
    };

    let mut run = |name: &str, f: &dyn Fn(&mut Vec<u8>)| {
        let mut buf = base.clone();
        f(&mut buf);
        let (m, t, mean) = score(&buf);
        println!(
            "{name:24} exact {m}/{t} ({:.1}%) mean|d| {mean:.2}",
            100.0 * m as f64 / t as f64
        );
    };

    run("mean-jacobi (current)", &|b| {
        jacobi_fill(b, w, h, &[], true, PASSES)
    });
    let orders: &[(&str, [usize; 4])] = &[
        ("copy UDLR", [0, 1, 2, 3]),
        ("copy DULR", [1, 0, 2, 3]),
        ("copy LRUD", [2, 3, 0, 1]),
        ("copy RLDU", [3, 2, 1, 0]),
        ("copy LRDU", [2, 3, 1, 0]),
        ("copy UDRL", [0, 1, 3, 2]),
    ];
    for (nm, ord) in orders {
        run(nm, &|b| jacobi_fill(b, w, h, ord, false, PASSES));
    }
    run("voronoi L1 cap32", &|b| voronoi_fill(b, w, h, false, 32.0));
    run("voronoi L2 cap32", &|b| voronoi_fill(b, w, h, true, 32.0));

    let sep = |b: &mut Vec<u8>, horiz_first: bool, mean: bool, iters: u32| {
        let mut filled: Vec<u8> = (0..w * h).map(|i| u8::from(b[i * 4 + 3] > 0)).collect();
        for it in 0..iters {
            let horiz = (it % 2 == 0) == horiz_first;
            let snap = b.clone();
            let of = filled.clone();
            for y in 0..h {
                for x in 0..w {
                    let idx = y * w + x;
                    if of[idx] != 0 {
                        continue;
                    }
                    let (n1, n2) = if horiz {
                        (
                            if x > 0 { Some(idx - 1) } else { None },
                            if x + 1 < w { Some(idx + 1) } else { None },
                        )
                    } else {
                        (
                            if y > 0 { Some(idx - w) } else { None },
                            if y + 1 < h { Some(idx + w) } else { None },
                        )
                    };
                    let f1 = n1.filter(|&p| of[p] != 0);
                    let f2 = n2.filter(|&p| of[p] != 0);
                    let src = match (f1, f2) {
                        (Some(a), Some(bb)) => {
                            if mean {
                                for c in 0..3 {
                                    let s = snap[a * 4 + c] as u32 + snap[bb * 4 + c] as u32;
                                    b[idx * 4 + c] = ((s + 1) / 2) as u8;
                                }
                                filled[idx] = 1;
                                continue;
                            }
                            Some(a)
                        }
                        (Some(a), None) => Some(a),
                        (None, Some(bb)) => Some(bb),
                        (None, None) => None,
                    };
                    if let Some(s) = src {
                        for c in 0..3 {
                            b[idx * 4 + c] = snap[s * 4 + c];
                        }
                        filled[idx] = 1;
                    }
                }
            }
        }
    };
    run("sep H-first copy x64", &|b| sep(b, true, false, 64));
    run("sep V-first copy x64", &|b| sep(b, false, false, 64));
    run("sep H-first mean x64", &|b| sep(b, true, true, 64));
    run("sep V-first mean x64", &|b| sep(b, false, true, 64));

    let rb = |b: &mut Vec<u8>, mean: bool, order: [usize; 4], phase_first: usize, iters: u32| {
        let mut filled: Vec<u8> = (0..w * h).map(|i| u8::from(b[i * 4 + 3] > 0)).collect();
        for _ in 0..iters {
            for phase in [phase_first, 1 - phase_first] {
                let snap = b.clone();
                let of = filled.clone();
                for y in 0..h {
                    for x in 0..w {
                        if (x + y) % 2 != phase {
                            continue;
                        }
                        let idx = y * w + x;
                        if of[idx] != 0 {
                            continue;
                        }
                        let neigh = |k: usize| -> Option<usize> {
                            match k {
                                0 if y > 0 => Some(idx - w),
                                1 if y + 1 < h => Some(idx + w),
                                2 if x > 0 => Some(idx - 1),
                                3 if x + 1 < w => Some(idx + 1),
                                _ => None,
                            }
                        };
                        if mean {
                            let (mut s, mut cnt) = ([0u32; 3], 0u32);
                            for k in 0..4 {
                                if let Some(p) = neigh(k) {
                                    if of[p] != 0 {
                                        for c in 0..3 {
                                            s[c] += snap[p * 4 + c] as u32;
                                        }
                                        cnt += 1;
                                    }
                                }
                            }
                            if cnt > 0 {
                                for c in 0..3 {
                                    b[idx * 4 + c] = ((s[c] + cnt / 2) / cnt) as u8;
                                }
                                filled[idx] = 1;
                            }
                        } else {
                            for &k in &order {
                                if let Some(p) = neigh(k) {
                                    if of[p] != 0 {
                                        for c in 0..3 {
                                            b[idx * 4 + c] = snap[p * 4 + c];
                                        }
                                        filled[idx] = 1;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                for i in 0..w * h {
                    if filled[i] != 0 {
                        continue;
                    }
                }
                let _ = &snap;

                let _ = of;
            }
        }
    };
    run("rb mean even-first x32", &|b| {
        rb(b, true, [0, 1, 2, 3], 0, 32)
    });
    run("rb mean odd-first x32", &|b| {
        rb(b, true, [0, 1, 2, 3], 1, 32)
    });
    run("rb copy LRUD even x32", &|b| {
        rb(b, false, [2, 3, 0, 1], 0, 32)
    });
    run("rb copy LRUD odd x32", &|b| {
        rb(b, false, [2, 3, 0, 1], 1, 32)
    });
    run("8conn mean jacobi x32", &|b| {
        let mut filled: Vec<u8> = (0..w * h).map(|i| u8::from(b[i * 4 + 3] > 0)).collect();
        for _ in 0..32 {
            let snap = b.clone();
            let of = filled.clone();
            let mut any = false;
            for y in 0..h {
                for x in 0..w {
                    let idx = y * w + x;
                    if of[idx] != 0 {
                        continue;
                    }
                    let (mut s, mut cnt) = ([0u32; 3], 0u32);
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                                continue;
                            }
                            let p = (ny as usize) * w + nx as usize;
                            if of[p] != 0 {
                                for c in 0..3 {
                                    s[c] += snap[p * 4 + c] as u32;
                                }
                                cnt += 1;
                            }
                        }
                    }
                    if cnt > 0 {
                        for c in 0..3 {
                            b[idx * 4 + c] = ((s[c] + cnt / 2) / cnt) as u8;
                        }
                        filled[idx] = 1;
                        any = true;
                    }
                }
            }
            if !any {
                break;
            }
        }
    });
}
