use abgen::unity::bundle_file::{Bundle, FileContent};
use abgen::value::Value;

fn gi(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn payloads(path: &str) -> Vec<(String, i64, i64, i64, Vec<u8>)> {
    let b = Bundle::load(std::path::Path::new(path)).unwrap();
    let mut ress: Vec<(String, Vec<u8>)> = Vec::new();
    for f in &b.files {
        if let FileContent::Raw(d) = &f.content {
            ress.push((f.name.clone(), d.clone()));
        }
    }
    let mut out = Vec::new();
    for f in &b.files {
        let FileContent::Serialized(sf) = &f.content else {
            continue;
        };
        for o in &sf.objects {
            if o.class_id != 28 {
                continue;
            }
            let Ok(v) = sf.read_typetree(o) else { continue };
            if gi(&v, "m_TextureFormat") != 25 {
                continue;
            }
            let name = v
                .get("m_Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let (w, h, mips) = (gi(&v, "m_Width"), gi(&v, "m_Height"), gi(&v, "m_MipCount"));
            let inline = v.get("image data").and_then(|x| match x {
                Value::Bytes(b) if !b.is_empty() => Some(b.clone()),
                _ => None,
            });
            let payload = if let Some(d) = inline {
                d
            } else {
                let sd = v.get("m_StreamData").unwrap();
                let off = gi(sd, "offset") as usize;
                let size = gi(sd, "size") as usize;
                let p = sd.get("path").and_then(|x| x.as_str()).unwrap_or("");
                let base = p.rsplit('/').next().unwrap_or(p);
                let Some((_, rd)) = ress.iter().find(|(n, _)| n == base) else {
                    continue;
                };
                rd[off..off + size].to_vec()
            };
            out.push((name, w, h, mips, payload));
        }
    }
    out
}

fn mip_offset(w: i64, h: i64, m: i64) -> usize {
    let (mut mw, mut mh) = (w, h);
    let mut off = 0usize;
    for _ in 0..m {
        let bw = ((mw + 3) / 4).max(1) as usize;
        let bh = ((mh + 3) / 4).max(1) as usize;
        off += bw * bh * 16;
        mw = (mw / 2).max(1);
        mh = (mh / 2).max(1);
    }
    off
}

fn grid(block: &[u8]) -> String {
    let mut img = vec![0u32; 16];
    texture2ddecoder::decode_bc7(block, 4, 4, &mut img).unwrap();
    let mut s = String::new();
    for y in 0..4 {
        for x in 0..4 {
            let p = img[y * 4 + x];
            let b = (p & 0xff) as u8;
            let g = ((p >> 8) & 0xff) as u8;
            let r = ((p >> 16) & 0xff) as u8;
            let a = ((p >> 24) & 0xff) as u8;
            s += &format!("({r:>3},{g:>3},{b:>3},{a:>3}) ");
        }
        s += "\n";
    }
    s
}

fn main() {
    let list = std::env::args().nth(1).expect("pairs file");
    let max: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let mut shown = 0;
    for line in std::fs::read_to_string(&list).unwrap().lines() {
        if shown >= max {
            break;
        }
        let mut it = line.split('\t');
        let (op, rp) = (it.next().unwrap(), it.next().unwrap());
        let ours = payloads(op);
        let theirs = payloads(rp);
        for (name, w, h, mips, opay) in &ours {
            let Some((_, _, _, _, rpay)) = theirs
                .iter()
                .find(|(n, tw, th, tm, _)| n == name && tw == w && th == h && tm == mips)
            else {
                continue;
            };
            if opay.len() != rpay.len() {
                continue;
            }

            let m22 = (0..*mips).find(|&m| (w >> m).max(1) == 2 && (h >> m).max(1) == 2);
            let Some(m22) = m22 else { continue };
            let off = mip_offset(*w, *h, m22);
            if off + 16 > opay.len() {
                continue;
            }
            let ob = &opay[off..off + 16];
            let rb = &rpay[off..off + 16];
            if ob == rb {
                continue;
            }

            let rest_same = opay[..off] == rpay[..off] && opay[off + 16..] == rpay[off + 16..];
            println!(
                "== {} {}x{} mips={} 2x2-mip={} rest_same={} ({})",
                name, w, h, mips, m22, rest_same, op
            );
            println!("-- ours block {:02x?}", ob);
            print!("{}", grid(ob));
            println!("-- ref  block {:02x?}", rb);
            print!("{}", grid(rb));
            shown += 1;
        }
    }
}
