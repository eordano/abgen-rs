use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

type Decoded = Option<(Vec<u8>, u32, u32)>;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn ref_tex(path: &str) -> Option<(u32, u32, i32, i64, Vec<u8>)> {
    let b = Bundle::load(std::path::Path::new(path)).ok()?;
    let mut ress: Vec<(String, Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data.clone()));
        }
    }
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for obj in &sf.objects {
            if obj.class_id != 28 {
                continue;
            }
            let v = sf.read_typetree(obj).ok()?;
            if gi(&v, "m_TextureFormat") != 25 {
                continue;
            }
            let payload: Vec<u8> = match v.get("image data") {
                Some(Value::Bytes(bts)) if !bts.is_empty() => bts.clone(),
                _ => {
                    let sd = v.get("m_StreamData")?;
                    let off = gi(sd, "offset") as usize;
                    let size = gi(sd, "size") as usize;
                    let p = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                    let base = p.rsplit('/').next().unwrap_or(p);
                    let (_, d) = ress.iter().find(|(nm, _)| nm == base)?;
                    d[off..off + size].to_vec()
                }
            };
            return Some((
                gi(&v, "m_Width") as u32,
                gi(&v, "m_Height") as u32,
                gi(&v, "m_MipCount") as i32,
                gi(&v, "m_ColorSpace"),
                payload,
            ));
        }
    }
    None
}

fn main() {
    let src_path = std::env::args().nth(1).expect("source jpeg");
    let ref_path = std::env::args().nth(2).expect("ref bundle");
    let raw = std::fs::read(&src_path).unwrap();
    let (tw, th, mips, cs, rpay) = ref_tex(&ref_path).expect("ref texture");
    let mip0_blocks = (tw.div_ceil(4) * th.div_ceil(4)) as usize;

    let inputs: Vec<(&str, Decoded)> = vec![
        ("9c-box   ", libjpeg9c::decode_rgba(&raw, false)),
        ("9c-fancy ", libjpeg9c::decode_rgba(&raw, true)),
        ("turbo-fcy", abgen::ffi::decode_jpeg_rgba(&raw).ok()),
        (
            "turbo-box",
            { std::env::set_var("ABGEN_JPEG_TURBO_BOX","1"); let r=abgen::ffi::decode_jpeg_rgba_box(&raw).ok(); std::env::remove_var("ABGEN_JPEG_TURBO_BOX"); r },
        ),
    ];
    for (name, dec) in inputs {
        let Some((rgba, w, h)) = dec else {
            println!("{name}: decode failed");
            continue;
        };
        if (w, h) != (tw, th) {
            println!("{name}: size {w}x{h} != target {tw}x{th} (resize case, skipping)");
            continue;
        }
        for (pname, profile) in [
            ("slow ", abgen::bc7_pure::Bc7Profile::Slow),
            ("basic", abgen::bc7_pure::Bc7Profile::Basic),
        ] {
            let srgb = cs == 1;
            let perceptual = srgb;
            let (ours, _) = abgen::bc7_pure::encode_bc7_mip_chain_with_profile(
                &rgba, w, h, Some(mips), true, srgb, perceptual, profile,
            );
            if ours.len() != rpay.len() {
                println!("{name} {pname}: LEN MISMATCH {} vs {}", ours.len(), rpay.len());
                continue;
            }
            let nb = ours.len() / 16;
            let mut ident = 0usize;
            let mut ident0 = 0usize;
            for i in 0..nb {
                if ours[i * 16..i * 16 + 16] == rpay[i * 16..i * 16 + 16] {
                    ident += 1;
                    if i < mip0_blocks {
                        ident0 += 1;
                    }
                }
            }
            println!(
                "{name} {pname} cs={cs}: ident {ident}/{nb} ({:.1}%)  mip0 {ident0}/{mip0_blocks} ({:.1}%)",
                100.0 * ident as f64 / nb as f64,
                100.0 * ident0 as f64 / mip0_blocks as f64
            );
        }
    }
}
