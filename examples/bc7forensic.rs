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
    let (ns, pb, rb, imb, cb, ab, npb, _shared, ib1, ib2) = match mode {
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

fn extract(bundle: &Bundle) -> BTreeMap<i64, (String, usize, usize, i64, Vec<u8>)> {
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

fn png_dims(path: &str) -> Option<(u32, u32)> {
    let d = std::fs::read(path).ok()?;
    if d.len() < 24 || &d[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let w = u32::from_be_bytes([d[16], d[17], d[18], d[19]]);
    let h = u32::from_be_bytes([d[20], d[21], d[22], d[23]]);
    Some((w, h))
}

fn sha1_hex_prefix4(s: &str) -> String {
    use sha1::{Digest, Sha1};
    let dg = Sha1::digest(s.as_bytes());
    let mut out = String::with_capacity(8);
    for b in &dg[..2] {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[derive(Default)]
struct Tally {
    pairs_seen: usize,
    pairs_resize: usize,
    pairs_noresize: usize,
    blk_diff: usize,
    mode_mismatch: BTreeMap<(u8, u8), usize>,
    same_mode: BTreeMap<u8, usize>,
    part_diff: usize,
    rot_diff: usize,
    ep_diff: usize,
    ep_lsb: usize,
    ep_by_mode: BTreeMap<u8, (usize, usize, usize, usize)>,
    pbit_only: usize,
    idx_only: usize,
    unparsed: usize,
    pairs_diff_mip0_only: usize,
    pairs_diff_with_deeper: usize,
    pairs_byte_id_tex: usize,
}

fn main() {
    let listf = std::env::args().nth(1).expect("pairs list file");
    let lines: Vec<String> = std::fs::read_to_string(&listf)
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect();
    let root = std::env::var("ABGEN_CONTENT_ROOT").unwrap();

    let mut t = Tally::default();
    for line in &lines {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let (ours_p, ref_p, cid) = (parts[0], parts[1], parts[2]);
        let Ok(ours) = Bundle::load(std::path::Path::new(ours_p)) else {
            continue;
        };
        let Ok(refb) = Bundle::load(std::path::Path::new(ref_p)) else {
            continue;
        };
        let op = extract(&ours);
        let rp = extract(&refb);
        t.pairs_seen += 1;

        let hex = sha1_hex_prefix4(cid);
        let src = format!("{}/{}/{}", root, hex, cid);
        let src_dims = png_dims(&src);

        let mut any_tex = false;
        let mut tex_byte_id = true;
        let mut diff_mip0 = false;
        let mut diff_deeper = false;
        let mut resized = false;
        for (pid, (_name, w, h, mips, opay)) in &op {
            let Some((_rn, rw, rh, _rm, rpay)) = rp.get(pid) else {
                continue;
            };
            if w != rw || h != rh || opay.len() != rpay.len() {
                continue;
            }
            any_tex = true;
            let no_resize = src_dims == Some((*w as u32, *h as u32));
            if src_dims.is_some() && !no_resize {
                resized = true;
            }
            let bw0 = w.div_ceil(4).max(1);
            let bh0 = h.div_ceil(4).max(1);
            let mip0_blocks = bw0 * bh0;
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
                tex_byte_id = false;
                let m = mip_of.get(i).copied().unwrap_or(-1);
                if m == 0 {
                    diff_mip0 = true;
                } else {
                    diff_deeper = true;
                }
                if !(no_resize && i < mip0_blocks) {
                    continue;
                }
                t.blk_diff += 1;
                let (Some(of), Some(rf)) = (parse_block(ob), parse_block(rb)) else {
                    t.unparsed += 1;
                    continue;
                };
                if of.mode != rf.mode {
                    *t.mode_mismatch.entry((of.mode, rf.mode)).or_default() += 1;
                } else {
                    *t.same_mode.entry(of.mode).or_default() += 1;
                    if of.partition != rf.partition {
                        t.part_diff += 1;
                    } else if of.rotation != rf.rotation || of.idx_mode != rf.idx_mode {
                        t.rot_diff += 1;
                    } else if of.endpoints != rf.endpoints {
                        t.ep_diff += 1;
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
                            t.ep_lsb += 1;
                        }
                        let idx_also = of.color_indices != rf.color_indices
                            || of.alpha_indices != rf.alpha_indices;
                        let pbit_also = of.pbits != rf.pbits;
                        let e = t.ep_by_mode.entry(of.mode).or_default();
                        e.0 += 1;
                        if lsb {
                            e.1 += 1;
                        }
                        if idx_also {
                            e.2 += 1;
                        }
                        if pbit_also {
                            e.3 += 1;
                        }
                    } else if of.pbits != rf.pbits {
                        t.pbit_only += 1;
                    } else {
                        t.idx_only += 1;
                    }
                }
            }
        }
        if any_tex {
            if std::env::var_os("BC7F_PERPAIR").is_some() {
                let texcid = ref_p
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .trim_end_matches("_windows");
                eprintln!(
                    "PERPAIR\t{}\t{}\t{}",
                    texcid,
                    if resized { 0 } else { 1 },
                    if tex_byte_id { 1 } else { 0 }
                );
            }
            if resized {
                t.pairs_resize += 1;
            } else {
                t.pairs_noresize += 1;
            }
            if tex_byte_id {
                t.pairs_byte_id_tex += 1;
            } else if diff_mip0 && !diff_deeper {
                t.pairs_diff_mip0_only += 1;
            } else if diff_deeper {
                t.pairs_diff_with_deeper += 1;
            }
        }
    }

    println!("=== BC7 FORENSIC TAXONOMY (mip-0, no-resize PNG subset) ===");
    println!("pairs seen           {}", t.pairs_seen);
    println!("pairs no-resize      {}", t.pairs_noresize);
    println!("pairs resize         {}", t.pairs_resize);
    println!("pairs tex byte-id    {}", t.pairs_byte_id_tex);
    println!("pairs diff mip0-only {}", t.pairs_diff_mip0_only);
    println!("pairs diff w/ deeper {}", t.pairs_diff_with_deeper);
    println!();
    println!("mip0 no-resize differing blocks: {}", t.blk_diff);
    println!("  unparsed {}", t.unparsed);
    let mut mm: Vec<_> = t.mode_mismatch.iter().collect();
    mm.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    let mode_total: usize = t.mode_mismatch.values().sum();
    println!("  MODE mismatch (different mode chosen): {}", mode_total);
    for ((a, b), c) in mm.iter().take(20) {
        println!("    ours m{a} -> ref m{b}: {c}");
    }
    println!("  same-mode by mode: {:?}", t.same_mode);
    println!("  PARTITION diff (same mode): {}", t.part_diff);
    println!("  ROT/IDXMODE diff (same mode): {}", t.rot_diff);
    println!(
        "  ENDPOINT diff (same mode/part): {} (lsb<=1: {})",
        t.ep_diff, t.ep_lsb
    );
    println!(
        "    by mode (ep, lsb, idx-also, pbit-also): {:?}",
        t.ep_by_mode
    );
    println!("  PBIT-only diff: {}", t.pbit_only);
    println!("  INDEX-only diff: {}", t.idx_only);
}
