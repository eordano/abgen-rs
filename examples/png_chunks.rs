use abgen::local_store::LocalContentStore;
use std::collections::BTreeMap;

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

struct Chunks {
    gama: Option<u32>,
    srgb: Option<u8>,
    iccp_name: Option<String>,
    iccp_len: Option<usize>,
    chrm: bool,
    color_type: u8,
    bit_depth: u8,
    w: u32,
    h: u32,
}

fn parse_png_chunks(data: &[u8]) -> Option<Chunks> {
    if data.len() < 8 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let mut pos = 8usize;
    let mut c = Chunks {
        gama: None,
        srgb: None,
        iccp_name: None,
        iccp_len: None,
        chrm: false,
        color_type: 0,
        bit_depth: 0,
        w: 0,
        h: 0,
    };
    while pos + 8 <= data.len() {
        let len = be32(&data[pos..pos + 4]) as usize;
        let typ = &data[pos + 4..pos + 8];
        let dstart = pos + 8;
        let dend = dstart + len;
        if dend + 4 > data.len() {
            break;
        }
        let body = &data[dstart..dend];
        match typ {
            b"IHDR" => {
                if body.len() >= 13 {
                    c.w = be32(&body[0..4]);
                    c.h = be32(&body[4..8]);
                    c.bit_depth = body[8];
                    c.color_type = body[9];
                }
            }
            b"gAMA" => {
                if body.len() >= 4 {
                    c.gama = Some(be32(&body[0..4]));
                }
            }
            b"sRGB" => {
                c.srgb = Some(body.first().copied().unwrap_or(0));
            }
            b"iCCP" => {
                let nul = body.iter().position(|&b| b == 0).unwrap_or(0);
                c.iccp_name = Some(String::from_utf8_lossy(&body[..nul]).to_string());
                c.iccp_len = Some(body.len().saturating_sub(nul + 2));
            }
            b"cHRM" => c.chrm = true,
            b"IEND" => break,
            _ => {}
        }
        pos = dend + 4;
    }
    Some(c)
}

fn main() {
    let pairs = std::env::args().nth(1).expect("pairs.tsv");
    let root = std::env::args()
        .nth(2)
        .or_else(|| std::env::var("ABGEN_CONTENT_ROOT").ok())
        .expect("content root (arg or ABGEN_CONTENT_ROOT)");
    let store = LocalContentStore::new(root);
    let text = std::fs::read_to_string(&pairs).expect("read pairs");

    let mut n_total = 0usize;
    let mut n_png = 0usize;
    let mut n_gama_nontrivial = 0usize;
    let mut n_gama_trivial = 0usize;
    let mut n_srgb = 0usize;
    let mut n_iccp = 0usize;
    let mut n_chrm = 0usize;
    let mut iccp_names: BTreeMap<String, usize> = BTreeMap::new();
    let mut gama_vals: BTreeMap<u32, usize> = BTreeMap::new();

    for line in text.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        let kind = cols[2];
        if !kind.starts_with("standalone-texture") {
            continue;
        }
        n_total += 1;

        let ours = cols[0];
        let base = ours.rsplit('/').next().unwrap_or(ours);
        let cid = base
            .strip_suffix("_windows")
            .or_else(|| base.strip_suffix("_mac"))
            .unwrap_or(base);
        let data = match store.fetch(cid) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let Some(c) = parse_png_chunks(&data) else {
            continue;
        };
        n_png += 1;
        let mut tags = Vec::new();
        if let Some(g) = c.gama {
            *gama_vals.entry(g).or_default() += 1;

            if g == 45455 {
                n_gama_trivial += 1;
                tags.push(format!("gAMA={g}(trivial)"));
            } else {
                n_gama_nontrivial += 1;
                tags.push(format!("gAMA={g}(NONTRIVIAL)"));
            }
        }
        if let Some(s) = c.srgb {
            n_srgb += 1;
            tags.push(format!("sRGB={s}"));
        }
        if let Some(name) = &c.iccp_name {
            n_iccp += 1;
            *iccp_names.entry(name.clone()).or_default() += 1;
            tags.push(format!("iCCP='{name}'({}B)", c.iccp_len.unwrap_or(0)));
        }
        if c.chrm {
            n_chrm += 1;
            tags.push("cHRM".into());
        }
        if !tags.is_empty() {
            println!(
                "{cid} {}x{} ct{} bd{} {} [{}]",
                c.w,
                c.h,
                c.color_type,
                c.bit_depth,
                kind,
                tags.join(" ")
            );
        }
    }
    eprintln!("=== AGGREGATE ===");
    eprintln!("standalone-texture pairs: {n_total}");
    eprintln!("decoded as PNG: {n_png}");
    eprintln!("gAMA non-trivial: {n_gama_nontrivial}");
    eprintln!("gAMA trivial(45455): {n_gama_trivial}");
    eprintln!("sRGB chunk: {n_srgb}");
    eprintln!("iCCP chunk: {n_iccp}");
    eprintln!("cHRM chunk: {n_chrm}");
    eprintln!("--- gAMA value histogram ---");
    for (v, n) in &gama_vals {
        eprintln!("  gAMA={v}: {n}");
    }
    eprintln!("--- iCCP profile-name histogram ---");
    for (name, n) in &iccp_names {
        eprintln!("  '{name}': {n}");
    }
}
