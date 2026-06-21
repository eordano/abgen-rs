use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn po2_fill(rgba: &mut [u8], w: usize, h: usize, offsets: &[usize], prio: &[usize; 4], mean: bool) {
    let mut filled: Vec<u8> = (0..w * h).map(|i| u8::from(rgba[i * 4 + 3] > 0)).collect();
    for &k in offsets {
        let snap = rgba.to_vec();
        let of = filled.clone();
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if of[idx] != 0 {
                    continue;
                }
                let tap = |t: usize| -> Option<usize> {
                    match t {
                        0 if x >= k => Some(idx - k),
                        1 if x + k < w => Some(idx + k),
                        2 if y >= k => Some(idx - k * w),
                        3 if y + k < h => Some(idx + k * w),
                        _ => None,
                    }
                };
                if mean {
                    let (mut s, mut cnt) = ([0u32; 3], 0u32);
                    for t in 0..4 {
                        if let Some(p) = tap(t) {
                            if of[p] != 0 {
                                for c in 0..3 {
                                    s[c] += snap[p * 4 + c] as u32;
                                }
                                cnt += 1;
                            }
                        }
                    }
                    // guard also covers filled[idx]; integer rounding division is intended.
                    #[allow(clippy::manual_checked_ops)]
                    if cnt > 0 {
                        for c in 0..3 {
                            rgba[idx * 4 + c] = ((s[c] + cnt / 2) / cnt) as u8;
                        }
                        filled[idx] = 1;
                    }
                } else {
                    for &t in prio {
                        if let Some(p) = tap(t) {
                            if of[p] != 0 {
                                for c in 0..3 {
                                    rgba[idx * 4 + c] = snap[p * 4 + c];
                                }
                                filled[idx] = 1;
                                break;
                            }
                        }
                    }
                }
            }
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
                    let p = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                    let base = p.rsplit('/').next().unwrap_or(p);
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

    let pixscore = |buf: &[u8]| -> (usize, usize) {
        let (mut m, mut t) = (0usize, 0usize);
        for i in 0..w * h {
            if base[i * 4 + 3] != 0 {
                continue;
            }
            t += 1;
            let p = refpix[i];
            let rgb = [
                ((p >> 16) & 0xFF) as u8,
                ((p >> 8) & 0xFF) as u8,
                (p & 0xFF) as u8,
            ];
            if buf[i * 4] == rgb[0] && buf[i * 4 + 1] == rgb[1] && buf[i * 4 + 2] == rgb[2] {
                m += 1;
            }
        }
        (m, t)
    };

    let blkscore = |buf: &[u8]| -> (usize, usize) {
        let mut flipped = vec![0u8; w * h * 4];
        for y in 0..h {
            flipped[y * w * 4..(y + 1) * w * 4]
                .copy_from_slice(&buf[(h - 1 - y) * w * 4..(h - y) * w * 4]);
        }
        let (enc, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
            &flipped,
            tw,
            th,
            Some(1),
            true,
            true,
            true,
            abgen::bc7_pure::Bc7Profile::Basic,
        );
        let nb = bw * bh;
        let mut m = 0;
        for i in 0..nb {
            if enc[i * 16..i * 16 + 16] == rpay[i * 16..i * 16 + 16] {
                m += 1;
            }
        }
        (m, nb)
    };
    let score = |buf: &[u8]| -> (usize, usize, usize, usize) {
        let (pm, pt) = pixscore(buf);
        let (bm, bt) = blkscore(buf);
        (pm, pt, bm, bt)
    };

    let asc: Vec<usize> = vec![1, 2, 4, 8, 16];
    let desc: Vec<usize> = vec![16, 8, 4, 2, 1];
    let prios: &[(&str, [usize; 4])] = &[
        ("L R U D", [0, 1, 2, 3]),
        ("R L U D", [1, 0, 2, 3]),
        ("U D L R", [2, 3, 0, 1]),
        ("D U L R", [3, 2, 0, 1]),
        ("L R D U", [0, 1, 3, 2]),
        ("U L R D", [2, 0, 1, 3]),
        ("D L R U", [3, 0, 1, 2]),
        ("R L D U", [1, 0, 3, 2]),
    ];

    let jfa_fill = |rgba: &mut Vec<u8>, offsets: &[usize], metric: u8, improve: bool| {
        let n = w * h;
        let mut seed: Vec<i64> = vec![-1; n];
        for i in 0..n {
            if rgba[i * 4 + 3] > 0 {
                seed[i] = i as i64;
            }
        }
        let d_of = |i: usize, sd: i64| -> i64 {
            let (x, y) = ((i % w) as i64, (i / w) as i64);
            let (sx, sy) = ((sd % w as i64), (sd / w as i64));
            let (dx, dy) = (x - sx, y - sy);
            if metric == 0 {
                dx * dx + dy * dy
            } else {
                dx.abs() + dy.abs()
            }
        };
        for &k in offsets {
            let snap_seed = seed.clone();
            for y in 0..h {
                for x in 0..w {
                    let idx = y * w + x;
                    if rgba[idx * 4 + 3] > 0 {
                        continue;
                    }
                    if !improve && snap_seed[idx] >= 0 {
                        continue;
                    }
                    let mut best = if improve { seed[idx] } else { -1 };
                    let mut bestd = if best >= 0 { d_of(idx, best) } else { i64::MAX };
                    let taps = [
                        (x >= k).then(|| idx - k),
                        (x + k < w).then(|| idx + k),
                        (y >= k).then(|| idx - k * w),
                        (y + k < h).then(|| idx + k * w),
                    ];
                    for p in taps.into_iter().flatten() {
                        let sd = snap_seed[p];
                        if sd >= 0 {
                            let d = d_of(idx, sd);
                            if d < bestd {
                                bestd = d;
                                best = sd;
                            }
                        }
                    }
                    if best >= 0 {
                        seed[idx] = best;
                    }
                }
            }
        }
        let snap = rgba.clone();
        for i in 0..n {
            if rgba[i * 4 + 3] == 0 && seed[i] >= 0 {
                let s = seed[i] as usize;
                rgba[i * 4] = snap[s * 4];
                rgba[i * 4 + 1] = snap[s * 4 + 1];
                rgba[i * 4 + 2] = snap[s * 4 + 2];
            }
        }
    };
    if std::env::var("BC7PO2_MIPS").is_ok() {
        use abgen::bc7_pure as bp;
        let mut buf = base.clone();
        jfa_fill(&mut buf, &[16, 8, 4, 2, 1], 1, true);
        let bled0 = buf.clone();
        let mips = {
            let mut m = 1;
            while bp::compute_mip_chain_size(tw, th, m) < rpay.len() {
                m += 1;
            }
            m
        };
        let to_f = |lvl: &[u8]| -> Vec<f32> {
            let mut out = vec![0f32; lvl.len()];
            for i in 0..lvl.len() / 4 {
                out[i * 4] = bp::srgb_to_linear_u8(lvl[i * 4]);
                out[i * 4 + 1] = bp::srgb_to_linear_u8(lvl[i * 4 + 1]);
                out[i * 4 + 2] = bp::srgb_to_linear_u8(lvl[i * 4 + 2]);
                out[i * 4 + 3] = lvl[i * 4 + 3] as f32;
            }
            out
        };
        let to_u8 = |fl: &[f32]| -> Vec<u8> {
            let mut out = vec![0u8; fl.len()];
            for i in 0..fl.len() / 4 {
                out[i * 4] = bp::linear_to_srgb_u8(fl[i * 4]);
                out[i * 4 + 1] = bp::linear_to_srgb_u8(fl[i * 4 + 1]);
                out[i * 4 + 2] = bp::linear_to_srgb_u8(fl[i * 4 + 2]);
                out[i * 4 + 3] = bp::round_half_up_u8(fl[i * 4 + 3]);
            }
            out
        };
        let halve_w = |fl: &[f32], cw: usize, ch: usize| -> (Vec<f32>, usize, usize) {
            let nh = (ch / 2).max(1);
            let nw = (cw / 2).max(1);
            let fh = if ch > 1 { 2 } else { 1 };
            let fw = if cw > 1 { 2 } else { 1 };
            let mut out = vec![0f32; nh * nw * 4];
            for ny in 0..nh {
                for nx in 0..nw {
                    let mut acc = [0f32; 4];
                    let mut wsum = 0f32;
                    for dy in 0..fh {
                        for dx in 0..fw {
                            let i = ((ny * fh + dy) * cw + nx * fw + dx) * 4;
                            let a = fl[i + 3];
                            acc[0] += fl[i] * a;
                            acc[1] += fl[i + 1] * a;
                            acc[2] += fl[i + 2] * a;
                            acc[3] += a;
                            wsum += a;
                        }
                    }
                    let o = (ny * nw + nx) * 4;
                    if wsum > 0.0 {
                        out[o] = acc[0] / wsum;
                        out[o + 1] = acc[1] / wsum;
                        out[o + 2] = acc[2] / wsum;
                    }
                    out[o + 3] = acc[3] / (fh * fw) as f32;
                }
            }
            (out, nw, nh)
        };

        let bleed_lvl = |lvl: &mut Vec<u8>, cw: usize, ch: usize| {
            abgen::alpha_bleed::alpha_bleed_inplace(lvl, cw as u32, ch as u32);
        };
        let params = bp::Params::basic(true);
        let encode_lvl = |lvl: &[u8], cw: usize, ch: usize| -> Vec<u8> {
            let bx = cw.div_ceil(4).max(1);
            let by = ch.div_ceil(4).max(1);
            let mut blocks = vec![0u8; bx * by * 64];
            for byi in 0..by {
                for bxi in 0..bx {
                    for py in 0..4 {
                        for px in 0..4 {
                            let sx = (bxi * 4 + px).min(cw - 1);
                            let sy = (byi * 4 + py).min(ch - 1);
                            let src = (sy * cw + sx) * 4;
                            let dst = ((byi * bx + bxi) * 16 + py * 4 + px) * 4;
                            blocks[dst..dst + 4].copy_from_slice(&lvl[src..src + 4]);
                        }
                    }
                }
            }
            bp::encode_blocks(&blocks, bx * by, &params)
        };
        for variant in 0..5u32 {
            let mut enc_all: Vec<u8> = Vec::new();
            let mut cur_f = to_f(&bled0);
            let (mut cw, mut ch) = (w, h);
            let mut cur_u8 = bled0.clone();
            for m in 0..mips {
                let mut lvl = if m == 0 { bled0.clone() } else { to_u8(&cur_f) };
                if variant == 3 && m > 0 {
                    lvl = cur_u8.clone();
                }
                if (variant == 1 || variant == 2) && m > 0 {
                    bleed_lvl(&mut lvl, cw, ch);
                }
                enc_all.extend_from_slice(&encode_lvl(&lvl, cw, ch));
                if variant == 2 {
                    cur_f = to_f(&lvl);
                }
                if m < mips - 1 {
                    if variant == 3 {
                        let f = to_f(&lvl);
                        let (nf, nw2, nh2) = bp::box_halve(&f, cw, ch);
                        cur_u8 = to_u8(&nf);
                        cw = nw2;
                        ch = nh2;
                    } else if variant == 4 {
                        let (nf, nw2, nh2) = halve_w(&cur_f, cw, ch);
                        cur_f = nf;
                        cw = nw2;
                        ch = nh2;
                    } else {
                        let (nf, nw2, nh2) = bp::box_halve(&cur_f, cw, ch);
                        cur_f = nf;
                        cw = nw2;
                        ch = nh2;
                    }
                }
            }
            if enc_all.len() != rpay.len() {
                println!(
                    "V{variant}: len mismatch {} vs {}",
                    enc_all.len(),
                    rpay.len()
                );
                continue;
            }
            let nb = enc_all.len() / 16;
            let mut m = 0;
            for i in 0..nb {
                if enc_all[i * 16..i * 16 + 16] == rpay[i * 16..i * 16 + 16] {
                    m += 1;
                }
            }
            println!("V{variant}: {m}/{nb} bytes_equal={}", enc_all == rpay);
        }
        return;
    }
    if std::env::var("BC7PO2_FULL").is_ok() {
        let mut buf = base.clone();
        jfa_fill(&mut buf, &[16, 8, 4, 2, 1], 1, true);
        let mut flipped = vec![0u8; w * h * 4];
        for y in 0..h {
            flipped[y * w * 4..(y + 1) * w * 4]
                .copy_from_slice(&buf[(h - 1 - y) * w * 4..(h - y) * w * 4]);
        }
        let mips = if rpay.len() != bw * bh * 16 { {
                let mut m = 1;
                while abgen::bc7_pure::compute_mip_chain_size(tw, th, m) < rpay.len() {
                    m += 1;
                }
                m
            } } else { 1 };
        let (enc, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
            &flipped,
            tw,
            th,
            Some(mips),
            true,
            true,
            true,
            abgen::bc7_pure::Bc7Profile::Basic,
        );
        if enc.len() != rpay.len() {
            println!("FULL: len mismatch {} vs {}", enc.len(), rpay.len());
            return;
        }
        let nb = enc.len() / 16;
        let mut m0 = 0;
        let mut m = 0;
        for i in 0..nb {
            if enc[i * 16..i * 16 + 16] == rpay[i * 16..i * 16 + 16] {
                m += 1;
                if i < bw * bh {
                    m0 += 1;
                }
            }
        }
        println!(
            "FULL: mip0 {m0}/{} all {m}/{nb} ({:.2}%) bytes_equal={}",
            bw * bh,
            100.0 * m as f64 / nb as f64,
            enc == rpay
        );

        let mut off = 0usize;
        let (mut mw, mut mh) = (tw as usize, th as usize);
        for mip in 0..mips {
            let nbm = mw.div_ceil(4) * mh.div_ceil(4);
            let mut dm = 0;
            for i in 0..nbm {
                let a = &enc[(off + i) * 16..(off + i) * 16 + 16];
                let bcheck = &rpay[(off + i) * 16..(off + i) * 16 + 16];
                if a != bcheck {
                    dm += 1;
                }
            }
            if dm > 0 {
                let pw = mw.div_ceil(4) * 4;
                let ph = mh.div_ceil(4) * 4;
                let mut op = vec![0u32; pw * ph];
                let mut rp2 = op.clone();
                let _ =
                    texture2ddecoder::decode_bc7(&enc[off * 16..(off + nbm) * 16], pw, ph, &mut op);
                let _ = texture2ddecoder::decode_bc7(
                    &rpay[off * 16..(off + nbm) * 16],
                    pw,
                    ph,
                    &mut rp2,
                );
                let mut pixdiff = 0;
                let mut maxd = 0i32;
                for i in 0..pw * ph {
                    let a = op[i];
                    let bb = rp2[i];
                    if a != bb {
                        pixdiff += 1;
                        for sh in [0, 8, 16, 24] {
                            maxd = maxd
                                .max((((a >> sh) & 255) as i32 - ((bb >> sh) & 255) as i32).abs());
                        }
                    }
                }
                println!("  mip{mip} {mw}x{mh}: {dm}/{nbm} blk diff, {pixdiff} px diff (max chan delta {maxd})");
                if mw <= 16 {
                    let pw = mw.div_ceil(4) * 4;
                    let ph = mh.div_ceil(4) * 4;
                    let mut op = vec![0u32; pw * ph];
                    let mut rp = op.clone();
                    let _ = texture2ddecoder::decode_bc7(
                        &enc[off * 16..(off + nbm) * 16],
                        pw,
                        ph,
                        &mut op,
                    );
                    let _ = texture2ddecoder::decode_bc7(
                        &rpay[off * 16..(off + nbm) * 16],
                        pw,
                        ph,
                        &mut rp,
                    );
                    for y in 0..mh.min(8) {
                        let mut lo = String::new();
                        let mut lr = String::new();
                        for x in 0..mw.min(8) {
                            let a = op[y * pw + x];
                            let bb = rp[y * pw + x];
                            lo += &format!(
                                "{:3},{:3},{:3},{:3}|",
                                (a >> 16) & 255,
                                (a >> 8) & 255,
                                a & 255,
                                (a >> 24) & 255
                            );
                            lr += &format!(
                                "{:3},{:3},{:3},{:3}|",
                                (bb >> 16) & 255,
                                (bb >> 8) & 255,
                                bb & 255,
                                (bb >> 24) & 255
                            );
                        }
                        println!("    y{y} ours {lo}");
                        println!("       ref  {lr}");
                    }
                }
            }
            off += nbm;
            mw = (mw / 2).max(1);
            mh = (mh / 2).max(1);
        }
        return;
    }
    if std::env::var("BC7PO2_JFA").is_ok() {
        for (oname, offs) in [
            ("asc", vec![1usize, 2, 4, 8, 16]),
            ("desc", vec![16usize, 8, 4, 2, 1]),
        ] {
            for metric in [0u8, 1] {
                for improve in [false, true] {
                    let mut buf = base.clone();
                    jfa_fill(&mut buf, &offs, metric, improve);
                    let (m, t, bm, bt) = score(&buf);
                    println!(
                        "jfa {oname} {} improve={improve}: pix {m}/{t} ({:.2}%)  blk {bm}/{bt} ({:.2}%)",
                        if metric == 0 { "L2" } else { "L1" },
                        100.0 * m as f64 / t as f64,
                        100.0 * bm as f64 / bt as f64
                    );
                }
            }
        }
        return;
    }
    if std::env::var("BC7PO2_TIES").is_ok() {
        let offsets = [1usize, 2, 4, 8, 16];
        let mut buf = base.clone();
        let mut filled: Vec<u8> = (0..w * h).map(|i| u8::from(buf[i * 4 + 3] > 0)).collect();
        let mut tally: std::collections::BTreeMap<String, usize> = Default::default();
        for &k in &offsets {
            let snap = buf.clone();
            let of = filled.clone();
            for y in 0..h {
                for x in 0..w {
                    let idx = y * w + x;
                    if of[idx] != 0 {
                        continue;
                    }
                    let mut cands: Vec<(usize, [u8; 3])> = Vec::new();
                    let taps = [
                        (0usize, (x >= k).then(|| idx - k)),
                        (1, (x + k < w).then(|| idx + k)),
                        (2, (y >= k).then(|| idx - k * w)),
                        (3, (y + k < h).then(|| idx + k * w)),
                    ];
                    for (t, opt) in taps {
                        if let Some(p) = opt {
                            if of[p] != 0 {
                                cands.push((t, [snap[p * 4], snap[p * 4 + 1], snap[p * 4 + 2]]));
                            }
                        }
                    }
                    if cands.is_empty() {
                        continue;
                    }

                    let distinct: std::collections::BTreeSet<[u8; 3]> =
                        cands.iter().map(|c| c.1).collect();
                    if distinct.len() >= 2 {
                        let p = refpix[idx];
                        let rgb = [
                            ((p >> 16) & 0xFF) as u8,
                            ((p >> 8) & 0xFF) as u8,
                            (p & 0xFF) as u8,
                        ];
                        let mut who = String::new();
                        for (t, v) in &cands {
                            if *v == rgb {
                                who += match t {
                                    0 => "L",
                                    1 => "R",
                                    2 => "U",
                                    _ => "D",
                                };
                            }
                        }

                        let n = cands.len() as u32;
                        let mut mean = [0u8; 3];
                        for (c, m) in mean.iter_mut().enumerate() {
                            let s: u32 = cands.iter().map(|cc| cc.1[c] as u32).sum();
                            *m = ((s + n / 2) / n) as u8;
                        }
                        let key = if !who.is_empty() {
                            format!("k={k} taps={} exact={}", cands.len(), who)
                        } else if mean == rgb {
                            format!("k={k} taps={} MEAN", cands.len())
                        } else {
                            format!("k={k} taps={} noisy", cands.len())
                        };
                        *tally.entry(key).or_default() += 1;
                    }

                    let v = cands[0].1;
                    buf[idx * 4] = v[0];
                    buf[idx * 4 + 1] = v[1];
                    buf[idx * 4 + 2] = v[2];
                    filled[idx] = 1;
                }
            }
        }
        for (k, v) in &tally {
            println!("{k}: {v}");
        }
        return;
    }
    if let Ok(spec) = std::env::var("BC7PO2_AB") {
        let parse = |sp: &str| -> (Vec<usize>, [usize; 4]) {
            let parts: Vec<&str> = sp.split(',').collect();
            let offs = if parts[0] == "asc" {
                vec![1, 2, 4, 8, 16]
            } else {
                vec![16, 8, 4, 2, 1]
            };
            let b: Vec<usize> = parts[1].bytes().map(|c| (c - b'0') as usize).collect();
            (offs, [b[0], b[1], b[2], b[3]])
        };
        let two: Vec<&str> = spec.split(';').collect();
        let (o1, p1) = parse(two[0]);
        let (o2, p2) = parse(two[1]);
        let mut b1 = base.clone();
        po2_fill(&mut b1, w, h, &o1, &p1, false);
        let mut b2 = base.clone();
        po2_fill(&mut b2, w, h, &o2, &p2, false);
        let (mut n_dis, mut win1, mut win2, mut neither) = (0usize, 0usize, 0usize, 0usize);
        for i in 0..w * h {
            if base[i * 4 + 3] != 0 {
                continue;
            }
            let v1 = [b1[i * 4], b1[i * 4 + 1], b1[i * 4 + 2]];
            let v2 = [b2[i * 4], b2[i * 4 + 1], b2[i * 4 + 2]];
            if v1 == v2 {
                continue;
            }
            n_dis += 1;
            let p = refpix[i];
            let rgb = [
                ((p >> 16) & 0xFF) as u8,
                ((p >> 8) & 0xFF) as u8,
                (p & 0xFF) as u8,
            ];
            let far =
                |a: [u8; 3], bb: [u8; 3]| (0..3).any(|c| (a[c] as i32 - bb[c] as i32).abs() >= 8);
            if rgb == v1 && far(rgb, v2) {
                win1 += 1;
            } else if rgb == v2 && far(rgb, v1) {
                win2 += 1;
            } else {
                neither += 1;
            }
        }
        println!(
            "disagree {n_dis}: {} wins {win1}, {} wins {win2}, neither {neither}",
            two[0], two[1]
        );
        return;
    }
    if let Ok(spec) = std::env::var("BC7PO2_DUMP") {
        let parts: Vec<&str> = spec.split(',').collect();
        let offs: &Vec<usize> = if parts[0] == "asc" { &asc } else { &desc };
        let prio: [usize; 4] = {
            let b: Vec<usize> = parts[1].bytes().map(|c| (c - b'0') as usize).collect();
            [b[0], b[1], b[2], b[3]]
        };
        let mut buf = base.clone();
        po2_fill(&mut buf, w, h, offs, &prio, false);

        let mut shown = 0;
        let mut covered = vec![false; w * h];
        for i in 0..w * h {
            if base[i * 4 + 3] != 0 || covered[i] {
                continue;
            }
            let p = refpix[i];
            let rgb = [
                ((p >> 16) & 0xFF) as u8,
                ((p >> 8) & 0xFF) as u8,
                (p & 0xFF) as u8,
            ];
            if buf[i * 4] == rgb[0] && buf[i * 4 + 1] == rgb[1] && buf[i * 4 + 2] == rgb[2] {
                continue;
            }
            let (x, y) = (i % w, i / w);
            println!(
                "--- mismatch at tex ({x},{y}) sim {},{},{} ref {},{},{}",
                buf[i * 4],
                buf[i * 4 + 1],
                buf[i * 4 + 2],
                rgb[0],
                rgb[1],
                rgb[2]
            );
            for yy in y.saturating_sub(4)..(y + 5).min(h) {
                let mut ls = String::new();
                let mut lr = String::new();
                for xx in x.saturating_sub(6)..(x + 7).min(w) {
                    let j = yy * w + xx;
                    covered[j] = true;
                    let pp = refpix[j];
                    ls += &format!(
                        "{:3},{:3},{:3}|",
                        buf[j * 4],
                        buf[j * 4 + 1],
                        buf[j * 4 + 2]
                    );
                    lr += &format!(
                        "{:3},{:3},{:3}|",
                        (pp >> 16) & 0xFF,
                        (pp >> 8) & 0xFF,
                        pp & 0xFF
                    );
                }
                println!("y={yy:3} sim {ls}");
                println!("      ref {lr}");
            }
            shown += 1;
            if shown >= 6 {
                break;
            }
        }
        return;
    }
    for (oname, offs) in [("asc", &asc), ("desc", &desc)] {
        for (pname, prio) in prios {
            let mut buf = base.clone();
            po2_fill(&mut buf, w, h, offs, prio, false);
            let (m, t, bm, bt) = score(&buf);
            println!(
                "{oname:4} copy {pname}: pix {m}/{t} ({:.2}%)  blk {bm}/{bt} ({:.2}%)",
                100.0 * m as f64 / t as f64,
                100.0 * bm as f64 / bt as f64
            );
        }
        let mut buf = base.clone();
        po2_fill(&mut buf, w, h, offs, &[0, 1, 2, 3], true);
        let (m, t, bm, bt) = score(&buf);
        println!(
            "{oname:4} mean       : pix {m}/{t} ({:.2}%)  blk {bm}/{bt} ({:.2}%)",
            100.0 * m as f64 / t as f64,
            100.0 * bm as f64 / bt as f64
        );
    }
}
