use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn cf(c: &Value, k: &str) -> i64 {
    c.as_map().unwrap().get(k).and_then(|x| x.as_i64()).unwrap()
}

struct MeshVtx {
    weights: Vec<[u32; 4]>,
}

fn read_blendweights(p: &Path) -> Vec<(i64, MeshVtx)> {
    let bytes = match std::fs::read(p) {
        Ok(b) => b,
        Err(_) => return vec![],
    };
    let b = match Bundle::load_bytes(&bytes) {
        Ok(b) => b,
        Err(_) => return vec![],
    };
    let sf = match b.serialized() {
        Some(s) => s,
        None => return vec![],
    };
    let mut out = Vec::new();
    for o in sf.objects.iter() {
        if o.class_id != 43 {
            continue;
        }
        let v = match sf.read_typetree(o) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let m = match v.as_map() {
            Some(m) => m,
            None => continue,
        };
        let vd = match m.get("m_VertexData").and_then(|x| x.as_map()) {
            Some(x) => x,
            None => continue,
        };
        let chans = match vd.get("m_Channels").and_then(|x| x.as_array()) {
            Some(c) => c.to_vec(),
            None => continue,
        };
        let data = match vd.get("m_DataSize") {
            Some(Value::Bytes(d)) => d.clone(),
            _ => continue,
        };
        let vc = match vd.get("m_VertexCount").and_then(|x| x.as_i64()) {
            Some(x) => x as usize,
            None => continue,
        };
        const CH_BW: usize = 12;
        if chans.len() <= CH_BW || cf(&chans[CH_BW], "dimension") <= 0 {
            continue;
        }
        let mut by_stream: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
        for (i, c) in chans.iter().enumerate() {
            if cf(c, "dimension") > 0 {
                by_stream.entry(cf(c, "stream")).or_default().push(i);
            }
        }
        let mut base = 0usize;
        let mut stream_base: BTreeMap<i64, (usize, usize)> = BTreeMap::new();
        for (si, (s, cis)) in by_stream.iter().enumerate() {
            if si > 0 {
                while !base.is_multiple_of(16) {
                    base += 1;
                }
            }
            let stride_raw = cis
                .iter()
                .map(|&ci| cf(&chans[ci], "offset") + cf(&chans[ci], "dimension") * 4)
                .max()
                .unwrap();
            let stride = ((stride_raw + 3) & !3) as usize;
            stream_base.insert(*s, (base, stride));
            base += stride * vc;
        }
        let s = cf(&chans[CH_BW], "stream");
        let (b0, st) = stream_base[&s];
        let coff = cf(&chans[CH_BW], "offset") as usize;
        let mut weights = Vec::with_capacity(vc);
        let mut ok = true;
        for vix in 0..vc {
            let row = b0 + vix * st;
            let mut w = [0u32; 4];
            for (k, wk) in w.iter_mut().enumerate() {
                let off = row + coff + k * 4;
                if off + 4 > data.len() {
                    ok = false;
                    break;
                }
                *wk = u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
            }
            if !ok {
                break;
            }
            weights.push(w);
        }
        if ok {
            out.push((o.path_id, MeshVtx { weights }));
        }
    }
    out
}

#[inline]
fn f32b(u: u32) -> f32 {
    f32::from_bits(u)
}

