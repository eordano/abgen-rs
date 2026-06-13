use abgen::bc7_pure::{box_halve, linear_to_srgb_u8, srgb_to_linear_u8};
use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

struct Tex {
    name: String,
    w: usize,
    h: usize,
    mips: i64,
    payload: Vec<u8>,
}

fn extract(bundle: &Bundle) -> BTreeMap<i64, Tex> {
    let mut out = BTreeMap::new();
    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &bundle.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data));
        }
    }
    for f in &bundle.files {
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
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("tex")
                .to_string();
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let mips = gi(&v, "m_MipCount");
            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(b) if !b.is_empty() => Some(b.as_slice()),
                _ => None,
            });
            let payload: Vec<u8> = if let Some(d) = inline {
                d.to_vec()
            } else if let Some(sd) = v.get("m_StreamData") {
                let off = gi(sd, "offset") as usize;
                let size = gi(sd, "size") as usize;
                let path = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                let base = path.rsplit('/').next().unwrap_or(path);
                let Some((_, data)) = ress.iter().find(|(nm, _)| nm == base) else {
                    continue;
                };
                if off + size > data.len() {
                    continue;
                }
                data[off..off + size].to_vec()
            } else {
                continue;
            };
            out.insert(
                obj.path_id,
                Tex {
                    name,
                    w,
                    h,
                    mips,
                    payload,
                },
            );
        }
    }
    out
}

fn mip_byte_off(w: usize, h: usize, level: usize) -> (usize, usize, usize, usize) {
    let (mut mw, mut mh) = (w, h);
    let mut off = 0usize;
    for _ in 0..level {
        let pw = mw.div_ceil(4).max(1);
        let ph = mh.div_ceil(4).max(1);
        off += pw * ph * 16;
        mw = (mw / 2).max(1);
        mh = (mh / 2).max(1);
    }
    let pw = mw.div_ceil(4).max(1);
    let ph = mh.div_ceil(4).max(1);
    (off, pw * ph * 16, mw, mh)
}

fn decode_mip(blocks: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    let pw = (w + 3) & !3;
    let ph = (h + 3) & !3;
    let mut px = vec![0u32; pw * ph];
    texture2ddecoder::decode_bc7(blocks, pw, ph, &mut px).ok()?;
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let [b, g, r, a] = px[y * pw + x].to_le_bytes();
            let d = (y * w + x) * 4;
            rgba[d] = r;
            rgba[d + 1] = g;
            rgba[d + 2] = b;
            rgba[d + 3] = a;
        }
    }
    Some(rgba)
}

fn srgb_to_lin_pow(c: u8) -> f32 {
    let s = c as f32 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}
fn lin_to_srgb_pow_round(l: f32) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let s = if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8
}

#[inline(never)]
fn box_halve_generic(arr: &[f64], w: usize, h: usize) -> (Vec<f64>, usize, usize) {
    let nh = (h / 2).max(1);
    let nw = (w / 2).max(1);
    let fh = if h > 1 { 2 } else { 1 };
    let fw = if w > 1 { 2 } else { 1 };
    let denom = (fh * fw) as f64;
    let mut out = vec![0f64; nh * nw * 4];
    let rs = w * 4;
    let mut ny = 0;
    while ny < nh {
        let mut nx = 0;
        while nx < nw {
            let mut c = 0;
            while c < 4 {
                let mut acc = 0f64;
                let mut dy = 0;
                while dy < fh {
                    let mut dx = 0;
                    while dx < fw {
                        acc += core::hint::black_box(
                            arr[(ny * fh + dy) * rs + (nx * fw + dx) * 4 + c],
                        );
                        dx += 1;
                    }
                    dy += 1;
                }
                out[(ny * nw + nx) * 4 + c] = acc / denom;
                c += 1;
            }
            nx += 1;
        }
        ny += 1;
    }
    (out, nw, nh)
}

fn box_halve_u8(arr: &[u8], w: usize, h: usize, round: bool) -> (Vec<u8>, usize, usize) {
    let nh = (h / 2).max(1);
    let nw = (w / 2).max(1);
    let fh = if h > 1 { 2 } else { 1 };
    let fw = if w > 1 { 2 } else { 1 };
    let denom = (fh * fw) as u32;
    let mut out = vec![0u8; nh * nw * 4];
    let rs = w * 4;
    for ny in 0..nh {
        for nx in 0..nw {
            for c in 0..4 {
                let mut acc = 0u32;
                for dy in 0..fh {
                    for dx in 0..fw {
                        acc += arr[(ny * fh + dy) * rs + (nx * fw + dx) * 4 + c] as u32;
                    }
                }
                let v = if round {
                    (acc + denom / 2) / denom
                } else {
                    acc / denom
                };
                out[(ny * nw + nx) * 4 + c] = v as u8;
            }
        }
    }
    (out, nw, nh)
}

