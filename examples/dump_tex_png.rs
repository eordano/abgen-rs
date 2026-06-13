use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn main() {
    let bundle_path = std::env::args().nth(1).expect("bundle path");
    let out_dir = std::env::args().nth(2).expect("out dir");
    std::fs::create_dir_all(&out_dir).unwrap();

    let b = Bundle::load(std::path::Path::new(&bundle_path)).unwrap();

    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data));
        }
    }

    let mut n = 0;
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for obj in &sf.objects {
            if obj.class_id != 28 {
                continue;
            }
            let v = match sf.read_typetree(obj) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("read_typetree pid={}: {e}", obj.path_id);
                    continue;
                }
            };
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("tex")
                .to_string();
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let fmt = gi(&v, "m_TextureFormat");

            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(bts) if !bts.is_empty() => Some(bts.as_slice()),
                _ => None,
            });
            let stream_buf;
            let payload: &[u8] = if let Some(d) = inline {
                d
            } else if let Some(sd) = v.get("m_StreamData") {
                let off = gi(sd, "offset") as usize;
                let size = gi(sd, "size") as usize;
                let path = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                let base = path.rsplit('/').next().unwrap_or(path);
                let Some((_, data)) = ress.iter().find(|(nm, _)| nm == base) else {
                    eprintln!("{name}: stream path '{path}' not in bundle");
                    continue;
                };
                if off + size > data.len() {
                    eprintln!("{name}: stream range OOB");
                    continue;
                }
                stream_buf = data[off..off + size].to_vec();
                &stream_buf
            } else {
                eprintln!("{name}: no payload");
                continue;
            };

            let mut rgba = vec![0u8; w * h * 4];
            let ok = match fmt {
                25 => decode_blocks(payload, w, h, &mut rgba, texture2ddecoder::decode_bc7),
                29 => decode_blocks(payload, w, h, &mut rgba, texture2ddecoder::decode_bc5),
                10 => decode_blocks(payload, w, h, &mut rgba, texture2ddecoder::decode_bc1),
                12 => decode_blocks(payload, w, h, &mut rgba, texture2ddecoder::decode_bc3),
                4 => {
                    let need = w * h * 4;
                    if payload.len() >= need {
                        rgba.copy_from_slice(&payload[..need]);
                        true
                    } else {
                        false
                    }
                }
                5 => {
                    let need = w * h * 4;
                    if payload.len() >= need {
                        for i in 0..w * h {
                            rgba[i * 4] = payload[i * 4 + 1];
                            rgba[i * 4 + 1] = payload[i * 4 + 2];
                            rgba[i * 4 + 2] = payload[i * 4 + 3];
                            rgba[i * 4 + 3] = payload[i * 4];
                        }
                        true
                    } else {
                        false
                    }
                }
                3 => {
                    let need = w * h * 3;
                    if payload.len() >= need {
                        for i in 0..w * h {
                            rgba[i * 4] = payload[i * 3];
                            rgba[i * 4 + 1] = payload[i * 3 + 1];
                            rgba[i * 4 + 2] = payload[i * 3 + 2];
                            rgba[i * 4 + 3] = 255;
                        }
                        true
                    } else {
                        false
                    }
                }
                other => {
                    eprintln!("{name}: unsupported TextureFormat {other}");
                    false
                }
            };
            if !ok {
                continue;
            }

            let mut flipped = vec![0u8; w * h * 4];
            for y in 0..h {
                let src = (h - 1 - y) * w * 4;
                flipped[y * w * 4..(y + 1) * w * 4].copy_from_slice(&rgba[src..src + w * 4]);
            }

            let safe: String = name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let out = format!("{out_dir}/{:02}_{safe}_{}x{}_fmt{}.png", n, w, h, fmt);
            image::save_buffer(&out, &flipped, w as u32, h as u32, image::ColorType::Rgba8)
                .unwrap();
            println!("{out}\tpid={}\t{}x{} fmt={}", obj.path_id, w, h, fmt);
            n += 1;
        }
    }
    eprintln!("decoded {n} textures");
}

type BlockDecoder = fn(&[u8], usize, usize, &mut [u32]) -> Result<(), &'static str>;

fn decode_blocks(data: &[u8], w: usize, h: usize, rgba: &mut [u8], f: BlockDecoder) -> bool {
    let mut px = vec![0u32; w * h];
    match f(data, w, h, &mut px) {
        Ok(()) => {
            for (i, p) in px.iter().enumerate() {
                let [b, g, r, a] = p.to_le_bytes();
                rgba[i * 4] = r;
                rgba[i * 4 + 1] = g;
                rgba[i * 4 + 2] = b;
                rgba[i * 4 + 3] = a;
            }
            true
        }
        Err(e) => {
            eprintln!("decode failed: {e}");
            false
        }
    }
}
