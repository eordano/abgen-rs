// Render-test for the magenta broken-asset placeholder font.
//   cargo run --example placeholder_demo -- /tmp/ph.png
use abgen::placeholder;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/ph.png".into());
    let img = placeholder::missing_texture("MISSING:", "models/PlantSF_12/SciFiPack_TX.png", 256);
    img.save(&out).unwrap();
    let img2 = placeholder::error_texture(
        &["BAD TEXTURE:", "file1.png", "DECODE FAILED 0123456789"],
        256,
    );
    img2.save(out.replace(".png", "_2.png")).unwrap();
    eprintln!("wrote {out} (+_2)");
}