fn cmp(cand: &[u8], reff: &[u8]) -> (usize, usize, [u32; 4]) {
    let n = cand.len() / 4;
    let mut exact = 0;
    let mut maxd = [0u32; 4];
    for i in 0..n {
        let mut all = true;
        for c in 0..4 {
            let d = (cand[i * 4 + c] as i32 - reff[i * 4 + c] as i32).unsigned_abs();
            if d > maxd[c] {
                maxd[c] = d;
            }
            if d != 0 {
                all = false;
            }
        }
        if all {
            exact += 1;
        }
    }
    (exact, n, maxd)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ours = Bundle::load(std::path::Path::new(&args[0])).unwrap();
    let refb = Bundle::load(std::path::Path::new(&args[1])).unwrap();
    let target_pid: Option<i64> = args.get(2).and_then(|s| s.parse().ok());
    let op = extract(&ours);
    let rp = extract(&refb);

    for (pid, ot) in &op {
        if let Some(t) = target_pid {
            if *pid != t {
                continue;
            }
        }
        let Some(rt) = rp.get(pid) else { continue };
        if ot.w != rt.w
            || ot.h != rt.h
            || ot.mips != rt.mips
            || ot.payload.len() != rt.payload.len()
        {
            continue;
        }

        let (o0, l0, _, _) = mip_byte_off(ot.w, ot.h, 0);
        let Some(src) = decode_mip(&ot.payload[o0..o0 + l0], ot.w, ot.h) else {
            continue;
        };

        let has_alpha = src.iter().skip(3).step_by(4).any(|&a| a != 255);
        println!(
            "pid={pid} {} {}x{} mips={} alpha={}",
            ot.name, ot.w, ot.h, ot.mips, has_alpha
        );

        let n0 = ot.w * ot.h;
        let mut lin_f32_lut = vec![0f32; n0 * 4];
        let mut lin_f32_pow = vec![0f32; n0 * 4];
        let mut lin_f64_pow = vec![0f64; n0 * 4];
        for i in 0..n0 {
            for c in 0..3 {
                lin_f32_lut[i * 4 + c] = srgb_to_linear_u8(src[i * 4 + c]);
                let lp = srgb_to_lin_pow(src[i * 4 + c]);
                lin_f32_pow[i * 4 + c] = lp;
                lin_f64_pow[i * 4 + c] = lp as f64;
            }
            lin_f32_lut[i * 4 + 3] = src[i * 4 + 3] as f32;
            lin_f32_pow[i * 4 + 3] = src[i * 4 + 3] as f32;
            lin_f64_pow[i * 4 + 3] = src[i * 4 + 3] as f64;
        }

        for level in 1..ot.mips as usize {
            let (ro, rl, mw, mh) = mip_byte_off(rt.w, rt.h, level);
            let Some(refpx) = decode_mip(&rt.payload[ro..ro + rl], mw, mh) else {
                continue;
            };

            let a = {
                let mut cur = lin_f32_lut.clone();
                let (mut cw, mut chh) = (ot.w, ot.h);
                for _ in 0..level {
                    let (nx, nw, nh) = box_halve(&cur, cw, chh);
                    cur = nx;
                    cw = nw;
                    chh = nh;
                }
                let mut out = vec![0u8; cw * chh * 4];
                for i in 0..cw * chh {
                    for c in 0..3 {
                        out[i * 4 + c] = linear_to_srgb_u8(cur[i * 4 + c]);
                    }
                    out[i * 4 + 3] = (cur[i * 4 + 3] + 0.5).floor().clamp(0.0, 255.0) as u8;
                }
                out
            };

            let b = {
                let mut cur = lin_f32_pow.clone();
                let (mut cw, mut chh) = (ot.w, ot.h);
                for _ in 0..level {
                    let (nx, nw, nh) = box_halve(&cur, cw, chh);
                    cur = nx;
                    cw = nw;
                    chh = nh;
                }
                let mut out = vec![0u8; cw * chh * 4];
                for i in 0..cw * chh {
                    for c in 0..3 {
                        out[i * 4 + c] = lin_to_srgb_pow_round(cur[i * 4 + c]);
                    }
                    out[i * 4 + 3] = (cur[i * 4 + 3] + 0.5).floor().clamp(0.0, 255.0) as u8;
                }
                out
            };

            let cc = {
                let mut cur = lin_f64_pow.clone();
                let (mut cw, mut chh) = (ot.w, ot.h);
                for _ in 0..level {
                    let (nx, nw, nh) = box_halve_generic(&cur, cw, chh);
                    cur = nx;
                    cw = nw;
                    chh = nh;
                }
                let mut out = vec![0u8; cw * chh * 4];
                for i in 0..cw * chh {
                    for c in 0..3 {
                        out[i * 4 + c] = lin_to_srgb_pow_round(cur[i * 4 + c] as f32);
                    }
                    out[i * 4 + 3] = (cur[i * 4 + 3] + 0.5).floor().clamp(0.0, 255.0) as u8;
                }
                out
            };

            let d = {
                let mut cur = src.clone();
                let (mut cw, mut chh) = (ot.w, ot.h);
                for _ in 0..level {
                    let (nx, nw, nh) = box_halve_u8(&cur, cw, chh, true);
                    cur = nx;
                    cw = nw;
                    chh = nh;
                }
                cur
            };

            let e = {
                let mut cur = src.clone();
                let (mut cw, mut chh) = (ot.w, ot.h);
                for _ in 0..level {
                    let (nx, nw, nh) = box_halve_u8(&cur, cw, chh, false);
                    cur = nx;
                    cw = nw;
                    chh = nh;
                }
                cur
            };

            let (ea, na, ma) = cmp(&a, &refpx);
            let (eb, _, mb) = cmp(&b, &refpx);
            let (ec, _, mc) = cmp(&cc, &refpx);
            let (ed, _, md) = cmp(&d, &refpx);
            let (ee, _, me) = cmp(&e, &refpx);
            println!(
                "  m{level} {mw}x{mh} ({na}px) exact/maxRGBA  A_curr {ea} {ma:?} | B_f32pow {eb} {mb:?} | C_f64pow {ec} {mc:?} | D_u8round {ed} {md:?} | E_u8trunc {ee} {me:?}"
            );
        }
    }
}
