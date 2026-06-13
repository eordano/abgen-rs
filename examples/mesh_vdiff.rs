use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;

#[derive(Clone, Copy, Debug)]
struct Ch {
    stream: u8,
    offset: u8,
    format: u8,
    dim: u8,
}

const ATTR_NAMES: &[&str] = &[
    "POSITION",
    "NORMAL",
    "TANGENT",
    "COLOR",
    "UV0",
    "UV1",
    "UV2",
    "UV3",
    "UV4",
    "UV5",
    "UV6",
    "UV7",
    "BLENDWEIGHT",
    "BLENDINDICES",
];

fn blob_and_channels(b: &Bundle, pid: i64) -> (Vec<u8>, Vec<Ch>, usize) {
    let sf = b.serialized().unwrap();
    let o = sf.objects.iter().find(|o| o.path_id == pid).unwrap();
    let v = sf.read_typetree(o).unwrap();
    let vd = v.get("m_VertexData").unwrap();
    let vcount = vd.get("m_VertexCount").and_then(|x| x.as_i64()).unwrap() as usize;
    let blob = match vd.get("m_DataSize") {
        Some(Value::Bytes(d)) => d.clone(),
        _ => panic!("no m_DataSize bytes"),
    };
    let mut chans = Vec::new();
    if let Some(arr) = vd.get("m_Channels").and_then(|x| x.as_array()) {
        for c in arr {
            chans.push(Ch {
                stream: c.get("stream").and_then(|x| x.as_i64()).unwrap_or(0) as u8,
                offset: c.get("offset").and_then(|x| x.as_i64()).unwrap_or(0) as u8,
                format: c.get("format").and_then(|x| x.as_i64()).unwrap_or(0) as u8,
                dim: c.get("dimension").and_then(|x| x.as_i64()).unwrap_or(0) as u8,
            });
        }
    }
    (blob, chans, vcount)
}

fn fmt_size(format: u8) -> usize {
    match format {
        0 => 4,
        1 => 2,
        2 => 1,
        3 => 1,
        10 | 11 => 4,
        _ => 4,
    }
}

