use abgen::bc7_pure::{encode_blocks, Bc7Profile, Params};
use std::io::Read;

fn main() {
    let probe = std::env::args().nth(1).expect("probe file");
    let prof = std::env::args().nth(2).unwrap_or_else(|| "basic".into());
    let perc: bool = std::env::args().nth(3).map(|s| s == "1").unwrap_or(true);

    let params = match prof.as_str() {
        "basic" => Params::basic(perc),
        "slow" => Params::slow(perc),
        _ => {
            eprintln!("profile basic|slow");
            std::process::exit(2)
        }
    };
    let _ = Bc7Profile::Basic;

    let f = std::fs::File::open(&probe).unwrap();
    let mut rdr = std::io::BufReader::new(f);
    let mut cnt = [0u8; 4];
    rdr.read_exact(&mut cnt).unwrap();
    let n = u32::from_le_bytes(cnt) as usize;

    let mut diff_total = 0u64;
    let mut diff_recovered = 0u64;
    let mut match_total = 0u64;
    let mut match_kept = 0u64;

    let mut rec = [0u8; 100];
    for _ in 0..n {
        if rdr.read_exact(&mut rec).is_err() {
            break;
        }
        let input = &rec[0..64];
        let ours = &rec[64..80];
        let refb = &rec[80..96];
        let is_diff = rec[96] == 1;
        let enc = encode_blocks(input, 1, &params);
        let eq_ref = enc.as_slice() == refb;
        let _ = ours;
        if is_diff {
            diff_total += 1;
            if eq_ref {
                diff_recovered += 1;
            }
        } else {
            match_total += 1;
            if eq_ref {
                match_kept += 1;
            }
        }
    }
    println!("Rust port profile={prof} perc={perc}  N={n}");
    println!(
        "  DIFF  recovered(==ref): {diff_recovered}/{diff_total} ({:.2}%)",
        100.0 * diff_recovered as f64 / diff_total.max(1) as f64
    );
    println!(
        "  MATCH kept(==ref):      {match_kept}/{match_total} ({:.2}%)",
        100.0 * match_kept as f64 / match_total.max(1) as f64
    );
}
