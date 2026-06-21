
use abgen::validate::{bundle_cab_names, validate_bundle, Severity, ValidateCtx};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn usage() -> ! {
    eprintln!(
        "usage: abgen-validate <bundle-file | dir> [more...] [--quiet] [--warn-fatal]\n\
         \n\
         Reference-free structural validator for emitted AssetBundles. Reports\n\
         ERR/WARN findings per bundle and exits non-zero if any ERR fires.\n\
         --quiet      only print findings + summary (suppress per-file OK lines)\n\
         --warn-fatal treat WARN findings as failures (non-zero exit)."
    );
    std::process::exit(2);
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut quiet = false;
    let mut warn_fatal = false;
    for a in &argv {
        match a.as_str() {
            "--quiet" => quiet = true,
            "--warn-fatal" => warn_fatal = true,
            "-h" | "--help" => usage(),
            s if s.starts_with('-') => {
                eprintln!("unknown flag: {s}");
                usage();
            }
            s => paths.push(PathBuf::from(s)),
        }
    }
    if paths.is_empty() {
        usage();
    }

    let mut bundles: Vec<PathBuf> = Vec::new();
    for p in &paths {
        collect_bundles(p, &mut bundles);
    }
    bundles.sort();
    bundles.dedup();
    if bundles.is_empty() {
        eprintln!("no bundle files found under the given path(s)");
        std::process::exit(2);
    }

    let single_file = bundles.len() == 1 && paths.len() == 1 && paths[0].is_file();
    let ctx = if single_file {
        ValidateCtx::single_file()
    } else {
        let mut cabs: HashSet<String> = HashSet::new();
        for b in &bundles {
            if let Ok(data) = std::fs::read(b) {
                for name in bundle_cab_names(&data) {
                    cabs.insert(name);
                }
            }
        }
        ValidateCtx::with_global_cabs(cabs)
    };

    let mut n_err = 0usize;
    let mut n_warn = 0usize;
    let mut n_bundles = 0usize;
    let mut n_clean = 0usize;

    for b in &bundles {
        n_bundles += 1;
        let label = b.display().to_string();
        let data = match std::fs::read(b) {
            Ok(d) => d,
            Err(e) => {
                println!("ERR  E0 {label}: cannot read file: {e}");
                n_err += 1;
                continue;
            }
        };
        let findings = validate_bundle(&data, &label, &ctx);
        if findings.is_empty() {
            n_clean += 1;
            if !quiet {
                println!("OK   {label}");
            }
            continue;
        }
        for f in &findings {
            match f.severity {
                Severity::Error => n_err += 1,
                Severity::Warn => n_warn += 1,
            }
            println!("{} {} {}: {}", f.severity.as_str(), f.code, f.bundle, f.msg);
        }
    }

    println!(
        "\n{n_bundles} bundles, {n_clean} clean, {n_err} errors, {n_warn} warns"
    );
    if n_err > 0 || (warn_fatal && n_warn > 0) {
        std::process::exit(1);
    }
}

fn collect_bundles(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_file() {
        if looks_like_bundle(p) {
            out.push(p.to_path_buf());
        }
        return;
    }
    let Ok(rd) = std::fs::read_dir(p) else { return };
    for ent in rd.flatten() {
        let path = ent.path();
        if path.is_dir() {
            collect_bundles(&path, out);
        } else if looks_like_bundle(&path) {
            out.push(path);
        }
    }
}

fn looks_like_bundle(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".json") || name.ends_with(".manifest") || name.ends_with(".txt") {
        return false;
    }
    if name.ends_with("_windows") || name.ends_with("_mac") || name.ends_with("_webgl") {
        return true;
    }

    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(p) else {
        return false;
    };
    let mut magic = [0u8; 7];
    matches!(f.read_exact(&mut magic), Ok(())) && &magic == b"UnityFS"
}
