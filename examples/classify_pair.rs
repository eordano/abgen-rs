use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use rayon::prelude::*;
use std::collections::HashMap;

const FLOAT_REL: f32 = 1e-3;
const TEX_RMSE_FAR: f64 = 0.08;

struct Obj {
    class: i32,
    pid: i64,
    name: String,
    data: Vec<u8>,
}

struct Side {
    objs: Vec<Obj>,
    ress: Vec<(String, Vec<u8>)>,
    textures: Vec<(String, u32, u32, Vec<u8>)>,
}

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn load_side(path: &str) -> Result<Side, String> {
    let b = Bundle::load(std::path::Path::new(path)).map_err(|e| format!("{e:#}"))?;
    let mut objs = Vec::new();
    let mut ress = Vec::new();
    for f in &b.files {
        match &f.content {
            FileContent::Raw(d) => ress.push((f.name.clone(), d.clone())),
            FileContent::Serialized(sf) => {
                for o in &sf.objects {
                    let name = sf
                        .read_typetree(o)
                        .ok()
                        .and_then(|v| v.get("m_Name").and_then(|x| x.as_str()).map(String::from))
                        .unwrap_or_default();
                    objs.push(Obj {
                        class: o.class_id,
                        pid: o.path_id,
                        name,
                        data: o.data.clone(),
                    });
                }
            }
        }
    }

    let mut textures = Vec::new();
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for o in &sf.objects {
            if o.class_id != 28 {
                continue;
            }
            let Ok(v) = sf.read_typetree(o) else { continue };
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let (w, h) = (gi(&v, "m_Width") as usize, gi(&v, "m_Height") as usize);
            let fmt = gi(&v, "m_TextureFormat");
            let inline = v.get("image data").and_then(|x| match x {
                Value::Bytes(b) if !b.is_empty() => Some(b.clone()),
                _ => None,
            });
            let payload: Vec<u8> = if let Some(d) = inline {
                d
            } else if let Some(sd) = v.get("m_StreamData") {
                let off = gi(sd, "offset") as usize;
                let size = gi(sd, "size") as usize;
                let base = sd
                    .get("path")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .to_string();
                match ress.iter().find(|(n, _)| *n == base) {
                    Some((_, d)) if off + size <= d.len() => d[off..off + size].to_vec(),
                    _ => continue,
                }
            } else {
                continue;
            };
            let mut rgba = vec![0u8; w * h * 4];
            let ok = match fmt {
                25 => texture2ddecoder::decode_bc7(&payload, w, h, px_buf(&mut rgba)).is_ok(),
                29 => texture2ddecoder::decode_bc5(&payload, w, h, px_buf(&mut rgba)).is_ok(),
                10 => texture2ddecoder::decode_bc1(&payload, w, h, px_buf(&mut rgba)).is_ok(),
                12 => texture2ddecoder::decode_bc3(&payload, w, h, px_buf(&mut rgba)).is_ok(),
                4 | 5 | 3 => {
                    raw_to_rgba(&payload, w, h, fmt, &mut rgba);
                    true
                }
                _ => false,
            };
            if ok {
                if fmt == 25 || fmt == 29 || fmt == 10 || fmt == 12 {
                    bgra_fixup(&mut rgba);
                }
                textures.push((name, w as u32, h as u32, rgba));
            }
        }
    }
    Ok(Side {
        objs,
        ress,
        textures,
    })
}

fn px_buf(rgba: &mut [u8]) -> &mut [u32] {
    unsafe { std::slice::from_raw_parts_mut(rgba.as_mut_ptr() as *mut u32, rgba.len() / 4) }
}
fn bgra_fixup(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
}
fn raw_to_rgba(p: &[u8], w: usize, h: usize, fmt: i64, out: &mut [u8]) {
    let n = w * h;
    match fmt {
        4 if p.len() >= n * 4 => out.copy_from_slice(&p[..n * 4]),
        5 if p.len() >= n * 4 => {
            for i in 0..n {
                out[i * 4] = p[i * 4 + 1];
                out[i * 4 + 1] = p[i * 4 + 2];
                out[i * 4 + 2] = p[i * 4 + 3];
                out[i * 4 + 3] = p[i * 4];
            }
        }
        3 if p.len() >= n * 3 => {
            for i in 0..n {
                out[i * 4] = p[i * 3];
                out[i * 4 + 1] = p[i * 3 + 1];
                out[i * 4 + 2] = p[i * 3 + 2];
                out[i * 4 + 3] = 255;
            }
        }
        _ => {}
    }
}