fn solve_recip(total: f32, w_ours: &[u32; 4], w_ref: &[u32; 4]) -> Option<(u32, u32)> {
    if total <= 0.0 || !total.is_finite() {
        return None;
    }
    let center = (1.0f32 / total).to_bits();
    const W: i64 = 80;
    let mut lo: Option<u32> = None;
    let mut hi: Option<u32> = None;
    let check = |mbits: u32| -> bool {
        let m = f32b(mbits);
        if !m.is_finite() {
            return false;
        }
        for k in 0..4 {
            let wo = f32b(w_ours[k]);
            if wo == 0.0 {
                if f32b(w_ref[k]) != 0.0 {
                    return false;
                }
                continue;
            }
            let prod = (wo * m).to_bits();
            if prod != w_ref[k] {
                return false;
            }
        }
        true
    };
    for d in -W..=W {
        let mbits = (center as i64 + d) as u32;
        if check(mbits) {
            if lo.is_none() {
                lo = Some(mbits);
            }
            hi = Some(mbits);
        } else if lo.is_some() {
            break;
        }
    }
    match (lo, hi) {
        (Some(l), Some(h)) => Some((l, h)),
        _ => None,
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let ours_dir = a.next().expect("ours dir");
    let ref_dir = a.next().expect("ref dir");
    let out_path = a.next();

    let entries: Vec<_> = std::fs::read_dir(&ref_dir)
        .expect("read ref dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.contains('.'))
        .collect();

    use std::io::Write;
    let mut w: Box<dyn Write> = match &out_path {
        Some(p) => Box::new(std::io::BufWriter::new(std::fs::File::create(p).unwrap())),
        None => Box::new(std::io::BufWriter::new(std::io::stdout())),
    };
    writeln!(w, "total_bits\ttotal_f32\trecip_lo\trecip_hi\tnlanes\tcid").unwrap();

    let mut n_samples = 0u64;
    let mut n_vertices = 0u64;
    let mut n_skinned_meshes = 0u64;
    let mut n_inconsistent = 0u64;
    let mut n_already_id = 0u64;

    for cid in &entries {
        let odir = PathBuf::from(&ours_dir).join(cid);
        let rdir = PathBuf::from(&ref_dir).join(cid);
        if !odir.is_dir() || !rdir.is_dir() {
            continue;
        }

        let bundle_files: Vec<String> = std::fs::read_dir(&odir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with("_windows") || n.ends_with("_mac"))
            .collect();
        for bf in &bundle_files {
            let op = odir.join(bf);
            let rp = rdir.join(bf);
            if !rp.exists() {
                continue;
            }
            harvest_pair(
                &op,
                &rp,
                cid,
                w.as_mut(),
                &mut n_skinned_meshes,
                &mut n_vertices,
                &mut n_already_id,
                &mut n_samples,
                &mut n_inconsistent,
            );
        }
    }
    w.flush().unwrap();
    eprintln!("skinned_meshes={n_skinned_meshes} vertices={n_vertices} already_id={n_already_id} samples={n_samples} inconsistent={n_inconsistent}");
}

#[allow(clippy::too_many_arguments)]
fn harvest_pair(
    op: &Path,
    rp: &Path,
    cid: &str,
    w: &mut dyn std::io::Write,
    n_skinned_meshes: &mut u64,
    n_vertices: &mut u64,
    n_already_id: &mut u64,
    n_samples: &mut u64,
    n_inconsistent: &mut u64,
) {
    {
        let ours = read_blendweights(op);
        if ours.is_empty() {
            return;
        }
        let refm = read_blendweights(rp);
        let refmap: BTreeMap<i64, &MeshVtx> = refm.iter().map(|(p, m)| (*p, m)).collect();
        for (pid, om) in &ours {
            let rm = match refmap.get(pid) {
                Some(r) => *r,
                None => continue,
            };
            if om.weights.len() != rm.weights.len() {
                continue;
            }
            *n_skinned_meshes += 1;
            for (wo, wr) in om.weights.iter().zip(rm.weights.iter()) {
                *n_vertices += 1;
                let total = ((f32b(wo[0]) + f32b(wo[1])) + f32b(wo[2])) + f32b(wo[3]);
                if wo == wr {
                    *n_already_id += 1;
                    continue;
                }
                match solve_recip(total, wo, wr) {
                    Some((lo, hi)) => {
                        let nz = wo.iter().filter(|&&x| f32b(x) != 0.0).count();
                        writeln!(
                            w,
                            "{:08x}\t{:.9}\t{:08x}\t{:08x}\t{}\t{}",
                            total.to_bits(),
                            total,
                            lo,
                            hi,
                            nz,
                            cid
                        )
                        .unwrap();
                        *n_samples += 1;
                    }
                    None => {
                        *n_inconsistent += 1;
                    }
                }
            }
        }
    }
}
