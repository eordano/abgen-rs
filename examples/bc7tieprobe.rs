use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;
use std::collections::BTreeMap;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn extract(bundle: &Bundle) -> BTreeMap<i64, (usize, usize, i64, Vec<u8>)> {
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
            let w = gi(&v, "m_Width") as usize;
            let h = gi(&v, "m_Height") as usize;
            let mips = gi(&v, "m_MipCount");
            let inline = v.get("image data").and_then(|x| match x {
                Value::Bytes(b) if !b.is_empty() => Some(b.clone()),
                _ => None,
            });
            let payload = if let Some(d) = inline {
                d
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
            out.insert(obj.path_id, (w, h, mips, payload));
        }
    }
    out
}

fn dec(b: &[u8; 16]) -> [u32; 16] {
    let mut o = [0u32; 16];
    texture2ddecoder::decode_bc7(b, 4, 4, &mut o).ok();
    o
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let ob = Bundle::load(std::path::Path::new(&a[0])).unwrap();
    let rb = Bundle::load(std::path::Path::new(&a[1])).unwrap();
    let pid: i64 = a[2].parse().unwrap();
    let blk: usize = a[3].parse().unwrap();
    let op = extract(&ob);
    let rp = extract(&rb);
    let (_, _, _, opay) = &op[&pid];
    let (_, _, _, rpay) = &rp[&pid];
    let o: &[u8; 16] = opay[blk * 16..blk * 16 + 16].try_into().unwrap();
    let r: &[u8; 16] = rpay[blk * 16..blk * 16 + 16].try_into().unwrap();
    let od = dec(o);
    let rd = dec(r);
    println!("ours bytes: {:02x?}", o);
    println!("ref  bytes: {:02x?}", r);
    println!(
        "ours mode {}  ref mode {}",
        o[0].trailing_zeros(),
        r[0].trailing_zeros()
    );
    println!("decode-identical: {}", od == rd);

    println!("decoded pixels (u32 ARGB):");
    for row in 0..4 {
        let r0: Vec<String> = (0..4).map(|c| format!("{:08x}", od[row * 4 + c])).collect();
        println!("  {}", r0.join(" "));
    }

    let mut set: Vec<u32> = od.to_vec();
    set.sort();
    set.dedup();
    println!("distinct decoded colors: {}", set.len());

    let mut rgba = [0u8; 64];
    for (i, p) in rd.iter().enumerate() {
        let [b, g, r, a] = p.to_le_bytes();
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = a;
    }
    let params = abgen::bc7_pure::Params::basic(false);
    let enc = abgen::bc7_pure::encode_blocks(&rgba, 1, &params);
    let e: &[u8; 16] = enc[..16].try_into().unwrap();
    println!("re-encoded bytes: {:02x?}", e);
    println!("re-encoded mode {}", e[0].trailing_zeros());
    println!(
        "re-encode == ours: {}   re-encode == ref: {}",
        e == o,
        e == r
    );
}
