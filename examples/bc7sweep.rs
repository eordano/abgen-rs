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
            let mips = gi(&v, "m_MipCount") as i32;
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
            found = Some((w, h, mips, cs, payload));
        }
    }
    let (tw, th, mips, cs, rpay) = found.expect("no BC7 texture in ref");
    println!(
        "src {sw}x{sh} -> target {tw}x{th} mips={mips} cs={cs} ref payload {} blocks",
        rpay.len() / 16
    );

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
    if has_alpha {
        abgen::alpha_bleed::alpha_bleed_inplace(&mut bled, tw, th);
    }

    let mip0_blocks = (tw.div_ceil(4) * th.div_ceil(4)) as usize;

    for (bname, buf) in [("bleed", &bled), ("nobleed", &resized)] {
        if !has_alpha && bname == "bleed" {
            continue;
        }
        for profile in [
            abgen::bc7_pure::Bc7Profile::Basic,
            abgen::bc7_pure::Bc7Profile::Slow,
        ] {
            for perceptual in [true, false] {
                for srgb in [cs == 1, cs != 1] {
                    let (ours, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
                        buf,
                        tw,
                        th,
                        Some(mips),
                        true,
                        srgb,
                        perceptual,
                        profile,
                    );
                    if ours.len() != rpay.len() {
                        println!("{bname} {profile:?} perc={perceptual} srgb={srgb}: LEN MISMATCH {} vs {}", ours.len(), rpay.len());
                        continue;
                    }
                    let nb = ours.len() / 16;
                    let mut ident = 0;
                    let mut ident0 = 0;
                    for i in 0..nb {
                        if ours[i * 16..i * 16 + 16] == rpay[i * 16..i * 16 + 16] {
                            ident += 1;
                            if i < mip0_blocks {
                                ident0 += 1;
                            }
                        }
                    }
                    println!(
                        "{bname:7} {profile:?} perc={perceptual:5} srgb={srgb:5}: ident {ident}/{nb} ({:.1}%)  mip0 {ident0}/{mip0_blocks} ({:.1}%)",
                        100.0 * ident as f64 / nb as f64,
                        100.0 * ident0 as f64 / mip0_blocks as f64
                    );
                }
            }
        }
    }
}
