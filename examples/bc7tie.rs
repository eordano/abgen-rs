use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn read(&mut self, n: usize) -> u64 {
        let mut v = 0u64;
        for i in 0..n {
            let bit = (self.data[self.pos >> 3] >> (self.pos & 7)) & 1;
            v |= (bit as u64) << i;
            self.pos += 1;
        }
        v
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
struct BlockFields {
    mode: u8,
    partition: u8,
    rotation: u8,
    idx_mode: u8,
    ns: u8,
    endpoints: Vec<[u8; 4]>,
    pbits: Vec<u8>,
    color_indices: Vec<u8>,
    alpha_indices: Vec<u8>,
}

fn parse_block(b: &[u8; 16]) -> Option<BlockFields> {
    let mode = b[0].trailing_zeros() as u8;
    if mode > 7 {
        return None;
    }
    let mut r = BitReader::new(b);
    r.read(mode as usize + 1);
    let mut f = BlockFields {
        mode,
        ..Default::default()
    };
    let (ns, pb, rb, imb, cb, ab, npb, _shared, ib1, ib2) = match mode {
        0 => (3usize, 4, 0, 0, 4, 0, 6, false, 3, 0),
        1 => (2, 6, 0, 0, 6, 0, 2, true, 3, 0),
        2 => (3, 6, 0, 0, 5, 0, 0, false, 2, 0),
        3 => (2, 6, 0, 0, 7, 0, 4, false, 2, 0),
        4 => (1, 0, 2, 1, 5, 6, 0, false, 2, 3),
        5 => (1, 0, 2, 0, 7, 8, 0, false, 2, 2),
        6 => (1, 0, 0, 0, 7, 7, 2, false, 4, 0),
        7 => (2, 6, 0, 0, 5, 5, 4, false, 2, 0),
        _ => unreachable!(),
    };
    f.ns = ns as u8;
    f.partition = r.read(pb) as u8;
    f.rotation = r.read(rb) as u8;
    f.idx_mode = r.read(imb) as u8;
    let neps = ns * 2;
    let mut eps = vec![[0u8; 4]; neps];
    for chan in 0..3 {
        for ep in eps.iter_mut().take(neps) {
            ep[chan] = r.read(cb) as u8;
        }
    }
    if ab > 0 {
        for ep in eps.iter_mut().take(neps) {
            ep[3] = r.read(ab) as u8;
        }
    }
    f.endpoints = eps;
    for _ in 0..npb {
        f.pbits.push(r.read(1) as u8);
    }
    let anchors2 = [
        15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 2, 8, 2, 2, 8, 8, 15,
        2, 8, 2, 2, 8, 8, 2, 2, 15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2, 2, 15, 15, 6, 6, 2, 6, 8,
        15, 15, 2, 2, 15, 15, 15, 15, 15, 2, 2, 15,
    ];
    let anchors3a = [
        3, 3, 15, 15, 8, 3, 15, 15, 8, 8, 6, 6, 6, 5, 3, 3, 3, 3, 8, 15, 3, 3, 6, 10, 5, 8, 8, 6,
        8, 5, 15, 15, 8, 15, 3, 5, 6, 10, 8, 15, 15, 3, 15, 5, 15, 15, 15, 15, 3, 15, 5, 5, 5, 8,
        5, 10, 5, 10, 8, 13, 15, 12, 3, 3,
    ];
    let anchors3b = [
        15, 8, 8, 3, 15, 15, 3, 8, 15, 15, 15, 15, 15, 15, 15, 8, 15, 8, 15, 3, 15, 8, 15, 8, 3,
        15, 6, 10, 15, 15, 10, 8, 15, 3, 15, 10, 10, 8, 9, 10, 6, 15, 8, 15, 3, 6, 6, 8, 15, 3, 15,
        15, 15, 15, 15, 15, 15, 15, 15, 15, 3, 15, 15, 8,
    ];
    let is_anchor = |pix: usize| -> bool {
        if pix == 0 {
            return true;
        }
        if ns == 2 {
            return pix == anchors2[f.partition as usize];
        }
        if ns == 3 {
            return pix == anchors3a[f.partition as usize]
                || pix == anchors3b[f.partition as usize];
        }
        false
    };
    for pix in 0..16 {
        let bits = ib1 - if is_anchor(pix) { 1 } else { 0 };
        f.color_indices.push(r.read(bits) as u8);
    }
    if ib2 > 0 {
        for pix in 0..16 {
            let bits = ib2 - if pix == 0 { 1 } else { 0 };
            f.alpha_indices.push(r.read(bits) as u8);
        }
    }
    Some(f)
}

fn endpoint_swap(o: &BlockFields, r: &BlockFields) -> Option<Vec<bool>> {
    if o.mode != r.mode || o.partition != r.partition || o.ns != r.ns {
        return None;
    }
    let ns = o.ns as usize;
    let mut mask = vec![false; ns];
    let mut any = false;
    for s in 0..ns {
        let ol = o.endpoints[s * 2];
        let oh = o.endpoints[s * 2 + 1];
        let rl = r.endpoints[s * 2];
        let rh = r.endpoints[s * 2 + 1];
        if ol == rl && oh == rh {
            mask[s] = false;
        } else if ol == rh && oh == rl {
            mask[s] = true;
            any = true;
        } else {
            return None;
        }
    }
    if any {
        Some(mask)
    } else {
        None
    }
}

fn decode_block(b: &[u8; 16]) -> [u32; 16] {
    let mut out = [0u32; 16];
    texture2ddecoder::decode_bc7(b, 4, 4, &mut out).ok();
    out
}

fn extract_bc7_payloads(bundle: &Bundle) -> BTreeMap<i64, (String, usize, usize, i64, Vec<u8>)> {
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
            out.insert(obj.path_id, (name, w, h, mips, payload));
        }
    }
    out
}

