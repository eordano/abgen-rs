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
            if prim.has_source_normals || prim.indices.len() < 3 {
                continue;
            }
            let path = format!("{pre}.prim{k}.txt");
            let mut f = fs::File::create(&path).unwrap();
            writeln!(
                f,
                "# node={ni} prim={pi} name={} go={} nverts={} nidx={}",
                prim.name,
                prim.go_name,
                prim.positions.len(),
                prim.indices.len()
            )
            .unwrap();
            for p in &prim.positions {
                let x = p[0] as f32;
                let y = p[1] as f32;
                let z = p[2] as f32;
                writeln!(
                    f,
                    "P {:08x} {:08x} {:08x}",
                    x.to_bits(),
                    y.to_bits(),
                    z.to_bits()
                )
                .unwrap();
            }
            for tri in prim.indices.chunks(3) {
                writeln!(f, "I {} {} {}", tri[0], tri[1], tri[2]).unwrap();
            }
            for n in &prim.normals {
                let x = n[0] as f32;
                let y = n[1] as f32;
                let z = n[2] as f32;
                writeln!(
                    f,
                    "N {:08x} {:08x} {:08x}",
                    x.to_bits(),
                    y.to_bits(),
                    z.to_bits()
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
        println!("no normal-less primitives");
    }
}
