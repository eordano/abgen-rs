use abgen::unity::bundle_file::Bundle;
use abgen::value::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn rd(p: &str, pid: i64) -> (Vec<Value>, Vec<u8>, usize) {
    let b = Bundle::load(&PathBuf::from(p)).expect("load");
    let sf = b.serialized().expect("sf");
    let o = sf.objects.iter().find(|o| o.path_id == pid).expect("pid");
    let v = sf.read_typetree(o).unwrap();
    let m = v.as_map().unwrap();
    let vd = m.get("m_VertexData").and_then(|x| x.as_map()).expect("vd");
    let chans = vd
        .get("m_Channels")
        .and_then(|x| x.as_array())
        .unwrap()
        .to_vec();
    let data = match vd.get("m_DataSize") {
        Some(Value::Bytes(d)) => d.clone(),
        _ => panic!("no datasize"),
    };
    let vc = vd.get("m_VertexCount").and_then(|x| x.as_i64()).unwrap() as usize;
    (chans, data, vc)
}

fn cf(c: &Value, k: &str) -> i64 {
    c.as_map().unwrap().get(k).and_then(|x| x.as_i64()).unwrap()
}

fn f32at(d: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn main() {
    let mut a = std::env::args().skip(1);
    let ours = a.next().unwrap();
    let refb = a.next().unwrap();
    let pid: i64 = a.next().unwrap().parse().unwrap();
    let (chans, od, vc) = rd(&ours, pid);
    let (_rc, rd2, _) = rd(&refb, pid);
    let names = [
        "POS", "NRM", "TAN", "COL", "UV0", "UV1", "UV2", "UV3", "UV4", "UV5", "UV6", "UV7", "BW",
        "BI",
    ];
    let mut by_stream: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (i, c) in chans.iter().enumerate() {
        if cf(c, "dimension") > 0 {
            by_stream.entry(cf(c, "stream")).or_default().push(i);
        }
    }
    let mut base = 0usize;
    let mut stream_base: BTreeMap<i64, (usize, usize)> = BTreeMap::new();
    for (si, (s, cis)) in by_stream.iter().enumerate() {
        if si > 0 {
            while base % 16 != 0 {
                base += 1;
            }
        }
        let stride_raw = cis
            .iter()
            .map(|&ci| cf(&chans[ci], "offset") + cf(&chans[ci], "dimension") * 4)
            .max()
            .unwrap();
        let stride = ((stride_raw + 3) & !3) as usize;
        stream_base.insert(*s, (base, stride));
        base += stride * vc;
    }
    println!("vertexCount={vc} datalen={}", od.len());
    for (s, cis) in by_stream.iter() {
        let (b, st) = stream_base[s];
        print!("stream {s} base={b} stride={st}: ");
        for &ci in cis {
            print!(
                "{}@off{} dim{}  ",
                names[ci],
                cf(&chans[ci], "offset"),
                cf(&chans[ci], "dimension")
            );
        }
        println!();
    }
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let mut samples: Vec<String> = Vec::new();
    for (&s, cis) in by_stream.iter() {
        let (b, st) = stream_base[&s];
        for v in 0..vc {
            let row = b + v * st;
            for &ci in cis {
                let coff = cf(&chans[ci], "offset") as usize;
                let dim = cf(&chans[ci], "dimension") as usize;
                for k in 0..dim {
                    let o = row + coff + k * 4;
                    if o + 4 > od.len() || o + 4 > rd2.len() {
                        continue;
                    }
                    if od[o..o + 4] != rd2[o..o + 4] {
                        let key = format!("{}[{}]", names[ci], k);
                        *tally.entry(key.clone()).or_default() += 1;
                        if samples.len() < 40 {
                            let of = f32at(&od, o);
                            let rf = f32at(&rd2, o);
                            let ob = of.to_bits();
                            let rb = rf.to_bits();
                            let ulp = (ob as i64 - rb as i64).abs();
                            samples.push(format!("v{v} {key}: ours={of:.9}({ob:08x}) ref={rf:.9}({rb:08x}) ulp={ulp}"));
                        }
                    }
                }
            }
        }
    }
    println!("--- lane diff tally ---");
    for (k, n) in &tally {
        println!("  {k}: {n}");
    }
    println!("--- samples ---");
    for s in &samples {
        println!("  {s}");
    }
}