#[derive(Default)]
struct Agg {
    tie_total: usize,
    nontie: usize,
    unparsed: usize,
    mode_mismatch: BTreeMap<(u8, u8), usize>,
    ep_swap_clean: usize,
    ep_swap_part: BTreeMap<u8, usize>,
    same_mode_ep: usize,
    part_diff: usize,
    rot_idx_diff: usize,
    pbit_only: usize,
    idx_only: usize,
    ref_lower_mode: usize,
    ref_higher_mode: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tsv = args.iter().any(|a| a == "--tsv");
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let ours = Bundle::load(std::path::Path::new(paths[0])).unwrap();
    let refb = Bundle::load(std::path::Path::new(paths[1])).unwrap();
    let op = extract_bc7_payloads(&ours);
    let rp = extract_bc7_payloads(&refb);

    let mut g = Agg::default();

    for (pid, (_name, w, h, mips, opay)) in &op {
        let Some((_rname, rw, rh, rmips, rpay)) = rp.get(pid) else {
            continue;
        };
        if w != rw || h != rh || mips != rmips || opay.len() != rpay.len() {
            continue;
        }
        let nb = opay.len() / 16;
        let mut mip_of = Vec::with_capacity(nb);
        {
            let (mut mw, mut mh) = (*w, *h);
            for m in 0..*mips {
                let bw = mw.div_ceil(4).max(1);
                let bh = mh.div_ceil(4).max(1);
                for _ in 0..bw * bh {
                    mip_of.push(m);
                }
                mw = (mw / 2).max(1);
                mh = (mh / 2).max(1);
            }
        }
        for i in 0..nb {
            let ob: &[u8; 16] = opay[i * 16..i * 16 + 16].try_into().unwrap();
            let rb: &[u8; 16] = rpay[i * 16..i * 16 + 16].try_into().unwrap();
            if ob == rb {
                continue;
            }
            let od = decode_block(ob);
            let rd = decode_block(rb);
            if od != rd {
                g.nontie += 1;
                continue;
            }
            g.tie_total += 1;
            let (Some(of), Some(rf)) = (parse_block(ob), parse_block(rb)) else {
                g.unparsed += 1;
                continue;
            };
            let mut cat = String::new();
            if of.mode != rf.mode {
                *g.mode_mismatch.entry((of.mode, rf.mode)).or_default() += 1;
                if rf.mode < of.mode {
                    g.ref_lower_mode += 1;
                } else {
                    g.ref_higher_mode += 1;
                }
                cat = format!("mode{}->{}", of.mode, rf.mode);
            } else if of.partition != rf.partition {
                g.part_diff += 1;
                cat = format!("m{}part{}->{}", of.mode, of.partition, rf.partition);
            } else if of.rotation != rf.rotation || of.idx_mode != rf.idx_mode {
                g.rot_idx_diff += 1;
                cat = format!("m{}rotidx", of.mode);
            } else if let Some(mask) = endpoint_swap(&of, &rf) {
                g.ep_swap_clean += 1;
                let nsw = mask.iter().filter(|x| **x).count() as u8;
                *g.ep_swap_part.entry(nsw).or_default() += 1;
                cat = format!("m{}EPSWAP{}", of.mode, nsw);
            } else if of.endpoints != rf.endpoints {
                g.same_mode_ep += 1;
                cat = format!("m{}ep", of.mode);
            } else if of.pbits != rf.pbits {
                g.pbit_only += 1;
                cat = format!("m{}pbit", of.mode);
            } else {
                g.idx_only += 1;
                cat = format!("m{}idx", of.mode);
            }
            if tsv {
                let ep_swapped = endpoint_swap(&of, &rf).is_some() as u8;
                let ep_maxd = if of.mode == rf.mode {
                    of.endpoints
                        .iter()
                        .zip(rf.endpoints.iter())
                        .flat_map(|(a, b)| {
                            a.iter()
                                .zip(b.iter())
                                .map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs())
                        })
                        .max()
                        .unwrap_or(0)
                } else {
                    0
                };
                let pbit_diff = (of.pbits != rf.pbits) as u8;
                let idx_diff = (of.color_indices != rf.color_indices
                    || of.alpha_indices != rf.alpha_indices) as u8;
                println!(
                    "{pid}\t{}\t{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{ep_swapped}\t{ep_maxd}\t{pbit_diff}\t{idx_diff}\t{cat}",
                    mip_of.get(i).copied().unwrap_or(-1),
                    of.mode, rf.mode, of.partition, rf.partition,
                    of.rotation, rf.rotation, of.idx_mode, rf.idx_mode,
                );
            }
        }
    }

    if !tsv {
        println!(
            "DECODE-IDENTICAL ties: {}  (decode-divergent diffs: {})",
            g.tie_total, g.nontie
        );
        println!("unparsed: {}", g.unparsed);
        println!(
            "  EP-SWAP (clean endpoint-pair swap): {}  by #subsets-swapped {:?}",
            g.ep_swap_clean, g.ep_swap_part
        );
        println!("  same-mode endpoint diff (non-swap): {}", g.same_mode_ep);
        println!("  partition diff: {}", g.part_diff);
        println!("  rot/idxmode diff: {}", g.rot_idx_diff);
        println!("  pbit-only: {}", g.pbit_only);
        println!("  index-only: {}", g.idx_only);
        println!(
            "  mode mismatch: {} (ref-lower {} / ref-higher {})",
            g.mode_mismatch.values().sum::<usize>(),
            g.ref_lower_mode,
            g.ref_higher_mode
        );
        let mut mm: Vec<_> = g.mode_mismatch.iter().collect();
        mm.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        for ((a, b), c) in mm.iter().take(20) {
            println!("    m{a}->m{b}: {c}");
        }
    }
}
