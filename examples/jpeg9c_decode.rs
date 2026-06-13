fn main() {
    let args: Vec<String> = std::env::args().collect();
    let jpeg = std::fs::read(&args[1]).unwrap();
    let (rgba, w, h) = libjpeg9c::decode_rgba(&jpeg, false).unwrap();

    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for px in rgba.chunks(4) {
        rgb.extend_from_slice(&px[..3]);
    }
    use std::io::Write;
    std::io::stdout().write_all(&rgb).unwrap();
    eprintln!("{}x{}", w, h);
}
