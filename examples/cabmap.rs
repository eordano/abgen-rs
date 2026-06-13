fn main() {
    let dir = std::env::args().nth(1).expect("dir");
    let mut rows: Vec<(String, String)> = Vec::new();
    for e in std::fs::read_dir(&dir).expect("read_dir") {
        let e = e.expect("entry");
        if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        rows.push((abgen::cabname::cab_name(&name), name));
    }
    rows.sort();
    for (cab, name) in rows {
        println!("{}\t{}", cab, name);
    }
}
