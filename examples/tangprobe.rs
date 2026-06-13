use std::env;
use std::fs;
use std::io::Write;

fn main() {
    let glb = env::args().nth(1).expect("glb path");
    let pre = env::args().nth(2).expect("out prefix");
    let bytes = fs::read(&glb).expect("read glb");
    let ext = if bytes.starts_with(b"glTF") {
        "glb"
    } else {
        "gltf"
    };
    let scene = abgen::gltf::parse(&bytes, ext, None).expect("parse");
    let mut k = 0usize;
    for (ni, node) in scene.nodes.iter().enumerate() {
        for (pi, prim) in node.primitives.iter().enumerate() {
            let Some(tangents) = prim.tangents.as_ref() else {
                continue;
            };
            let empty: Vec<[f64; 2]> = Vec::new();
            let uvs = prim.uvs.as_deref().unwrap_or(&empty);
            let path = format!("{pre}.prim{k}.txt");
            let mut f = fs::File::create(&path).unwrap();
            writeln!(
                f,
                "# node={ni} prim={pi} name={} go={} nverts={} nidx={} nuv={}",
                prim.name,
                prim.go_name,
                prim.positions.len(),
                prim.indices.len(),
                uvs.len()
            )
            .unwrap();
            for p in &prim.positions {
                writeln!(
                    f,
                    "P {:08x} {:08x} {:08x}",
                    (p[0] as f32).to_bits(),
                    (p[1] as f32).to_bits(),
                    (p[2] as f32).to_bits()
                )
                .unwrap();
            }
            for nrm in &prim.normals {
                writeln!(
                    f,
                    "N {:08x} {:08x} {:08x}",
                    (nrm[0] as f32).to_bits(),
                    (nrm[1] as f32).to_bits(),
                    (nrm[2] as f32).to_bits()
                )
                .unwrap();
            }
            for u in uvs {
                writeln!(
                    f,
                    "U {:08x} {:08x}",
                    (u[0] as f32).to_bits(),
                    (u[1] as f32).to_bits()
                )
                .unwrap();
            }
            for tri in prim.indices.chunks(3) {
                writeln!(f, "I {} {} {}", tri[0], tri[1], tri[2]).unwrap();
            }
            for t in tangents {
                writeln!(
                    f,
                    "T {:08x} {:08x} {:08x} {:08x}",
                    (t[0] as f32).to_bits(),
                    (t[1] as f32).to_bits(),
                    (t[2] as f32).to_bits(),
                    (t[3] as f32).to_bits()
                )
                .unwrap();
            }
            println!(
                "{path}: node={ni} prim={pi} verts={} idx={}",
                prim.positions.len(),
                prim.indices.len()
            );
            k += 1;
        }
    }
    if k == 0 {
        println!("no tangent-bearing primitives");
    }
}
