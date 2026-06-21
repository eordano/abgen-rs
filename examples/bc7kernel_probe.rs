use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::{HashMap, HashSet};
use std::io::Read;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

type Entry = (usize, usize, i64, Vec<u8>);

fn extract(bundle: &Bundle) -> Vec<Entry> {
    let mut ress: Vec<(String, &Vec<u8>)> = Vec::new();
    for f in &bundle.files {
        if let FileContent::Raw(data) = &f.content {
            ress.push((f.name.clone(), data));
        }
    }
    let mut out = Vec::new();
    let mut entries: Vec<(i64, Entry)> = Vec::new();
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
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let mips = gi(&v, "m_MipCount");
            let inline: Option<&[u8]> = v.get("image data").and_then(|x| match x {
                Value::Bytes(b) if !b.is_empty() => Some(b.as_slice()),
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
            entries.push((obj.path_id, (w, h, mips, payload)));
        }
    }
    entries.sort_by_key(|(pid, _)| *pid);
    for (_, e) in entries {
        out.push(e);
    }
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let cap_path = args.next().expect("capture file");
    let pairs_path = args.next().expect("pairs tsv");
    let out_path = args.next().expect("output probe file");

    struct Need {
        rb: [u8; 16],
        is_diff: bool,
    }

    let mut positions: Vec<([u8; 16], Need)> = Vec::new();
    let mut wanted: HashSet<[u8; 16]> = HashSet::new();

    let lines: Vec<String> = std::fs::read_to_string(&pairs_path)
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect();
    for line in &lines {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() < 3 {
            continue;
        }
        if !p[2].starts_with("standalone-texture") {
            continue;
        }
        let (ours_p, ref_p) = (p[0], p[1]);
        let Ok(ours) = Bundle::load(std::path::Path::new(ours_p)) else {
            continue;
        };
        let Ok(refb) = Bundle::load(std::path::Path::new(ref_p)) else {
            continue;
        };
        let op = extract(&ours);
        let rp = extract(&refb);
        if op.len() != rp.len() {
            continue;
        }
        for (ti, (w, h, _mips, opay)) in op.iter().enumerate() {
            let (rw, rh, _rm, rpay) = &rp[ti];
            if w != rw || h != rh || opay.len() != rpay.len() {
                continue;
            }
            let bw0 = w.div_ceil(4).max(1);
            let bh0 = h.div_ceil(4).max(1);
            let mip0_blocks = bw0 * bh0;
            for i in 0..mip0_blocks {
                if i * 16 + 16 > opay.len() {
                    break;
                }
                let mut ob = [0u8; 16];
                ob.copy_from_slice(&opay[i * 16..i * 16 + 16]);
                let mut rb = [0u8; 16];
                rb.copy_from_slice(&rpay[i * 16..i * 16 + 16]);
                let is_diff = ob != rb;
                positions.push((ob, Need { rb, is_diff }));
                wanted.insert(ob);
            }
        }
    }
    eprintln!(
        "pass1: {} mip0 positions, {} distinct ours-blocks wanted",
        positions.len(),
        wanted.len()
    );

    let mut map: HashMap<[u8; 16], [u8; 64]> = HashMap::with_capacity(wanted.len());
    {
        let f = std::fs::File::open(&cap_path).expect("open capture");
        let mut rdr = std::io::BufReader::with_capacity(1 << 24, f);
        let mut buf = [0u8; 80];
        let mut seen = 0u64;
        while let Ok(()) = rdr.read_exact(&mut buf) {
            seen += 1;
            let mut k = [0u8; 16];
            k.copy_from_slice(&buf[..16]);
            if wanted.contains(&k) && !map.contains_key(&k) {
                let mut vv = [0u8; 64];
                vv.copy_from_slice(&buf[16..80]);
                map.insert(k, vv);
            }
            if seen.is_multiple_of(50_000_000) {
                eprintln!("  scanned {seen}M*1e-6 ... map {}", map.len());
            }
        }
        eprintln!(
            "pass2: scanned {seen} records, recovered {} inputs",
            map.len()
        );
    }

    let sample: Option<usize> = std::env::var("ABGEN_PROBE_SAMPLE")
        .ok()
        .and_then(|s| s.parse().ok());
    let tot_diff = positions.iter().filter(|(_, n)| n.is_diff).count() as u64;
    let tot_match = positions.len() as u64 - tot_diff;
    let stride_diff = sample
        .map(|n| (tot_diff as usize / n.max(1)).max(1))
        .unwrap_or(1);
    let stride_match = sample
        .map(|n| (tot_match as usize / n.max(1)).max(1))
        .unwrap_or(1);

    let mut probe: Vec<u8> = Vec::new();
    let mut nrec: u32 = 0;
    let (mut nrec_diff, mut nrec_match, mut unmapped) = (0u64, 0u64, 0u64);
    let (mut di, mut mi) = (0usize, 0usize);
    for (ob, need) in &positions {
        let keep = if need.is_diff {
            let k = di % stride_diff == 0;
            di += 1;
            k
        } else {
            let k = mi % stride_match == 0;
            mi += 1;
            k
        };
        if !keep {
            continue;
        }
        let Some(input) = map.get(ob) else {
            unmapped += 1;
            continue;
        };
        if need.is_diff {
            nrec_diff += 1;
        } else {
            nrec_match += 1;
        }
        probe.extend_from_slice(input);
        probe.extend_from_slice(ob);
        probe.extend_from_slice(&need.rb);
        probe.push(if need.is_diff { 1 } else { 0 });
        probe.extend_from_slice(&[0u8; 3]);
        nrec += 1;
    }

    let mut f = std::fs::File::create(&out_path).expect("create probe");
    use std::io::Write;
    f.write_all(&nrec.to_le_bytes()).unwrap();
    f.write_all(&probe).unwrap();
    eprintln!(
        "probe: {nrec} blocks ({nrec_diff} diff, {nrec_match} match, {unmapped} unmapped-skipped) -> {out_path}"
    );
}
