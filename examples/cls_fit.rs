fn main() {
    let tols: Vec<f32> = vec![abgen::animation_mecanim::CONST_CURVE_VALUE_TOL];
    for p in std::env::args().skip(1) {
        let bytes = std::fs::read(&p).unwrap();
        println!("== {p}");
        for &t in &tols {
            let counts = abgen::animation_mecanim::clip_partition_counts(&bytes, t);
            let s: Vec<String> = counts
                .iter()
                .map(|(n, sc, cc)| format!("{n}:{sc}/{cc}"))
                .collect();
            println!("  tol={t:<9e} {}", s.join("  "));
        }
    }
}