fn tex_rmse(a: &Side, b: &Side) -> f64 {
    let mut worst: f64 = 0.0;
    for (an, aw, ah, ad) in &a.textures {
        let mut best: Option<f64> = None;
        for (bn, bw, bh, bd) in &b.textures {
            if an != bn || aw != bw || ah != bh {
                continue;
            }

            let mut acc = 0f64;
            let mut cnt = 0usize;
            for (pa, pb) in ad.chunks_exact(4).zip(bd.chunks_exact(4)) {
                let (aa, ab) = (pa[3] as f64 / 255.0, pb[3] as f64 / 255.0);
                for c in 0..3 {
                    let d = pa[c] as f64 * aa - pb[c] as f64 * ab;
                    acc += d * d;
                }
                let da = (pa[3] as f64 - pb[3] as f64) * 0.5;
                acc += da * da;
                cnt += 4;
            }
            let rmse = (acc / cnt as f64).sqrt() / 255.0;
            best = Some(best.map_or(rmse, |b: f64| b.min(rmse)));
        }
        if let Some(r) = best {
            worst = worst.max(r);
        }
    }
    worst
}

#[derive(Default)]
struct Windows {
    id: usize,
    float: usize,
    flip: usize,
    tex: usize,
    structural: usize,
}

fn close_f32(a: f32, b: f32) -> bool {
    if a == b {
        return true;
    }
    if !a.is_finite() || !b.is_finite() {
        return a.to_bits() == b.to_bits();
    }
    (a - b).abs() <= f32::max(1e-4, FLOAT_REL * f32::max(a.abs(), b.abs()))
}

fn flip_f32(a: f32, b: f32) -> bool {
    a.to_bits() ^ b.to_bits() == 0x8000_0000
}

fn classify_windows(class: i32, a: &[u8], b: &[u8], idmap: &HashMap<i64, i64>, w: &mut Windows) {
    debug_assert_eq!(a.len(), b.len());
    let mut i = 0usize;
    while i < a.len() {
        if a[i] == b[i] {
            i += 1;
            continue;
        }

        let start = i;
        let mut end = i + 1;
        let mut gap = 0;
        while end < a.len() && gap < 8 {
            if a[end] != b[end] {
                gap = 0;
            } else {
                gap += 1;
            }
            end += 1;
        }
        let end = end - gap;
        i = end;

        let s8 = start & !7;
        let e8 = (end + 7) & !7;
        let mut all_id = e8 <= a.len() && e8 > s8;
        let mut any = false;
        let mut off = s8;
        while all_id && off + 8 <= e8.min(a.len()) {
            let av = i64::from_le_bytes(a[off..off + 8].try_into().unwrap());
            let bv = i64::from_le_bytes(b[off..off + 8].try_into().unwrap());
            if av == bv {
                off += 8;
                continue;
            }
            any = true;
            let mapped = idmap.get(&bv) == Some(&av);
            if !mapped {
                all_id = false;
            }
            off += 8;
        }
        if all_id && any {
            w.id += 1;
            continue;
        }

        if class == 28 {
            w.tex += 1;
            continue;
        }

        if class == 142 || class == 49 {
            w.id += 1;
            continue;
        }

        let s4 = start & !3;
        let e4 = (end + 3) & !3;
        let mut all_float = e4 <= a.len() && e4 > s4;
        let mut anyf = false;
        let mut anyflip = false;
        let mut off = s4;
        while all_float && off + 4 <= e4.min(a.len()) {
            let av = f32::from_le_bytes(a[off..off + 4].try_into().unwrap());
            let bv = f32::from_le_bytes(b[off..off + 4].try_into().unwrap());
            if av.to_bits() != bv.to_bits() {
                if flip_f32(av, bv) {
                    anyflip = true;
                } else {
                    anyf = true;
                    if !close_f32(av, bv) {
                        all_float = false;
                    }
                }
            }
            off += 4;
        }
        if all_float && (anyf || anyflip) {
            if anyflip {
                w.flip += 1;
            } else {
                w.float += 1;
            }
            continue;
        }
        w.structural += 1;
    }
}

