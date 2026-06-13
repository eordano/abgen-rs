use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

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

struct Chan {
    stream: usize,
    offset: usize,
    format: u8,
    dim: usize,
}

fn fmt_size(format: u8) -> usize {
    match format {
        0 => 4,
        1 | 4 | 5 | 8 | 9 => 2,
        2 | 3 | 6 | 7 => 1,
        _ => 4,
    }
}

fn layout(vd: &Value) -> (usize, Vec<Chan>, Vec<usize>, Vec<usize>) {
    let vcount = as_i(get(vd, "m_VertexCount").unwrap()) as usize;
    let chans: Vec<Chan> = get(vd, "m_Channels")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| Chan {
                    stream: as_i(get(c, "stream").unwrap()) as usize,
                    offset: as_i(get(c, "offset").unwrap()) as usize,
                    format: as_i(get(c, "format").unwrap()) as u8,
                    dim: (as_i(get(c, "dimension").unwrap()) & 0xF) as usize,
                })
                .collect()
        })
        .unwrap_or_default();
    let nstreams = chans.iter().map(|c| c.stream).max().unwrap_or(0) + 1;
    let mut strides = vec![0usize; nstreams];
    for c in &chans {
        if c.dim == 0 {
            continue;
        }
        let end = c.offset + fmt_size(c.format) * c.dim;
        strides[c.stream] = strides[c.stream].max(end);
    }
    let mut starts = vec![0usize; nstreams];
    let mut acc = 0usize;
    for s in 0..nstreams {
        starts[s] = acc;
        acc += strides[s] * vcount;
        acc = (acc + 15) & !15;
    }
    (vcount, chans, strides, starts)
}

fn read_lane(
    data: &[u8],
    chans: &[Chan],
    strides: &[usize],
    starts: &[usize],
    vcount: usize,
    chan_idx: usize,
) -> Option<Vec<Vec<u32>>> {
    let c = chans.get(chan_idx)?;
    if c.dim == 0 || c.format != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(vcount);
    for v in 0..vcount {
        let base = starts[c.stream] + v * strides[c.stream] + c.offset;
        let mut comps = Vec::with_capacity(c.dim);
        for d in 0..c.dim {
            let o = base + d * 4;
            comps.push(u32::from_le_bytes(data[o..o + 4].try_into().unwrap()));
        }
        out.push(comps);
    }
    Some(out)
}

fn main() {
    let mut a = std::env::args().skip(1);
    let ours = PathBuf::from(a.next().expect("ours"));
    let refb = PathBuf::from(a.next().expect("ref"));
    let pre = a.next().expect("out prefix");
    let ob = Bundle::load(&ours).expect("load ours");
    let rb = Bundle::load(&refb).expect("load ref");
    let osf = ob.serialized().expect("osf");
    let rsf = rb.serialized().expect("rsf");
    let omap: BTreeMap<i64, _> = osf.objects.iter().map(|o| (o.path_id, o)).collect();

    let mut k = 0usize;
    for ro in &rsf.objects {
        if ro.class_id != 43 {
            continue;
        }
        let Some(oo) = omap.get(&ro.path_id) else {
            continue;
        };
        if oo.data == ro.data {
            continue;
        }
        let (Ok(rv), Ok(ov)) = (rsf.read_typetree(ro), osf.read_typetree(oo)) else {
            continue;
        };
        let name = get(&rv, "m_Name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let rvd = get(&rv, "m_VertexData").unwrap();
        let ovd = get(&ov, "m_VertexData").unwrap();
        let (vcount, chans, strides, starts) = layout(rvd);
        let (Some(Value::Bytes(rd)), Some(Value::Bytes(od))) =
            (get(rvd, "m_DataSize"), get(ovd, "m_DataSize"))
        else {
            continue;
        };

        let ifmt = get(&rv, "m_IndexFormat").map(as_i).unwrap_or(0);
        let Some(Value::Bytes(ib)) = get(&rv, "m_IndexBuffer") else {
            continue;
        };
        let indices: Vec<u32> = if ifmt == 0 {
            ib.chunks_exact(2)
                .map(|c| u16::from_le_bytes(c.try_into().unwrap()) as u32)
                .collect()
        } else {
            ib.chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect()
        };
        let pos = read_lane(rd, &chans, &strides, &starts, vcount, 0);
        let nrm = read_lane(rd, &chans, &strides, &starts, vcount, 1);
        let tan_r = read_lane(rd, &chans, &strides, &starts, vcount, 2);
        let tan_o = read_lane(od, &chans, &strides, &starts, vcount, 2);
        let uv = read_lane(rd, &chans, &strides, &starts, vcount, 4);
        let (Some(pos), Some(nrm), Some(tan_r), Some(tan_o)) = (pos, nrm, tan_r, tan_o) else {
            println!("SKIP '{}' pid={} (missing/non-f32 lane)", name, ro.path_id);
            continue;
        };

        let geom_same = {
            let same_lane = |a: &Vec<Vec<u32>>, b: &Vec<Vec<u32>>| a == b;
            let pos_o = read_lane(od, &chans, &strides, &starts, vcount, 0).unwrap();
            let nrm_o = read_lane(od, &chans, &strides, &starts, vcount, 1).unwrap();
            same_lane(&pos, &pos_o) && same_lane(&nrm, &nrm_o)
        };
        let path = format!("{pre}.{k}.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            "# name={} pid={} vcount={} ntris={} geom_same={} subm={}",
            name,
            ro.path_id,
            vcount,
            indices.len() / 3,
            geom_same,
            get(&rv, "m_SubMeshes")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
        )
        .unwrap();
        for p in &pos {
            writeln!(f, "P {:08x} {:08x} {:08x}", p[0], p[1], p[2]).unwrap();
        }
        for nn in &nrm {
            writeln!(f, "N {:08x} {:08x} {:08x}", nn[0], nn[1], nn[2]).unwrap();
        }
        if let Some(uv) = &uv {
            for u in uv.iter() {
                writeln!(f, "U {:08x} {:08x}", u[0], u[1]).unwrap();
            }
        }
        for t in indices.chunks_exact(3) {
            writeln!(f, "I {} {} {}", t[0], t[1], t[2]).unwrap();
        }
        for t in &tan_o {
            writeln!(f, "O {:08x} {:08x} {:08x} {:08x}", t[0], t[1], t[2], t[3]).unwrap();
        }
        for t in &tan_r {
            writeln!(f, "T {:08x} {:08x} {:08x} {:08x}", t[0], t[1], t[2], t[3]).unwrap();
        }
        println!(
            "{path}: '{}' pid={} v={} tris={} geom_same={}",
            name,
            ro.path_id,
            vcount,
            indices.len() / 3,
            geom_same
        );
        k += 1;
    }
}
