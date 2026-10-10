//! `newpub-agent`: the MCP server (default) and a command line over the engine. See docs/agent.md.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};
use newpub_engine::Session;
use newpub_engine::layout::FontStore;
use serde_json::Value;

#[derive(Parser)]
#[command(name = "newpub-agent", version, about = "Agent access to newpub: an MCP server and a command line")]
struct Cli {
    /// Use only the bundled fonts (deterministic output, as in journeys) instead of the system's fonts.
    #[arg(long, global = true)]
    bundled_fonts: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve MCP (JSON-RPC 2.0, one message per line) on stdin/stdout. This is the default.
    Serve {
        /// Open this publication first.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Run actions ({"cmd": ...} JSON objects, or "name" followed by its fields as JSON) on a publication.
    Exec {
        /// The publication to open first (omit to start blank).
        #[arg(long)]
        file: Option<PathBuf>,
        /// Save afterwards: `--save` writes back to --file, `--save=other.newspub` elsewhere.
        #[arg(long, require_equals = true, num_args = 0..=1, default_missing_value = "")]
        save: Option<String>,
        /// Actions, each a JSON object with a "cmd" field.
        actions: Vec<String>,
    },
    /// Answer a query ({"q": ...} JSON) about a publication.
    Query {
        #[arg(long)]
        file: Option<PathBuf>,
        query: String,
    },
    /// Render a page to a PNG file.
    Render {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value_t = 0)]
        page: usize,
        #[arg(long, default_value_t = 96.0)]
        dpi: f64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Print the publication status (what newpub_status returns).
    Status {
        #[arg(long)]
        file: PathBuf,
    },
    /// Print the action and query reference (optionally only entries mentioning a word).
    Reference { search: Option<String> },
}

fn session(bundled: bool) -> Session {
    let fonts = if bundled { FontStore::bundled() } else { FontStore::with_system() };
    let mut s = Session::new(Arc::new(fonts));
    s.base_dir = std::env::current_dir().unwrap_or_default();
    s
}

fn open(s: &mut Session, file: &Option<PathBuf>) -> Result<()> {
    if let Some(f) = file {
        let r = tool(s, "newpub_open", serde_json::json!({"path": f.to_string_lossy()}))?;
        if r.is_error {
            return Err(anyhow!("{}", r.text_of()));
        }
    }
    Ok(())
}

/// Runs a tool; a failure is printed to stderr and ends the program with exit status 1.
fn tool(s: &mut Session, name: &str, args: Value) -> Result<newpub_agent::ToolResult> {
    let r = newpub_agent::call_tool(s, name, &args)?;
    if r.is_error {
        eprintln!("{}", r.text_of());
        std::process::exit(1);
    }
    Ok(r)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut s = session(cli.bundled_fonts);
    match cli.command.unwrap_or(Cmd::Serve { file: None }) {
        Cmd::Serve { file } => {
            open(&mut s, &file)?;
            newpub_agent::serve_stdio(&mut s)
        }
        Cmd::Exec { file, save, actions } => {
            open(&mut s, &file)?;
            let mut i = 0;
            while i < actions.len() {
                let a = &actions[i];
                let (name, args): (String, Value) = if a.trim_start().starts_with('{') {
                    let v: Value = serde_json::from_str(a)?;
                    let name = v.get("cmd").and_then(|c| c.as_str()).ok_or_else(|| anyhow!("{a}: no \"cmd\""))?;
                    (name.to_string(), strip(v, "cmd"))
                } else {
                    // "name" then, optionally, its fields as a JSON object.
                    let next = actions.get(i + 1).filter(|n| !n.trim_start().starts_with(|c: char| c.is_alphabetic()));
                    let args = match next {
                        Some(n) => {
                            i += 1;
                            let v: Value = serde_json::from_str(n)?;
                            if !v.is_object() {
                                bail!("the fields of {a} must be a JSON object, got {n}");
                            }
                            v
                        }
                        None => Value::Null,
                    };
                    (a.clone(), args)
                };
                let r = tool(&mut s, "newpub_action", serde_json::json!({"action": name, "args": args}))?;
                println!("{}", r.text_of());
                i += 1;
            }
            if let Some(path) = save {
                let args = if path.is_empty() { serde_json::json!({}) } else { serde_json::json!({"path": path}) };
                println!("{}", tool(&mut s, "newpub_save", args)?.text_of());
            }
            Ok(())
        }
        Cmd::Query { file, query } => {
            open(&mut s, &file)?;
            let v: Value = serde_json::from_str(&query)?;
            let name = v.get("q").and_then(|q| q.as_str()).ok_or_else(|| anyhow!("{query}: no \"q\""))?.to_string();
            let r = tool(&mut s, "newpub_query", serde_json::json!({"query": name, "args": strip(v, "q")}))?;
            match &r.structured {
                Some(v) => println!("{}", serde_json::to_string_pretty(v)?),
                None => println!("{}", r.text_of()),
            }
            Ok(())
        }
        Cmd::Render { file, page, dpi, out } => {
            open(&mut s, &Some(file))?;
            if !(newpub_agent::MIN_DPI..=newpub_agent::MAX_DPI).contains(&dpi) {
                bail!("--dpi must be between {} and {}", newpub_agent::MIN_DPI, newpub_agent::MAX_DPI);
            }
            let png = s.page_png(page, dpi).map_err(|e| anyhow!("{e}"))?;
            std::fs::write(&out, png)?;
            println!("wrote {}", out.display());
            Ok(())
        }
        Cmd::Status { file } => {
            open(&mut s, &Some(file))?;
            println!("{}", serde_json::to_string_pretty(&newpub_agent::status(&mut s))?);
            Ok(())
        }
        Cmd::Reference { search } => {
            let args = match search {
                Some(w) => serde_json::json!({"search": w}),
                None => serde_json::json!({}),
            };
            println!("{}", tool(&mut s, "newpub_reference", args)?.text_of());
            Ok(())
        }
    }
}

fn strip(mut v: Value, key: &str) -> Value {
    if let Some(m) = v.as_object_mut() {
        m.remove(key);
    }
    v
}