fn classify(ours: &str, refp: &str) -> (u8, String) {
    let ob = match std::fs::read(ours) {
        Ok(d) => d,
        Err(e) => return (9, format!("read ours: {e}")),
    };
    let rb = match std::fs::read(refp) {
        Ok(d) => d,
        Err(e) => return (9, format!("read ref: {e}")),
    };
    if ob == rb {
        return (1, "bytes equal".into());
    }
    let (os, rs) = match (
        std::panic::catch_unwind(|| load_side(ours)),
        std::panic::catch_unwind(|| load_side(refp)),
    ) {
        (Ok(Ok(a)), Ok(Ok(b))) => (a, b),
        (Ok(Err(e)), _) => return (9, format!("parse ours: {e}")),
        (_, Ok(Err(e))) => return (9, format!("parse ref: {e}")),
        _ => return (9, "parse panic".into()),
    };

    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut used_o = vec![false; os.objs.len()];
    let mut used_r = vec![false; rs.objs.len()];
    let mut by_key: HashMap<(i32, i64), usize> = HashMap::new();
    for (i, o) in os.objs.iter().enumerate() {
        by_key.insert((o.class, o.pid), i);
    }
    for (j, r) in rs.objs.iter().enumerate() {
        if let Some(&i) = by_key.get(&(r.class, r.pid)) {
            if !used_o[i] {
                pairs.push((i, j));
                used_o[i] = true;
                used_r[j] = true;
            }
        }
    }

    for pass in 0..3 {
        for j in 0..rs.objs.len() {
            if used_r[j] {
                continue;
            }
            let r = &rs.objs[j];
            let m = os.objs.iter().enumerate().position(|(i, o)| {
                !used_o[i]
                    && o.class == r.class
                    && match pass {
                        0 => o.name == r.name && o.data.len() == r.data.len(),
                        1 => o.name == r.name,
                        _ => o.data.len() == r.data.len(),
                    }
            });
            if let Some(i) = m {
                pairs.push((i, j));
                used_o[i] = true;
                used_r[j] = true;
            }
        }
    }
    let extra_o = used_o.iter().filter(|u| !**u).count();
    let extra_r = used_r.iter().filter(|u| !**u).count();
    let id_changed = pairs.iter().any(|&(i, j)| os.objs[i].pid != rs.objs[j].pid);
    let idmap: HashMap<i64, i64> = pairs
        .iter()
        .map(|&(i, j)| (rs.objs[j].pid, os.objs[i].pid))
        .collect();

    let mut w = Windows::default();
    if extra_o + extra_r > 0 {
        w.structural += extra_o + extra_r;
    }
    let mut size_mismatch_objs = 0usize;
    for &(i, j) in &pairs {
        let (a, b) = (&os.objs[i].data, &rs.objs[j].data);
        if a.len() != b.len() {
            size_mismatch_objs += 1;
            w.structural += 1;
            continue;
        }
        if a != b {
            classify_windows(os.objs[i].class, a, b, &idmap, &mut w);
        }
    }

    for (name, rd) in &rs.ress {
        match os.ress.iter().find(|(n, _)| n == name) {
            Some((_, od)) if od == rd => {}
            Some((_, od)) => {
                if od.len() == rd.len() {
                    w.tex += 1;
                } else {
                    w.tex += 1;
                }
            }
            None => w.structural += 1,
        }
    }
    let rmse = tex_rmse(&os, &rs);
    let tex_far = rmse > TEX_RMSE_FAR;
    let ev = format!(
        "id_w={} float_w={} flip_w={} tex_w={} struct_w={} obj_pairs={} extra(ours/ref)={}/{} sizemis={} ids_changed={} tex_rmse={:.4} size {}vs{}",
        w.id, w.float, w.flip, w.tex, w.structural, pairs.len(), extra_o, extra_r,
        size_mismatch_objs, id_changed, rmse, ob.len(), rb.len()
    );

    if w.structural > 0 {
        return (if tex_far { 8 } else { 7 }, ev);
    }
    if tex_far {
        return (8, ev);
    }
    if ob.len() == rb.len() {
        if w.tex == 0 && w.float == 0 && w.flip == 0 && w.id > 0 {
            return (2, ev);
        }
        return (3, ev);
    }
    if ob.len() < rb.len() {
        return (if id_changed { 5 } else { 4 }, ev);
    }
    (6, ev)
}

fn main() {
    let pairs_path = std::env::args().nth(1).expect("pairs.tsv");
    let lines: Vec<(String, String, String)> = std::fs::read_to_string(&pairs_path)
        .unwrap()
        .lines()
        .filter_map(|l| {
            let mut it = l.split('\t');
            Some((
                it.next()?.to_string(),
                it.next()?.to_string(),
                it.next()?.to_string(),
            ))
        })
        .collect();
    let out: Vec<String> = lines
        .par_iter()
        .map(|(ours, refp, label)| {
            let (cat, ev) = classify(ours, refp);
            let base = std::path::Path::new(refp)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!("{label}\t{base}\tCAT{cat}\t{ev}")
        })
        .collect();
    for l in out {
        println!("{l}");
    }
}
