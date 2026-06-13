use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

const SEEDS: [(u8, u8, u8); 16] = [
    (255, 0, 0),
    (0, 255, 0),
    (0, 0, 255),
    (255, 255, 0),
    (255, 0, 255),
    (0, 255, 255),
    (255, 128, 0),
    (128, 0, 255),
    (0, 128, 255),
    (128, 255, 0),
    (255, 0, 128),
    (0, 255, 128),
    (200, 60, 60),
    (60, 200, 60),
    (60, 60, 200),
    (220, 220, 220),
];

fn main() {
    let path = std::env::args().nth(1).expect("bundle");
    let b = Bundle::load(std::path::Path::new(&path)).unwrap();
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
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let fmt = gi(&v, "m_TextureFormat");
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
            found = Some((w, h, fmt, payload));
        }
    }
    let (w, h, fmt, pay) = found.expect("no texture");
    eprintln!("{w}x{h} fmt={fmt}");
    assert_eq!(fmt, 25, "expected BC7");
    let bw = w.div_ceil(4);
    let bh = h.div_ceil(4);
    let mut pix = vec![0u32; w * h];
    texture2ddecoder::decode_bc7(&pay[..bw * bh * 16], w, h, &mut pix).unwrap();

    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let p = pix[(h - 1 - y) * w + x];
            let i = (y * w + x) * 4;
            rgba[i] = ((p >> 16) & 0xFF) as u8;
            rgba[i + 1] = ((p >> 8) & 0xFF) as u8;
            rgba[i + 2] = (p & 0xFF) as u8;
            rgba[i + 3] = ((p >> 24) & 0xFF) as u8;
        }
    }

    let letters = "rgbymcophskelnuw";
    let classify = |x: usize, y: usize| -> char {
        let i = (y * w + x) * 4;
        let (r, g, bch, a) = (rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]);
        if r < 6 && g < 6 && bch < 6 {
            return if a > 0 { '#' } else { '.' };
        }
        for (k, (sr, sg, sb)) in SEEDS.iter().enumerate() {
            if r == *sr && g == *sg && bch == *sb {
                return letters.as_bytes()[k] as char;
            }
        }
        for (k, (sr, sg, sb)) in SEEDS.iter().enumerate() {
            if (r as i32 - *sr as i32).abs() <= 8
                && (g as i32 - *sg as i32).abs() <= 8
                && (bch as i32 - *sb as i32).abs() <= 8
            {
                return letters.as_bytes()[k].to_ascii_uppercase() as char;
            }
        }
        '?'
    };
    for (sy, sx) in [(24usize, 24usize), (24, 72), (72, 24)] {
        println!("--- seed at png({sx},{sy}) ---");
        for y in sy.saturating_sub(20)..(sy + 21).min(h) {
            let mut line = String::new();
            for x in sx.saturating_sub(20)..(sx + 21).min(w) {
                line.push(classify(x, y));
            }
            println!("{line}");
        }
    }

    println!("--- pair region png(8..48, 208..240) ---");
    for y in 208..240 {
        let mut line = String::new();
        for x in 8..48 {
            line.push(classify(x, y));
        }
        println!("{line}");
    }

    println!("--- pair exact RGB rows 215..217,223..225,231..233 x=12..44 ---");
    for y in [215usize, 216, 217, 223, 224, 225, 231, 232, 233] {
        let mut line = format!("y={y:3} ");
        for x in 12..44 {
            let i = (y * w + x) * 4;
            line += &format!("{:3},{:3},{:3}|", rgba[i], rgba[i + 1], rgba[i + 2]);
        }
        println!("{line}");
    }

    println!("--- edge rows exact RGB (y=64..72, x=188..212) ---");
    for y in 64..72 {
        let mut line = format!("y={y:3} ");
        for x in 188..212 {
            let i = (y * w + x) * 4;
            line += &format!(
                "{:3},{:3},{:3},{:3}|",
                rgba[i],
                rgba[i + 1],
                rgba[i + 2],
                rgba[i + 3]
            );
        }
        println!("{line}");
    }

    println!("--- alpha staircase x=134..146 rows 0..64 (exact RGBA) ---");
    for y in (0..64).step_by(2) {
        let mut line = format!("y={y:3} ");
        for x in 136..145 {
            let i = (y * w + x) * 4;
            line += &format!(
                "{:3},{:3},{:3},{:3}|",
                rgba[i],
                rgba[i + 1],
                rgba[i + 2],
                rgba[i + 3]
            );
        }
        println!("{line}");
    }
}
