use abgen::regen::{regenerate, EntityKind, RegenConfig};
use abgen::Result;

fn usage() -> ! {
    eprintln!(
        "usage: regen_all --output <dir> [--entity-type scene|wearable|emote|all] \\\n         \
         [--platform windows|mac|webgl] [--jobs N] [--local <content-store>] \\\n         \
         [--content-env <path>] [--catalyst <url>] [--ab-version <vN>] \\\n         \
         [--limit N] [--offset N] [--no-compress] [--dry-run] [--force]\n\n  \
         By default already-present outputs are skipped (incremental top-off).\n  \
         --force rebuilds every asset even if its output already exists."
    );
    std::process::exit(2);
}

fn main() -> Result<()> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut cfg = RegenConfig {
        jobs: 32,
        platform: "windows".to_string(),
        ..Default::default()
    };
    let mut have_output = false;
    let mut i = 0;
    while i < argv.len() {
        let next = |i: &mut usize| -> String {
            *i += 1;
            argv.get(*i).cloned().unwrap_or_else(|| usage())
        };
        match argv[i].as_str() {
            "--output" | "-o" => {
                cfg.output_dir = next(&mut i);
                have_output = true;
            }
            "--entity-type" => cfg.entity_kind = EntityKind::parse(&next(&mut i))?,
            "--platform" => cfg.platform = next(&mut i),
            "--jobs" | "-j" => cfg.jobs = next(&mut i).parse().unwrap_or_else(|_| usage()),
            "--local" => cfg.local = Some(next(&mut i)),
            "--content-env" => cfg.content_env = next(&mut i),
            "--catalyst" => cfg.catalyst = next(&mut i),
            "--ab-version" => cfg.ab_version = next(&mut i),
            "--limit" => cfg.limit = Some(next(&mut i).parse().unwrap_or_else(|_| usage())),
            "--offset" => cfg.offset = next(&mut i).parse().unwrap_or_else(|_| usage()),
            "--no-compress" => cfg.compress = false,
            "--dry-run" => cfg.dry_run = true,
            "--force" => cfg.force = true,
            "--magenta-missing" => cfg.magenta_missing = true,
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown arg {other:?}");
                usage();
            }
        }
        i += 1;
    }
    if !have_output {
        usage();
    }

    let report = regenerate(&cfg)?;
    eprintln!(
        "DONE: entities={} glb_refs={} unique={} converted={} already={} failed={} bytes={} dedup={:.2}x in {:.1}s",
        report.entities,
        report.glb_refs,
        report.unique_assets,
        report.converted,
        report.already_present,
        report.failed,
        report.output_bytes,
        report.dedup_ratio(),
        report.elapsed_secs,
    );
    if !report.failures.is_empty() {
        eprintln!("first failures:");
        for (k, e) in report.failures.iter().take(20) {
            eprintln!("  {k}: {e}");
        }
        // Persist the FULL failure list (the stderr dump above is only a sample)
        // so the distribution is analyzable after the run. One TSV row per
        // failure: <entity::glb>\t<flattened anyhow chain>. Output-neutral —
        // does not affect any bundle bytes.
        let path = format!("{}/_failures.{}.tsv", cfg.output_dir, cfg.platform);
        let mut body = String::from("key\terror\n");
        for (k, e) in &report.failures {
            let e1 = e.replace('\t', " ").replace('\n', " | ");
            body.push_str(&format!("{k}\t{e1}\n"));
        }
        match std::fs::write(&path, body) {
            Ok(()) => eprintln!("wrote {} failures to {path}", report.failures.len()),
            Err(err) => eprintln!("warn: could not write {path}: {err}"),
        }
    }
    Ok(())
}
