//! `newpub-agent`: the MCP server (default) and a command line over the engine. See docs/agent.md.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
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
        /// Save afterwards, to this path or (with no value) back to --file.
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
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
        let r = newpub_agent::call_tool(s, "newpub_open", &serde_json::json!({"path": f.to_string_lossy()}));
        if r.is_error {
            return Err(anyhow!("{}", text_of(&r)));
        }
    }
    Ok(())
}

fn text_of(r: &newpub_agent::ToolResult) -> String {
    r.content.iter().filter_map(|c| c.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n")
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
                    (name.to_string(), v.clone())
                } else {
                    // "name" then optional fields JSON.
                    let next = actions.get(i + 1).filter(|n| n.trim_start().starts_with('{'));
                    let args = match next {
                        Some(n) => {
                            i += 1;
                            serde_json::from_str(n)?
                        }
                        None => Value::Null,
                    };
                    (a.clone(), args)
                };
                let r = newpub_agent::call_tool(
                    &mut s,
                    "newpub_action",
                    &serde_json::json!({"action": name, "args": strip_cmd(args)}),
                );
                println!("{}", text_of(&r));
                if r.is_error {
                    std::process::exit(1);
                }
                i += 1;
            }
            if let Some(path) = save {
                let path = if path.is_empty() { None } else { Some(path) };
                let args = match path {
                    Some(p) => serde_json::json!({"path": p}),
                    None => serde_json::json!({}),
                };
                let r = newpub_agent::call_tool(&mut s, "newpub_save", &args);
                println!("{}", text_of(&r));
                if r.is_error {
                    std::process::exit(1);
                }
            }
            Ok(())
        }
        Cmd::Query { file, query } => {
            open(&mut s, &file)?;
            let v: Value = serde_json::from_str(&query)?;
            let name = v.get("q").and_then(|q| q.as_str()).ok_or_else(|| anyhow!("{query}: no \"q\""))?.to_string();
            let r = newpub_agent::call_tool(
                &mut s,
                "newpub_query",
                &serde_json::json!({"query": name, "args": strip_q(v)}),
            );
            match &r.structured {
                Some(v) if !r.is_error => println!("{}", serde_json::to_string_pretty(v)?),
                _ => println!("{}", text_of(&r)),
            }
            if r.is_error {
                std::process::exit(1);
            }
            Ok(())
        }
        Cmd::Render { file, page, dpi, out } => {
            open(&mut s, &Some(file))?;
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
            println!("{}", text_of(&newpub_agent::call_tool(&mut s, "newpub_reference", &args)));
            Ok(())
        }
    }
}

fn strip_cmd(mut v: Value) -> Value {
    if let Some(m) = v.as_object_mut() {
        m.remove("cmd");
    }
    v
}

fn strip_q(mut v: Value) -> Value {
    if let Some(m) = v.as_object_mut() {
        m.remove("q");
    }
    v
}
