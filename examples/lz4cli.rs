fn main() {
    let mut args = std::env::args().skip(1);
    let inp = args.next().unwrap();
    let data = std::fs::read(&inp).unwrap();
    let out = abgen::lz4::compress_hc(&data);
    std::io::Write::write_all(&mut std::io::stdout().lock(), &out).unwrap();
}
