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
    fmt: i64,
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
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("tex")
                .to_string();
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let fmt = gi(&v, "m_TextureFormat");
            let mips = gi(&v, "m_MipCount");
            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(bts) if !bts.is_empty() => Some(bts.as_slice()),
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
                    fmt,
                    mips,
                    payload,
                },
            );
        }
    }
    out
}

type BlockDecoder = fn(&[u8], usize, usize, &mut [u32]) -> Result<(), &'static str>;

fn decode_level(data: &[u8], w: usize, h: usize, fmt: i64) -> Option<Vec<u8>> {
    let mut rgba = vec![0u8; w * h * 4];
    let dec = |f: BlockDecoder, rgba: &mut [u8]| -> bool {
        let mut px = vec![0u32; w * h];
        if f(data, w, h, &mut px).is_err() {
            return false;
        }
        for (i, p) in px.iter().enumerate() {
            let [b, g, r, a] = p.to_le_bytes();
            rgba[i * 4] = r;
            rgba[i * 4 + 1] = g;
            rgba[i * 4 + 2] = b;
            rgba[i * 4 + 3] = a;
        }
        true
    };
    let ok = match fmt {
        25 => dec(texture2ddecoder::decode_bc7, &mut rgba),
        29 => dec(texture2ddecoder::decode_bc5, &mut rgba),
        10 => dec(texture2ddecoder::decode_bc1, &mut rgba),
        12 => dec(texture2ddecoder::decode_bc3, &mut rgba),
        4 => {
            let need = w * h * 4;
            if data.len() >= need {
                rgba.copy_from_slice(&data[..need]);
                true
            } else {
                false
            }
        }
        3 => {
            let need = w * h * 3;
            if data.len() >= need {
                for i in 0..w * h {
                    rgba[i * 4] = data[i * 3];
                    rgba[i * 4 + 1] = data[i * 3 + 1];
                    rgba[i * 4 + 2] = data[i * 3 + 2];
                    rgba[i * 4 + 3] = 255;
                }
                true
            } else {
                false
            }
        }
        _ => false,
    };
    if ok {
        Some(rgba)
    } else {
        None
    }
}

fn level_bytes(w: usize, h: usize, fmt: i64) -> usize {
    match fmt {
        25 | 12 | 29 => w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 16,
        10 => w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * 8,
        4 | 5 => w * h * 4,
        3 => w * h * 3,
        _ => 0,
    }
}

#[derive(Default)]
struct Acc {
    n_diff_t: u64,
    n_diff_o: u64,
    n_t: u64,
    n_o: u64,
    sumd_t: u64,
    sumd_o: u64,
    maxd_t: u32,
    maxd_o: u32,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let walk_mips = args.iter().any(|a| a == "--mips");
    let json_in = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .expect("pairs.json");
    let pairs: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(json_in).unwrap()).unwrap();

    let mut verdicts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut results: Vec<serde_json::Value> = Vec::new();

    for pair in &pairs {
        let op = pair["ours"].as_str().unwrap();
        let rp = pair["ref"].as_str().unwrap();
        let cat = pair.get("cat").and_then(|x| x.as_str()).unwrap_or("?");
        let (Ok(ob), Ok(rb)) = (
            Bundle::load(std::path::Path::new(op)),
            Bundle::load(std::path::Path::new(rp)),
        ) else {
            *verdicts.entry("load-err").or_default() += 1;
            continue;
        };
        let ot = extract(&ob);
        let rt = extract(&rb);
        for (pid, o) in &ot {
            let Some(r) = rt.get(pid) else { continue };
            if o.w != r.w || o.h != r.h || o.fmt != r.fmt {
                *verdicts.entry("shape-mismatch").or_default() += 1;
                continue;
            }

            if o.fmt != 25 {
                continue;
            }
            let mut acc = Acc::default();
            let nlevels = if walk_mips { o.mips.max(1) as usize } else { 1 };
            let (mut mw, mut mh) = (o.w, o.h);
            let (mut ooff, mut roff) = (0usize, 0usize);
            for _lvl in 0..nlevels {
                let lb = level_bytes(mw, mh, o.fmt);
                if ooff + lb > o.payload.len() || roff + lb > r.payload.len() {
                    break;
                }
                let od = decode_level(&o.payload[ooff..ooff + lb], mw, mh, o.fmt);
                let rd = decode_level(&r.payload[roff..roff + lb], mw, mh, r.fmt);
                if let (Some(od), Some(rd)) = (od, rd) {
                    for i in 0..mw * mh {
                        let ra = rd[i * 4 + 3];
                        let dr = (od[i * 4] as i32 - rd[i * 4] as i32).unsigned_abs();
                        let dg = (od[i * 4 + 1] as i32 - rd[i * 4 + 1] as i32).unsigned_abs();
                        let db = (od[i * 4 + 2] as i32 - rd[i * 4 + 2] as i32).unsigned_abs();
                        let d = dr + dg + db;
                        let maxc = dr.max(dg).max(db);
                        if ra == 0 {
                            acc.n_t += 1;
                            if d > 0 {
                                acc.n_diff_t += 1;
                                acc.sumd_t += d as u64;
                                acc.maxd_t = acc.maxd_t.max(maxc);
                            }
                        } else {
                            acc.n_o += 1;
                            if d > 0 {
                                acc.n_diff_o += 1;
                                acc.sumd_o += d as u64;
                                acc.maxd_o = acc.maxd_o.max(maxc);
                            }
                        }
                    }
                }
                ooff += lb;
                roff += lb;
                mw = (mw / 2).max(1);
                mh = (mh / 2).max(1);
            }

            if acc.n_diff_t == 0 && acc.n_diff_o == 0 {
                *verdicts.entry("identical").or_default() += 1;
                continue;
            }

            let verdict = if acc.n_t == 0 {
                "no-transparent"
            } else {
                let frac_t = acc.n_diff_t as f64 / (acc.n_diff_t + acc.n_diff_o).max(1) as f64;
                let big_t = acc.maxd_t >= 8;
                if frac_t >= 0.6 && big_t {
                    "bleed-shaped"
                } else if acc.maxd_o <= 3 && acc.maxd_t <= 3 {
                    "encoder-noise"
                } else if frac_t < 0.4 {
                    "opaque-driven"
                } else {
                    "mixed"
                }
            };
            *verdicts.entry(verdict).or_default() += 1;
            results.push(serde_json::json!({
                "ours": op, "ref": rp, "cat": cat, "pid": pid,
                "name": o.name, "w": o.w, "h": o.h,
                "n_t": acc.n_t, "n_o": acc.n_o,
                "diff_t": acc.n_diff_t, "diff_o": acc.n_diff_o,
                "sumd_t": acc.sumd_t, "sumd_o": acc.sumd_o,
                "maxd_t": acc.maxd_t, "maxd_o": acc.maxd_o,
                "verdict": verdict,
            }));
        }
    }

    eprintln!("=== verdict tally ===");
    for (k, c) in &verdicts {
        eprintln!("{k}\t{c}");
    }
    let out = "/tmp/bleedsplit-out.json";
    std::fs::write(out, serde_json::to_vec(&results).unwrap()).unwrap();
    eprintln!("wrote {} texture results to {out}", results.len());
}
