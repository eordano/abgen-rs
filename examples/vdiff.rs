use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

const CHANNEL_NAMES: [&str; 14] = [
    "Position",
    "Normal",
    "Tangent",
    "Color",
    "UV0",
    "UV1",
    "UV2",
    "UV3",
    "UV4",
    "UV5",
    "UV6",
    "UV7",
    "BlendWeight",
    "BlendIndices",
];

#[derive(Debug, Clone)]
struct Chan {
    stream: u8,
    offset: u8,
    format: u8,
    dim: u8,
}

fn fmt_size(format: u8) -> usize {
    match format {
        0 => 4,
        1 => 2,
        2 | 3 => 1,
        4 | 5 => 2,
        6 | 7 => 1,
        8 | 9 => 2,
        10 | 11 => 4,
        _ => 4,
    }
}

fn get<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.as_map().and_then(|m| m.get(k))
}

fn as_i(v: &Value) -> i64 {
    match v {
        Value::Int(i) => *i,
        Value::Float(f) => *f as i64,
        Value::Bool(b) => *b as i64,
        _ => 0,
    }
}

fn main() {
    let mut a = std::env::args().skip(1);
    let ours = PathBuf::from(a.next().expect("ours"));
    let refb = PathBuf::from(a.next().expect("ref"));
    let max_rows: usize = a.next().map(|s| s.parse().unwrap()).unwrap_or(40);
    let ob = Bundle::load(&ours).expect("load ours");
    let rb = Bundle::load(&refb).expect("load ref");
    let osf = ob.serialized().expect("osf");
    let rsf = rb.serialized().expect("rsf");
    let rmap: BTreeMap<i64, _> = rsf.objects.iter().map(|o| (o.path_id, o)).collect();

    for oo in &osf.objects {
        if oo.class_id != 43 {
            continue;
        }
        let Some(ro) = rmap.get(&oo.path_id) else {
            continue;
        };
        if oo.data == ro.data {
            continue;
        }
        let (Ok(ov), Ok(rv)) = (osf.read_typetree(oo), rsf.read_typetree(ro)) else {
            continue;
        };
        let name = get(&ov, "m_Name").and_then(|v| v.as_str()).unwrap_or("");
        let ovd = get(&ov, "m_VertexData").unwrap();
        let rvd = get(&rv, "m_VertexData").unwrap();
        let vcount = as_i(get(ovd, "m_VertexCount").unwrap()) as usize;
        let chans: Vec<Chan> = get(ovd, "m_Channels")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|c| Chan {
                        stream: as_i(get(c, "stream").unwrap()) as u8,
                        offset: as_i(get(c, "offset").unwrap()) as u8,
                        format: as_i(get(c, "format").unwrap()) as u8,
                        dim: as_i(get(c, "dimension").unwrap()) as u8 & 0xF,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (Some(Value::Bytes(od)), Some(Value::Bytes(rd))) =
            (get(ovd, "m_DataSize"), get(rvd, "m_DataSize"))
        else {
            continue;
        };
        if od == rd {
            println!(
                "MESH '{}' pid={} : m_DataSize identical (diff elsewhere)",
                name, oo.path_id
            );
            continue;
        }
        if od.len() != rd.len() {
            println!(
                "MESH '{}' pid={} : m_DataSize LEN {} vs {}",
                name,
                oo.path_id,
                od.len(),
                rd.len()
            );
            continue;
        }

        let nstreams = chans.iter().map(|c| c.stream).max().unwrap_or(0) as usize + 1;
        let mut strides = vec![0usize; nstreams];
        for c in &chans {
            if c.dim == 0 {
                continue;
            }
            let end = c.offset as usize + fmt_size(c.format) * c.dim as usize;
            if end > strides[c.stream as usize] {
                strides[c.stream as usize] = end;
            }
        }
        let mut stream_start = vec![0usize; nstreams];
        let mut acc = 0usize;
        for s in 0..nstreams {
            stream_start[s] = acc;
            acc += strides[s] * vcount;
            acc = (acc + 15) & !15;
        }

        println!(
            "MESH '{}' pid={} vcount={} streams={:?} chans={}",
            name,
            oo.path_id,
            vcount,
            strides,
            chans
                .iter()
                .enumerate()
                .filter(|(_, c)| c.dim > 0)
                .map(|(i, c)| format!(
                    "{}@s{}+{} f{} d{}",
                    CHANNEL_NAMES.get(i).copied().unwrap_or("?"),
                    c.stream,
                    c.offset,
                    c.format,
                    c.dim
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let mut rows = 0usize;
        let mut skipped = 0usize;
        let mut chan_hist: BTreeMap<String, usize> = BTreeMap::new();
        let mut i = 0usize;
        while i < od.len() {
            if od[i] == rd[i] {
                i += 1;
                continue;
            }

            let mut s = nstreams - 1;
            for st in 0..nstreams {
                if i >= stream_start[st] && i < stream_start[st] + strides[st] * vcount {
                    s = st;
                    break;
                }
            }
            let rel = i - stream_start[s];
            let vtx = rel / strides[s];
            let within = rel % strides[s];

            let mut label = format!("s{}+{}", s, within);
            let mut comp = 0usize;
            let mut fsz = 4usize;
            let mut coff = within;
            for (ci, c) in chans.iter().enumerate() {
                if c.dim == 0 || c.stream as usize != s {
                    continue;
                }
                let sz = fmt_size(c.format) * c.dim as usize;
                if within >= c.offset as usize && within < c.offset as usize + sz {
                    fsz = fmt_size(c.format);
                    comp = (within - c.offset as usize) / fsz;
                    coff = (within - c.offset as usize) % fsz;
                    label = format!("{}.{}", CHANNEL_NAMES.get(ci).copied().unwrap_or("?"), comp);
                    break;
                }
            }
            let _ = comp;

            let elem = i - coff;
            let (oval, rval) = if fsz == 4 && elem + 4 <= od.len() {
                let o = f32::from_le_bytes(od[elem..elem + 4].try_into().unwrap());
                let r = f32::from_le_bytes(rd[elem..elem + 4].try_into().unwrap());
                (
                    format!("{:.9e}(0x{:08x})", o, o.to_bits()),
                    format!("{:.9e}(0x{:08x})", r, r.to_bits()),
                )
            } else if fsz == 2 && elem + 2 <= od.len() {
                (
                    format!(
                        "0x{:04x}",
                        u16::from_le_bytes(od[elem..elem + 2].try_into().unwrap())
                    ),
                    format!(
                        "0x{:04x}",
                        u16::from_le_bytes(rd[elem..elem + 2].try_into().unwrap())
                    ),
                )
            } else {
                (format!("0x{:02x}", od[i]), format!("0x{:02x}", rd[i]))
            };
            *chan_hist.entry(label.clone()).or_default() += 1;
            if rows < max_rows {
                println!("  v{} {} : {} -> {}", vtx, label, oval, rval);
                rows += 1;
            } else {
                skipped += 1;
            }

            i = elem + fsz.max(1);
        }
        if skipped > 0 {
            println!("  ... {} more diff elements", skipped);
        }
        println!(
            "  HIST: {}",
            chan_hist
                .iter()
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
}