fn main() {
    let ours = std::env::args().nth(1).unwrap();
    let refp = std::env::args().nth(2).unwrap();
    let pid: i64 = std::env::args().nth(3).unwrap().parse().unwrap();
    let ob = Bundle::load(std::path::Path::new(&ours)).unwrap();
    let rb = Bundle::load(std::path::Path::new(&refp)).unwrap();
    let (bo, chans, vc) = blob_and_channels(&ob, pid);
    let (br, _, _) = blob_and_channels(&rb, pid);
    assert_eq!(bo.len(), br.len());

    let nstreams = chans.iter().map(|c| c.stream).max().unwrap_or(0) as usize + 1;
    let mut stride = vec![0usize; nstreams];
    for c in &chans {
        if c.dim == 0 {
            continue;
        }
        let end = c.offset as usize + c.dim as usize * fmt_size(c.format);
        stride[c.stream as usize] = stride[c.stream as usize].max(end);
    }

    let mut base = vec![0usize; nstreams];
    let mut acc = 0usize;
    for s in 0..nstreams {
        base[s] = acc;
        acc += stride[s] * vc;
    }
    println!(
        "vcount={vc} blob={} streams={nstreams} strides={stride:?} bases={base:?}",
        bo.len()
    );

    let attr_of = |chan_idx: usize| -> &str {
        if chan_idx < ATTR_NAMES.len() {
            ATTR_NAMES[chan_idx]
        } else {
            "?"
        }
    };

    let mut reported: std::collections::BTreeMap<(usize, usize), (usize, i64, i64)> =
        std::collections::BTreeMap::new();
    let mut total = 0;
    for i in 0..bo.len() {
        if bo[i] == br[i] {
            continue;
        }
        total += 1;

        let mut s = 0;
        for st in 0..nstreams {
            if i >= base[st] && i < base[st] + stride[st] * vc {
                s = st;
            }
        }
        let local = i - base[s];
        let within = local % stride[s].max(1);

        let mut best: Option<(usize, usize)> = None;
        for (ci, c) in chans.iter().enumerate() {
            if c.stream as usize != s || c.dim == 0 {
                continue;
            }
            let fs = fmt_size(c.format);
            let start = c.offset as usize;
            let endb = start + c.dim as usize * fs;
            if within >= start && within < endb {
                let comp = (within - start) / fs;
                best = Some((ci, comp));
            }
        }
        if let Some((ci, comp)) = best {
            let e = reported.entry((ci, comp)).or_insert((0, 0, 0));
            e.0 += 1;
        }
    }
    println!("total diff bytes={total}");

    if nstreams >= 3 {
        let s = 2usize;
        let st = stride[s];
        let bse = base[s];
        let mut shown = 0;
        for vtx in 0..vc {
            let voff = bse + vtx * st;
            if bo[voff..voff + st] == br[voff..voff + st] {
                continue;
            }

            let wo: Vec<f32> = (0..4)
                .map(|c| f32::from_le_bytes(bo[voff + c * 4..voff + c * 4 + 4].try_into().unwrap()))
                .collect();
            let wr: Vec<f32> = (0..4)
                .map(|c| f32::from_le_bytes(br[voff + c * 4..voff + c * 4 + 4].try_into().unwrap()))
                .collect();
            let io: Vec<u32> = (0..4)
                .map(|c| {
                    u32::from_le_bytes(
                        bo[voff + 16 + c * 4..voff + 16 + c * 4 + 4]
                            .try_into()
                            .unwrap(),
                    )
                })
                .collect();
            let ir: Vec<u32> = (0..4)
                .map(|c| {
                    u32::from_le_bytes(
                        br[voff + 16 + c * 4..voff + 16 + c * 4 + 4]
                            .try_into()
                            .unwrap(),
                    )
                })
                .collect();
            if shown < 6 {
                println!("  s2 vtx{vtx}: w_ours={wo:?} w_ref={wr:?}\n           idx_ours={io:?} idx_ref={ir:?}");
                shown += 1;
            }
        }
    }
    for ((ci, comp), (cnt, _, _)) in &reported {
        let c = chans[*ci];
        let fs = fmt_size(c.format);
        let mut maxulp: i64 = 0;
        let mut ndiff = 0;

        if c.format == 10 || c.format == 11 {
            let mut maxd: i64 = 0;
            for vtx in 0..vc {
                let off = base[c.stream as usize]
                    + vtx * stride[c.stream as usize]
                    + c.offset as usize
                    + comp * fs;
                let uo = u32::from_le_bytes(bo[off..off + 4].try_into().unwrap());
                let ur = u32::from_le_bytes(br[off..off + 4].try_into().unwrap());
                if uo != ur {
                    ndiff += 1;
                    maxd = maxd.max((uo as i64 - ur as i64).abs());
                }
            }
            println!("  chan{ci}={} comp{comp} fmt={} (UINT): {cnt} diff-bytes, {ndiff} diff-uints, max delta {maxd}", attr_of(*ci), c.format);
            continue;
        }
        if c.format == 0 {
            for vtx in 0..vc {
                let off = base[c.stream as usize]
                    + vtx * stride[c.stream as usize]
                    + c.offset as usize
                    + comp * fs;
                let fo = f32::from_le_bytes(bo[off..off + 4].try_into().unwrap());
                let fr = f32::from_le_bytes(br[off..off + 4].try_into().unwrap());
                if fo.to_bits() != fr.to_bits() {
                    ndiff += 1;
                    let d = (fo.to_bits() as i64 - fr.to_bits() as i64).abs();
                    maxulp = maxulp.max(d);
                }
            }
        }
        println!(
            "  chan{ci}={} comp{comp} fmt={} : {cnt} diff-bytes, {ndiff} diff-floats, max {maxulp} ULP",
            attr_of(*ci), c.format
        );
    }
}
