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

    let (ns, pb, rb, imb, cb, ab, npb, shared, ib1, ib2) = match mode {
        0 => (3, 4, 0, 0, 4, 0, 6, false, 3, 0),
        1 => (2, 6, 0, 0, 6, 0, 2, true, 3, 0),
        2 => (3, 6, 0, 0, 5, 0, 0, false, 2, 0),
        3 => (2, 6, 0, 0, 7, 0, 4, false, 2, 0),
        4 => (1, 0, 2, 1, 5, 6, 0, false, 2, 3),
        5 => (1, 0, 2, 0, 7, 8, 0, false, 2, 2),
        6 => (1, 0, 0, 0, 7, 7, 2, false, 4, 0),
        7 => (2, 6, 0, 0, 5, 5, 4, false, 2, 0),
        _ => unreachable!(),
    };
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
    if shared {
        for _ in 0..npb {
            f.pbits.push(r.read(1) as u8);
        }
    } else {
        for _ in 0..npb {
            f.pbits.push(r.read(1) as u8);
        }
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
    let subset_of = |pix: usize| -> usize {
        if ns == 1 {
            0
        } else if ns == 2 {
            (bc7_p2(f.partition as usize) >> pix & 1) as usize
        } else {
            bc7_p3(f.partition as usize, pix)
        }
    };
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
    let _ = subset_of;

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

fn bc7_p2(p: usize) -> u16 {
    const T: [u16; 64] = [
        0xCCCC, 0x8888, 0xEEEE, 0xECC8, 0xC880, 0xFEEC, 0xFEC8, 0xEC80, 0xC800, 0xFFEC, 0xFE80,
        0xE800, 0xFFE8, 0xFF00, 0xFFF0, 0xF000, 0xF710, 0x008E, 0x7100, 0x08CE, 0x008C, 0x7310,
        0x3100, 0x8CCE, 0x088C, 0x3110, 0x6666, 0x366C, 0x17E8, 0x0FF0, 0x718E, 0x399C, 0xaaaa,
        0xf0f0, 0x5a5a, 0x33cc, 0x3c3c, 0x55aa, 0x9696, 0xa55a, 0x73ce, 0x13c8, 0x324c, 0x3bdc,
        0x6996, 0xc33c, 0x9966, 0x0660, 0x0272, 0x04e4, 0x4e40, 0x2720, 0xc936, 0x936c, 0x39c6,
        0x639c, 0x9336, 0x9cc6, 0x817e, 0xe718, 0xccf0, 0x0fcc, 0x7744, 0xee22,
    ];
    T[p]
}
fn bc7_p3(_p: usize, _pix: usize) -> usize {
    0
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
            let fmt = gi(&v, "m_TextureFormat");
            if fmt != 25 {
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let print_blocks = args.iter().any(|a| a == "--blocks");
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let ours = Bundle::load(std::path::Path::new(paths[0])).unwrap();
    let refb = Bundle::load(std::path::Path::new(paths[1])).unwrap();
    let op = extract_bc7_payloads(&ours);
    let rp = extract_bc7_payloads(&refb);

    let mut g_total = 0usize;
    let mut g_ident = 0usize;
    let mut g_mode_mismatch: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let mut g_part = 0usize;
    let mut g_rot = 0usize;
    let mut g_ep = 0usize;
    let mut g_ep_lsb = 0usize;
    let mut g_pbit_only = 0usize;
    let mut g_idx_only = 0usize;
    let mut g_unparsed = 0usize;
    let mut g_ep_by_mode: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
    let mut g_same_mode_by_mode: BTreeMap<u8, usize> = BTreeMap::new();
    let mut g_diff_alpha = 0usize;

    for (pid, (name, w, h, mips, opay)) in &op {
        let Some((rname, rw, rh, rmips, rpay)) = rp.get(pid) else {
            println!("pid={pid} {name}: missing in ref");
            continue;
        };
        if w != rw || h != rh || mips != rmips || opay.len() != rpay.len() {
            println!(
                "pid={pid} {name}/{rname}: shape mismatch {w}x{h}m{mips} len {} vs {rw}x{rh}m{rmips} len {}",
                opay.len(),
                rpay.len()
            );
            continue;
        }
        let nb = opay.len() / 16;
        let mut t_ident = 0usize;
        let mut t_diff = 0usize;

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
        let mut mip_diffs: BTreeMap<i64, usize> = BTreeMap::new();
        for i in 0..nb {
            let ob: &[u8; 16] = opay[i * 16..i * 16 + 16].try_into().unwrap();
            let rb: &[u8; 16] = rpay[i * 16..i * 16 + 16].try_into().unwrap();
            g_total += 1;
            if ob == rb {
                g_ident += 1;
                t_ident += 1;
                continue;
            }
            t_diff += 1;
            *mip_diffs
                .entry(mip_of.get(i).copied().unwrap_or(-1))
                .or_default() += 1;
            let (of, rf) = (parse_block(ob), parse_block(rb));
            let (Some(of), Some(rf)) = (of, rf) else {
                g_unparsed += 1;
                continue;
            };

            let ref_has_alpha = match rf.mode {
                4 | 5 => true,
                6 => rf.endpoints.iter().any(|e| e[3] != 127) || rf.pbits.iter().any(|p| *p == 0),
                7 => true,
                _ => false,
            };
            if ref_has_alpha {
                g_diff_alpha += 1;
            }
            let cat: String;
            if of.mode != rf.mode {
                *g_mode_mismatch.entry((of.mode, rf.mode)).or_default() += 1;
                cat = format!("mode {}->{}", of.mode, rf.mode);
            } else {
                *g_same_mode_by_mode.entry(of.mode).or_default() += 1;
                if of.partition != rf.partition {
                    g_part += 1;
                    cat = format!("m{} part {}->{}", of.mode, of.partition, rf.partition);
                } else if of.rotation != rf.rotation || of.idx_mode != rf.idx_mode {
                    g_rot += 1;
                    cat = format!("m{} rot/idxmode", of.mode);
                } else if of.endpoints != rf.endpoints {
                    g_ep += 1;
                    let max_d = of
                        .endpoints
                        .iter()
                        .zip(rf.endpoints.iter())
                        .flat_map(|(a, b)| {
                            a.iter()
                                .zip(b.iter())
                                .map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs())
                        })
                        .max()
                        .unwrap_or(0);
                    let lsb = max_d <= 1;
                    if lsb {
                        g_ep_lsb += 1;
                    }
                    let e = g_ep_by_mode.entry(of.mode).or_default();
                    e.0 += 1;
                    if lsb {
                        e.1 += 1;
                    }
                    cat = format!(
                        "m{} ep maxd={} pb {} idx {}",
                        of.mode,
                        max_d,
                        if of.pbits != rf.pbits { "DIFF" } else { "same" },
                        if of.color_indices != rf.color_indices
                            || of.alpha_indices != rf.alpha_indices
                        {
                            "DIFF"
                        } else {
                            "same"
                        }
                    );
                } else if of.pbits != rf.pbits {
                    g_pbit_only += 1;
                    cat = format!("m{} pbit-only", of.mode);
                } else {
                    g_idx_only += 1;
                    cat = format!(
                        "m{} idx-only c{} a{}",
                        of.mode,
                        if of.color_indices != rf.color_indices {
                            "DIFF"
                        } else {
                            "="
                        },
                        if of.alpha_indices != rf.alpha_indices {
                            "DIFF"
                        } else {
                            "="
                        }
                    );
                }
            }
            if print_blocks {
                println!(
                    "  pid={pid} mip={} blk={} {}",
                    mip_of.get(i).copied().unwrap_or(-1),
                    i,
                    cat
                );
            }
        }
        let mipstr: Vec<String> = mip_diffs.iter().map(|(m, c)| format!("m{m}:{c}")).collect();
        println!(
            "pid={pid} {name} {w}x{h} mips={mips} blocks={} ident={} diff={} [{}]",
            nb,
            t_ident,
            t_diff,
            mipstr.join(" ")
        );
    }

    println!("\n=== GLOBAL ===");
    println!(
        "total blocks {g_total} identical {g_ident} differing {}",
        g_total - g_ident
    );
    println!("unparsed {g_unparsed}");
    println!("mode mismatches:");
    let mut mm: Vec<_> = g_mode_mismatch.iter().collect();
    mm.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for ((a, b), c) in mm {
        println!("  ours m{a} -> ref m{b}: {c}");
    }
    println!("same-mode diffs by mode: {:?}", g_same_mode_by_mode);
    println!("  partition diff {g_part}  rot/idxmode diff {g_rot}");
    println!(
        "  endpoint diff {g_ep} (lsb-only {g_ep_lsb})  by mode {:?}",
        g_ep_by_mode
    );
    println!("  pbit-only {g_pbit_only}  index-only {g_idx_only}");
    println!("differing blocks with ref-side alpha info: {g_diff_alpha}");
}
