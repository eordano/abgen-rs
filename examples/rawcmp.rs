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

fn raw_blocks(data: &[u8]) -> Option<Vec<u8>> {
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
        let dec = match bct {
            0 => blk.to_vec(),
            2 | 3 => lz4::decompress(blk, usz).ok()?,
            _ => return None,
        };
        out.extend_from_slice(&dec);
        pos += csz;
    }
    Some(out)
}

fn pairs(ref_root: &Path, ours_root: &Path) -> Vec<(PathBuf, PathBuf, String, String)> {
    let mut out = Vec::new();
    for ent in std::fs::read_dir(ref_root).unwrap().flatten() {
        if !ent.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let en = ent.file_name().to_string_lossy().into_owned();
        for f in std::fs::read_dir(ent.path()).unwrap().flatten() {
            if !f.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let fname = f.file_name().to_string_lossy().into_owned();
            let ours = ours_root.join(&en).join(&fname);
            if ours.exists() {
                out.push((f.path(), ours, en.clone(), fname));
            }
        }
    }
    out
}

fn main() {
    let a = std::env::args().nth(1).expect("ours-dir");
    let b = std::env::args().nth(2).expect("ref-dir");
    let ps = pairs(Path::new(&b), Path::new(&a));
    let n_id = AtomicU64::new(0);
    let n_none = AtomicU64::new(0);
    let lines: Vec<String> = ps
        .par_iter()
        .filter_map(|(refp, oursp, ent, bun)| {
            let rd = std::fs::read(refp).ok()?;
            let od = std::fs::read(oursp).ok()?;
            if rd == od {
                n_id.fetch_add(1, Ordering::Relaxed);
                return None;
            }
            let rr = match raw_blocks(&rd) {
                Some(v) => v,
                None => {
                    n_none.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            };
            let ro = match raw_blocks(&od) {
                Some(v) => v,
                None => {
                    n_none.fetch_add(1, Ordering::Relaxed);
                    return None;
                }
            };
            let raw_equal = rr == ro;
            Some(format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                ent,
                bun,
                od.len() as i64 - rd.len() as i64,
                ro.len() as i64 - rr.len() as i64,
                ro.len(),
                rr.len(),
                raw_equal
            ))
        })
        .collect();
    eprintln!(
        "pairs={} byte_id_skipped={} parse_none={} emitted={}",
        ps.len(),
        n_id.load(Ordering::Relaxed),
        n_none.load(Ordering::Relaxed),
        lines.len()
    );
    println!("entity\tbundle\tcomp_delta\traw_delta\traw_ours\traw_ref\traw_equal");
    for l in lines {
        println!("{l}");
    }
}
