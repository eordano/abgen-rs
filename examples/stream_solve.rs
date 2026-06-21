use abgen::unity::bundle_file::{Bundle, FileContent};
use std::collections::BTreeMap;

type Keys = BTreeMap<(u32, i32), [u32; 4]>;
type Frames = Vec<(u32, Vec<i32>)>;
type Clip = (String, Keys, Frames);

fn clips(b: &Bundle) -> Vec<Clip> {
    let mut out = Vec::new();
    for f in &b.files {
        if let FileContent::Serialized(sf) = &f.content {
            for o in &sf.objects {
                if o.class_id != 74 {
                    continue;
                }
                let v = sf.read_typetree(o).unwrap();
                let name = v
                    .get("m_Name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let data = v
                    .get("m_MuscleClip")
                    .and_then(|m| m.get("m_Clip"))
                    .and_then(|m| m.get("data"))
                    .and_then(|m| m.get("m_StreamedClip"))
                    .and_then(|m| m.get("data"))
                    .and_then(|m| m.as_array())
                    .map(|a| a.to_vec())
                    .unwrap_or_default();
                let words: Vec<u32> = data.iter().map(|x| x.as_i64().unwrap() as u32).collect();
                let mut keys: Keys = BTreeMap::new();
                let mut frames: Vec<(u32, Vec<i32>)> = Vec::new();
                let mut i = 0usize;
                while i + 2 <= words.len() {
                    let t = words[i];
                    let n = words[i + 1] as i32;
                    i += 2;
                    let mut idxs = Vec::new();
                    for _ in 0..n {
                        let ci = words[i] as i32;
                        let c = [words[i + 1], words[i + 2], words[i + 3], words[i + 4]];
                        keys.insert((t, ci), c);
                        idxs.push(ci);
                        i += 5;
                    }
                    frames.push((t, idxs));
                }
                out.push((name, keys, frames));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let bo = Bundle::load(std::path::Path::new(&args.next().unwrap())).unwrap();
    let br = Bundle::load(std::path::Path::new(&args.next().unwrap())).unwrap();
    let max: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(20);
    let co = clips(&bo);
    let cr = clips(&br);
    for ((no, ko, _), (nr, kr, fr)) in co.iter().zip(cr.iter()) {
        println!("== clip {no} / {nr}: keys {} vs {}", ko.len(), kr.len());
        let mut slot_count = [0usize; 4];
        let mut shown = 0;
        for (k, vr) in kr.iter() {
            let Some(vo) = ko.get(k) else { continue };
            for s in 0..4 {
                if vo[s] != vr[s] {
                    slot_count[s] += 1;
                    if shown < max && s < 3 {
                        let (t, ci) = *k;
                        let tf = f32::from_bits(t);
                        let next = fr
                            .iter()
                            .filter(|(ft, idxs)| {
                                f32::from_bits(*ft) > tf
                                    && f32::from_bits(*ft).is_finite()
                                    && idxs.contains(&ci)
                            })
                            .map(|(ft, _)| *ft)
                            .next();
                        let (t1, v1) = match next {
                            Some(ft) => (f32::from_bits(ft), f32::from_bits(kr[&(ft, ci)][3])),
                            None => (f32::NAN, f32::NAN),
                        };
                        println!(
                            "  t={:?} curve={} slot={} our={:e}({:#010x}) ref={:e}({:#010x}) | v0={:e} v1={:e} dt={:?} sec_ref={:e} sec_our={:e}",
                            tf,
                            ci,
                            s,
                            f32::from_bits(vo[s]),
                            vo[s],
                            f32::from_bits(vr[s]),
                            vr[s],
                            f32::from_bits(vr[3]),
                            v1,
                            t1 - tf,
                            f32::from_bits(vr[2]),
                            f32::from_bits(vo[2]),
                        );
                        shown += 1;
                    }
                }
            }
        }
        println!(
            "  slot mismatches: a={} b={} c(slope)={} d(value)={}",
            slot_count[0], slot_count[1], slot_count[2], slot_count[3]
        );
        let mut miss = 0;
        for (k, vr) in kr.iter() {
            if !ko.contains_key(k) && miss < 20 {
                println!(
                    "  ref-only key t={:?} curve={} a={:e} b={:e} c={:e} d={:e}",
                    f32::from_bits(k.0),
                    k.1,
                    f32::from_bits(vr[0]),
                    f32::from_bits(vr[1]),
                    f32::from_bits(vr[2]),
                    f32::from_bits(vr[3]),
                );
                miss += 1;
            }
        }
    }
}
