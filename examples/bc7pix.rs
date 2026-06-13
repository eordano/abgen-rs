use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let src_path = args.next().expect("source image");
    let ref_path = args.next().expect("ref bundle");

    let raw = std::fs::read(&src_path).unwrap();
    let use_tj = std::env::var("BC7SWEEP_TURBOJPEG").is_ok();
    let img = if use_tj && raw.len() > 2 && raw[0] == 0xFF && raw[1] == 0xD8 {
        let (buf, w, h) = abgen::ffi::decode_jpeg_rgba(&raw).expect("tj decode");
        image::RgbaImage::from_raw(w, h, buf).unwrap()
    } else {
        image::load_from_memory(&raw).expect("decode").to_rgba8()
    };
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
            let cs = gi(&v, "m_ColorSpace");
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
            found = Some((w, h, cs, payload));
        }
    }
    let (tw, th, cs, rpay) = found.expect("no BC7 texture in ref");
    println!("src {sw}x{sh} -> target {tw}x{th} cs={cs}");

    let resized: Vec<u8> = if (tw, th) != (sw, sh) {
        abgen::resize::box_downscale_rgba(
            img.as_raw(),
            sw as usize,
            sh as usize,
            tw as usize,
            th as usize,
        )
    } else {
        img.as_raw().clone()
    };
    let has_alpha = resized.iter().skip(3).step_by(4).any(|&a| a < 255);
    let mut bled = resized.clone();
    if has_alpha && std::env::var("BC7PIX_NOBLEED").is_err() {
        abgen::alpha_bleed::alpha_bleed_inplace(&mut bled, tw, th);
    }

    let (w, h) = (tw as usize, th as usize);
    let bw = w.div_ceil(4);
    let bh = h.div_ceil(4);
    let mip0_bytes = bw * bh * 16;
    let mut refpix = vec![0u32; w * h];
    texture2ddecoder::decode_bc7(&rpay[..mip0_bytes], w, h, &mut refpix).expect("bc7 decode");

    let mut refrgba = vec![0u8; w * h * 4];
    for (i, p) in refpix.iter().enumerate() {
        refrgba[i * 4] = ((p >> 16) & 0xFF) as u8;
        refrgba[i * 4 + 1] = ((p >> 8) & 0xFF) as u8;
        refrgba[i * 4 + 2] = (p & 0xFF) as u8;
        refrgba[i * 4 + 3] = ((p >> 24) & 0xFF) as u8;
    }

    let mut ours = vec![0u8; w * h * 4];
    for y in 0..h {
        ours[y * w * 4..(y + 1) * w * 4]
            .copy_from_slice(&bled[(h - 1 - y) * w * 4..(h - y) * w * 4]);
    }

    let mut dist = vec![u32::MAX; w * h];
    let mut queue = std::collections::VecDeque::new();
    for i in 0..w * h {
        if ours[i * 4 + 3] == 255 {
            dist[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i % w, i / w);
        let d = dist[i] + 1;
        let mut push = |nx: usize, ny: usize| {
            let j = ny * w + nx;
            if dist[j] > d {
                dist[j] = d;
                queue.push_back(j);
            }
        };
        if x > 0 {
            push(x - 1, y);
        }
        if x + 1 < w {
            push(x + 1, y);
        }
        if y > 0 {
            push(x, y - 1);
        }
        if y + 1 < h {
            push(x, y + 1);
        }
    }

    {
        let mut by_d: std::collections::BTreeMap<u32, (usize, usize, u64)> = Default::default();
        for i in 0..w * h {
            if ours[i * 4 + 3] != 0 {
                continue;
            }
            let d = if dist[i] == u32::MAX {
                9999
            } else {
                dist[i].min(60)
            };
            let delta = (0..3)
                .map(|c| (ours[i * 4 + c] as i32 - refrgba[i * 4 + c] as i32).unsigned_abs())
                .max()
                .unwrap();
            let e = by_d.entry(d).or_default();
            e.1 += 1;
            e.2 += delta as u64;
            if delta == 0 {
                e.0 += 1;
            }
        }
        println!("transparent pixels by dist (exact-match/total, mean|d|):");
        for (d, (m, t, s)) in &by_d {
            println!(
                "  d={d}: {m}/{t} ({:.1}%) mean {:.2}",
                100.0 * *m as f64 / *t as f64,
                *s as f64 / *t as f64
            );
        }
    }

    if let Ok(spec) = std::env::var("BC7PIX_WINDOW") {
        let v: Vec<usize> = if let Some(dwant) = spec.strip_prefix("auto:") {
            let dwant: u32 = dwant.parse().unwrap();
            let mut found = vec![8, 8, 16, 16];
            for i in 0..w * h {
                if ours[i * 4 + 3] == 0 && dist[i] == dwant {
                    let delta = (0..3)
                        .map(|c| {
                            (ours[i * 4 + c] as i32 - refrgba[i * 4 + c] as i32).unsigned_abs()
                        })
                        .max()
                        .unwrap();
                    if delta > 8 {
                        let (x, y) = (i % w, i / w);
                        found = vec![x.saturating_sub(8), y.saturating_sub(8), 17, 17];
                        break;
                    }
                }
            }
            found
        } else {
            spec.split(',').map(|s| s.parse().unwrap()).collect()
        };
        let (wx, wy, ww, wh) = (v[0], v[1], v[2], v[3]);
        for y in wy..(wy + wh).min(h) {
            let mut line_o = String::new();
            let mut line_r = String::new();
            for x in wx..(wx + ww).min(w) {
                let i = y * w + x;
                line_o += &format!(
                    "{:3},{:3},{:3},{:3}|",
                    ours[i * 4],
                    ours[i * 4 + 1],
                    ours[i * 4 + 2],
                    ours[i * 4 + 3]
                );
                line_r += &format!(
                    "{:3},{:3},{:3},{:3}|",
                    refrgba[i * 4],
                    refrgba[i * 4 + 1],
                    refrgba[i * 4 + 2],
                    refrgba[i * 4 + 3]
                );
            }
            println!("y={y:4} ours {line_o}");
            println!("       ref  {line_r}");
        }
    }

    let mut hist: [std::collections::BTreeMap<u32, usize>; 3] = Default::default();
    let mut count = [0usize; 3];
    let mut sum_abs = [0u64; 3];
    for i in 0..w * h {
        let a = ours[i * 4 + 3];
        let class = if a == 255 {
            0
        } else if a > 0 {
            1
        } else {
            2
        };
        let d = (0..3)
            .map(|c| (ours[i * 4 + c] as i32 - refrgba[i * 4 + c] as i32).unsigned_abs())
            .max()
            .unwrap();
        count[class] += 1;
        sum_abs[class] += d as u64;
        *hist[class].entry(d.min(16)).or_default() += 1;
    }
    let names = ["opaque", "partial", "transparent"];
    for c in 0..3 {
        if count[c] == 0 {
            continue;
        }
        let h16: Vec<String> = hist[c].iter().map(|(d, n)| format!("{d}:{n}")).collect();
        println!(
            "{}: n={} mean|d|={:.2}  hist {}",
            names[c],
            count[c],
            sum_abs[c] as f64 / count[c] as f64,
            h16.join(" ")
        );
    }

    let mut amax = 0u32;
    let mut adiff = 0usize;
    for i in 0..w * h {
        let d = (ours[i * 4 + 3] as i32 - refrgba[i * 4 + 3] as i32).unsigned_abs();
        if d > 0 {
            adiff += 1;
        }
        amax = amax.max(d);
    }
    println!("alpha: pixels differing {adiff}/{} max {amax}", w * h);

    let (enc, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
        &bled,
        tw,
        th,
        Some(1),
        true,
        cs == 1,
        cs == 1,
        abgen::bc7_pure::Bc7Profile::Basic,
    );
    let mut by_dist: std::collections::BTreeMap<u32, (usize, usize)> = Default::default();
    for by in 0..bh {
        for bx in 0..bw {
            let bi = by * bw + bx;
            let same = enc[bi * 16..bi * 16 + 16] == rpay[bi * 16..bi * 16 + 16];

            let mut mind = u32::MAX;
            for py in by * 4..((by + 1) * 4).min(h) {
                for px in bx * 4..((bx + 1) * 4).min(w) {
                    mind = mind.min(dist[py * w + px]);
                }
            }
            let bucket = if mind == u32::MAX { 9999 } else { mind.min(20) };
            let e = by_dist.entry(bucket).or_default();
            if !same {
                e.0 += 1;
            }
            e.1 += 1;
        }
    }
    println!("mip0 differing blocks by min-dist-from-opaque (dist: diff/total):");
    for (d, (df, tot)) in &by_dist {
        println!(
            "  d={d}: {df}/{tot} ({:.1}%)",
            100.0 * *df as f64 / *tot as f64
        );
    }
}
