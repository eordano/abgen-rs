use abgen::lz4;
use rayon::prelude::*;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn read_cstring(cur: &mut Cursor<&Vec<u8>>) -> String {
    let mut out = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        if cur.read_exact(&mut buf).is_err() {
            break;
        }
        if buf[0] == 0 {
            break;
        }
        out.push(buf[0]);
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn lz4_blocks(data: &[u8]) -> Option<Vec<(Vec<u8>, Vec<u8>)>> {
    let owned = data.to_vec();
    let mut cur = Cursor::new(&owned);
    let mut sig = [0u8; 8];
    cur.read_exact(&mut sig).ok()?;
    if &sig != b"UnityFS\0" {
        return None;
    }
    let mut fmt = [0u8; 4];
    cur.read_exact(&mut fmt).ok()?;
    let fmtv = u32::from_be_bytes(fmt);
    let _vp = read_cstring(&mut cur);
    let _ve = read_cstring(&mut cur);
    let mut b8 = [0u8; 8];
    cur.read_exact(&mut b8).ok()?;
    let mut b4 = [0u8; 4];
    cur.read_exact(&mut b4).ok()?;
    let comp_bi = u32::from_be_bytes(b4) as usize;
    cur.read_exact(&mut b4).ok()?;
    let uncomp_bi = u32::from_be_bytes(b4) as usize;
    cur.read_exact(&mut b4).ok()?;
    let flags = u32::from_be_bytes(b4);
    if fmtv >= 7 {
        let p = cur.position() as usize;
        let pad = (16 - (p % 16)) % 16;
        cur.set_position((p + pad) as u64);
    }
    let header_end = cur.position() as usize;
    let bi_off = if flags & 0x80 != 0 {
        data.len() - comp_bi
    } else {
        header_end
    };
    let bi_comp = &data[bi_off..bi_off + comp_bi];
    let ct = flags & 0x3f;
    let bi: Vec<u8> = match ct {
        0 => bi_comp.to_vec(),
        2 | 3 => lz4::decompress(bi_comp, uncomp_bi).ok()?,
        _ => return None,
    };
    let mut br = Cursor::new(&bi);
    let mut h = [0u8; 16];
    br.read_exact(&mut h).ok()?;
    let mut bc = [0u8; 4];
    br.read_exact(&mut bc).ok()?;
    let blocks_count = u32::from_be_bytes(bc);
    let mut sizes = Vec::new();
    for _ in 0..blocks_count {
        let mut u = [0u8; 4];
        br.read_exact(&mut u).ok()?;
        let usz = u32::from_be_bytes(u) as usize;
        let mut c = [0u8; 4];
        br.read_exact(&mut c).ok()?;
        let csz = u32::from_be_bytes(c) as usize;
        let mut fl = [0u8; 2];
        br.read_exact(&mut fl).ok()?;
        let bflags = u16::from_be_bytes(fl) as u32;
        sizes.push((usz, csz, bflags));
    }
    let mut data_start = if flags & 0x80 != 0 {
        header_end
    } else {
        bi_off + comp_bi
    };
    if flags & 0x200 != 0 {
        data_start = (data_start + 15) & !15;
    }
    let mut pos = data_start;
    let mut out = Vec::new();
    for (usz, csz, bflags) in sizes {
        if pos + csz > data.len() {
            return None;
        }
        let blk = &data[pos..pos + csz];
        let bct = bflags & 0x3f;
        if bct == 2 || bct == 3 {
            let dec = lz4::decompress(blk, usz).ok()?;
            out.push((blk.to_vec(), dec));
        }
        pos += csz;
    }
    Some(out)
}

fn all_bundles(ref_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for ent in std::fs::read_dir(ref_root).unwrap().flatten() {
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        for f in std::fs::read_dir(ent.path()).unwrap().flatten() {
            if f.file_type().map(|t| t.is_file()).unwrap_or(false) {
                out.push(f.path());
            }
        }
    }
    out
}

fn main() {
    let refdir = std::env::args().nth(1).expect("ref-dir");
    let maxb: usize = std::env::args()
        .nth(2)
        .map(|s| s.parse().unwrap())
        .unwrap_or(50);
    let mut bundles = all_bundles(Path::new(&refdir));
    bundles.sort();
    bundles.truncate(maxb);

    let blocks_total = AtomicU64::new(0);
    let blocks_match = AtomicU64::new(0);
    let blocks_mismatch = AtomicU64::new(0);

    let mismatches: Vec<String> = bundles
        .par_iter()
        .flat_map_iter(|p| {
            let mut local = Vec::new();
            let data = match std::fs::read(p) {
                Ok(d) => d,
                Err(_) => return local.into_iter(),
            };
            if let Some(blks) = lz4_blocks(&data) {
                for (i, (ref_comp, raw)) in blks.iter().enumerate() {
                    blocks_total.fetch_add(1, Ordering::Relaxed);
                    let ours = lz4::compress_hc(raw);
                    if &ours == ref_comp {
                        blocks_match.fetch_add(1, Ordering::Relaxed);
                    } else {
                        blocks_mismatch.fetch_add(1, Ordering::Relaxed);
                        local.push(format!(
                            "{} block#{} raw_len={} ref_comp_len={} our_comp_len={}",
                            p.display(),
                            i,
                            raw.len(),
                            ref_comp.len(),
                            ours.len()
                        ));
                    }
                }
            }
            local.into_iter()
        })
        .collect();

    println!(
        "bundles={} lz4_blocks={} match={} mismatch={}",
        bundles.len(),
        blocks_total.load(Ordering::Relaxed),
        blocks_match.load(Ordering::Relaxed),
        blocks_mismatch.load(Ordering::Relaxed)
    );
    for m in mismatches.iter().take(40) {
        println!("MISMATCH {m}");
    }
}
