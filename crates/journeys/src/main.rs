use clap::Parser;
use newpub_journeys::runner::{JourneyResult, Status, load_script, run_journey};
use std::path::PathBuf;

/// Runs journey scripts from journeys/scripts and writes results.json.
#[derive(Parser)]
struct Args {
    /// Only run journeys whose id contains this text (repeatable).
    #[arg(long)]
    filter: Vec<String>,
    /// Output directory.
    #[arg(long, default_value = "target/journeys")]
    out: PathBuf,
    /// journeys/ directory.
    #[arg(long, default_value = "journeys")]
    root: PathBuf,
    /// Label for the OS in results.json (default: the running OS).
    #[arg(long)]
    os: Option<String>,
    /// List journeys without running them.
    #[arg(long)]
    list: bool,
}

fn main() {
    let args = Args::parse();
    let scripts_dir = args.root.join("scripts");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&scripts_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|x| x == "yaml" || x == "yml").unwrap_or(false))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    let root = std::fs::canonicalize(&args.root).unwrap_or(args.root.clone());
    std::fs::create_dir_all(&args.out).expect("create output dir");
    let out = std::fs::canonicalize(&args.out).unwrap_or(args.out.clone());
    let mut results: Vec<JourneyResult> = vec![];
    for f in files {
        let script = match load_script(&f) {
            Ok(s) => s,
            Err(e) => {
                let id = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if args.filter.is_empty() || args.filter.iter().any(|x| id.contains(x)) {
                    eprintln!("ERROR {id}: {e:#}");
                    results.push(JourneyResult {
                        id,
                        title: String::new(),
                        covers: vec![],
                        ui: false,
                        status: Status::Error,
                        message: format!("{e:#}"),
                        failed_step: None,
                        duration_ms: 0,
                        artifacts: vec![],
                        screenshots: vec![],
                    });
                }
                continue;
            }
        };
        let id = script.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if !args.filter.is_empty() && !args.filter.iter().any(|x| id.contains(x)) {
            continue;
        }
        if args.list {
            println!("{id}");
            continue;
        }
        let r = run_journey(&script, &root, &out);
        let tag = match r.status {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::NeedsApproval => "APPROVE",
            Status::Error => "ERROR",
        };
        println!(
            "{tag:8} {:12} {:6}ms  {}{}",
            r.id,
            r.duration_ms,
            r.title,
            if r.message.is_empty() { String::new() } else { format!("\n         {}", r.message) }
        );
        results.push(r);
    }
    if args.list {
        return;
    }
    let os = args.os.unwrap_or_else(|| std::env::consts::OS.to_string());
    let pass = results.iter().filter(|r| matches!(r.status, Status::Pass)).count();
    let doc = serde_json::json!({ "os": os, "total": results.len(), "passed": pass, "journeys": results });
    let path = out.join("results.json");
    std::fs::write(&path, serde_json::to_string_pretty(&doc).expect("serialise results")).expect("write results.json");
    println!("\n{pass}/{} journeys passed on {os}; results in {}", results.len(), path.display());
    if pass != results.len() {
        std::process::exit(1);
    }
}
