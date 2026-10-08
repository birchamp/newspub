//! Headless journey runner. See ARCHITECTURE.md §7.

use anyhow::{Context, Result, anyhow, bail};
use newpub_engine::{Action, Query, Session};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pass,
    Fail,
    NeedsApproval,
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct JourneyResult {
    pub id: String,
    pub title: String,
    pub covers: Vec<String>,
    pub ui: bool,
    pub status: Status,
    pub message: String,
    /// 1-based index of the failing step.
    pub failed_step: Option<usize>,
    pub duration_ms: u128,
    /// Files produced, relative to the results directory.
    pub artifacts: Vec<String>,
    pub screenshots: Vec<String>,
}

pub struct Ctx {
    pub id: String,
    /// journeys/ directory (scripts, fixtures, goldens).
    pub root: PathBuf,
    /// Output directory for this journey.
    pub out: PathBuf,
    /// Results root (artifact paths are relative to it).
    pub results_root: PathBuf,
    pub vars: HashMap<String, Value>,
    pub artifacts: Vec<String>,
    pub needs_approval: Vec<String>,
}

impl Ctx {
    pub fn rel(&self, p: &Path) -> String {
        p.strip_prefix(&self.results_root).unwrap_or(p).to_string_lossy().replace('\\', "/")
    }
}

/// Converts YAML to JSON values.
pub fn load_script(path: &Path) -> Result<Value> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let v: Value = serde_norway::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(v)
}

/// Replaces `"$name"` strings with bound values, `"@file:rel"` with file contents, and
/// `"@repeat:N:text"` with text repeated N times. Paths starting with `fixtures/` become absolute.
pub fn substitute(v: &Value, ctx: &Ctx) -> Result<Value> {
    Ok(match v {
        Value::String(s) => {
            if let Some(expr) = s.strip_prefix('$') {
                // `$name`, or `$name+K` / `$name-K` for numeric variables.
                let split = expr.find(['+', '-']).filter(|&i| i > 0);
                let (name, delta) = match split {
                    Some(i) => (&expr[..i], expr[i..].parse::<f64>().map_err(|_| anyhow!("bad expression ${expr}"))?),
                    None => (expr, 0.0),
                };
                let v = ctx.vars.get(name).cloned().ok_or_else(|| anyhow!("unbound variable ${name}"))?;
                if split.is_some() {
                    let n = v.as_f64().ok_or_else(|| anyhow!("${name} is not a number"))? + delta;
                    if n.fract() == 0.0 && n >= 0.0 { json!(n as u64) } else { json!(n) }
                } else {
                    v
                }
            } else if let Some(rel) = s.strip_prefix("@file:") {
                let p = ctx.root.join(rel);
                let t = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
                Value::String(t.replace("\r\n", "\n").trim_end_matches('\n').to_string())
            } else if let Some(rest) = s.strip_prefix("@repeat:") {
                let (n, text) = rest.split_once(':').ok_or_else(|| anyhow!("@repeat:N:text"))?;
                let n: usize = n.parse()?;
                Value::String(text.repeat(n))
            } else if s.starts_with("fixtures/") {
                Value::String(ctx.root.join(s).to_string_lossy().to_string())
            } else {
                v.clone()
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(|x| substitute(x, ctx)).collect::<Result<_>>()?),
        Value::Object(m) => {
            let mut o = Map::new();
            for (k, x) in m {
                o.insert(k.clone(), substitute(x, ctx)?);
            }
            Value::Object(o)
        }
        _ => v.clone(),
    })
}

/// Turns `{name: {...}}` into an Action (`{"cmd": name, ...}`).
pub fn to_action(name: &str, args: &Value) -> Result<Action> {
    let mut obj = match args {
        Value::Object(m) => m.clone(),
        Value::Null => Map::new(),
        other => bail!("arguments of {name} must be a map, got {other}"),
    };
    obj.insert("cmd".into(), Value::String(name.into()));
    serde_json::from_value(Value::Object(obj)).map_err(|e| anyhow!("{e}"))
}

pub fn run_query(s: &mut Session, q: &Value) -> Result<Value> {
    let q: Query = serde_json::from_value(q.clone()).map_err(|e| anyhow!("bad query {q}: {e}"))?;
    s.query(&q).map_err(|e| anyhow!("query failed: {e}"))
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64()
}

/// Approximate JSON equality (numbers within 1e-6 relative).
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap_or(f64::NAN), y.as_f64().unwrap_or(f64::NAN));
            (x - y).abs() <= 1e-6 * (1.0 + x.abs().max(y.abs()))
        }
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| json_eq(a, b)),
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map(|w| json_eq(v, w)).unwrap_or(false))
        }
        _ => a == b,
    }
}

/// Applies matcher keys (`equals`, `approx`, `gt`, …) to `actual`.
pub fn check_matchers(actual: &Value, spec: &Map<String, Value>) -> Result<()> {
    let mut any = false;
    for (k, want) in spec {
        let ok = match k.as_str() {
            "equals" => json_eq(actual, want),
            "not_equals" => !json_eq(actual, want),
            "approx" => {
                let arr = want.as_array().ok_or_else(|| anyhow!("approx: [value, tolerance]"))?;
                let (v, tol) = (arr.first().and_then(num).unwrap_or(0.0), arr.get(1).and_then(num).unwrap_or(0.01));
                num(actual).map(|a| (a - v).abs() <= tol).unwrap_or(false)
            }
            "gt" => num(actual).zip(num(want)).map(|(a, b)| a > b).unwrap_or(false),
            "gte" => num(actual).zip(num(want)).map(|(a, b)| a >= b).unwrap_or(false),
            "lt" => num(actual).zip(num(want)).map(|(a, b)| a < b).unwrap_or(false),
            "lte" => num(actual).zip(num(want)).map(|(a, b)| a <= b).unwrap_or(false),
            "contains" => match (actual, want) {
                (Value::String(a), Value::String(w)) => a.contains(w.as_str()),
                (Value::Array(a), w) => a.iter().any(|x| json_eq(x, w)),
                _ => false,
            },
            "not_contains" => match (actual, want) {
                (Value::String(a), Value::String(w)) => !a.contains(w.as_str()),
                (Value::Array(a), w) => !a.iter().any(|x| json_eq(x, w)),
                _ => false,
            },
            "starts_with" => actual.as_str().zip(want.as_str()).map(|(a, w)| a.starts_with(w)).unwrap_or(false),
            "ends_with" => actual.as_str().zip(want.as_str()).map(|(a, w)| a.ends_with(w)).unwrap_or(false),
            "len" => {
                let n = match actual {
                    Value::Array(a) => a.len(),
                    Value::String(s) => s.chars().count(),
                    Value::Object(o) => o.len(),
                    _ => bail!("len on non-collection {actual}"),
                };
                match want {
                    Value::Object(m) => {
                        check_matchers(&json!(n), m)?;
                        true
                    }
                    w => json_eq(&json!(n), w),
                }
            }
            "is_null" => actual.is_null() == want.as_bool().unwrap_or(true),
            "query" | "path" | "message" | "as" => continue,
            other => bail!("unknown matcher {other:?}"),
        };
        any = true;
        if !ok {
            bail!("expected {k} {want}, got {}", short(actual));
        }
    }
    if !any {
        bail!("expect step has no matcher");
    }
    Ok(())
}

pub fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 300 { format!("{}…", s.chars().take(300).collect::<String>()) } else { s }
}

/// JSON pointer lookup with a friendly error.
pub fn pointer<'a>(v: &'a Value, p: &str) -> Result<&'a Value> {
    v.pointer(p).ok_or_else(|| anyhow!("path {p} not found in {}", short(v)))
}

/// Runs one step. Returns Err for a failed expectation or action.
pub fn run_step(s: &mut Session, ctx: &mut Ctx, step: &Value) -> Result<()> {
    let m = step.as_object().ok_or_else(|| anyhow!("step must be a map"))?;
    let bind = m.get("as").cloned();
    let keys: Vec<&String> = m.keys().filter(|k| *k != "as").collect();
    if keys.len() != 1 {
        bail!("step must have exactly one action key (plus optional `as`), got {keys:?}");
    }
    let name = keys[0].as_str();
    let args = substitute(&m[name], ctx)?;
    match name {
        "expect" => {
            let spec = args.as_object().ok_or_else(|| anyhow!("expect needs a map"))?;
            let q = spec.get("query").ok_or_else(|| anyhow!("expect needs a query"))?;
            let mut actual = run_query(s, q)?;
            if let Some(p) = spec.get("path").and_then(|p| p.as_str()) {
                actual = pointer(&actual, p)?.clone();
            }
            check_matchers(&actual, spec).map_err(|e| anyhow!("{} — query {}", e, q))?;
            if let Some(Value::String(var)) = spec.get("as") {
                ctx.vars.insert(var.clone(), actual);
            }
        }
        "dump" => {
            // dump: {query: {...}, file: name.json} — writes a query result for debugging.
            let q = args.get("query").ok_or_else(|| anyhow!("dump needs a query"))?;
            let file = args.get("file").and_then(|f| f.as_str()).unwrap_or("dump.json");
            let v = run_query(s, q)?;
            let p = ctx.out.join(file);
            std::fs::write(&p, serde_json::to_string_pretty(&v)?)?;
            let r = ctx.rel(&p);
            ctx.artifacts.push(r);
        }
        "let" => {
            // let: {name: value-or-query}
            let spec = args.as_object().ok_or_else(|| anyhow!("let needs a map"))?;
            for (k, v) in spec {
                let val = match v.get("q") {
                    Some(_) => run_query(s, v)?,
                    None => v.clone(),
                };
                ctx.vars.insert(k.clone(), val);
            }
        }
        "expect_error" => {
            let spec = args.as_object().ok_or_else(|| anyhow!("expect_error needs a map"))?;
            let (an, aa) =
                spec.iter().find(|(k, _)| *k != "message").ok_or_else(|| anyhow!("expect_error needs an action"))?;
            let action = to_action(an, aa)?;
            match s.run(&action) {
                Ok(_) => bail!("expected {an} to fail, but it succeeded"),
                Err(e) => {
                    if let Some(msg) = spec.get("message").and_then(|m| m.as_str())
                        && !e.to_string().contains(msg)
                    {
                        bail!("expected error containing {msg:?}, got {e}");
                    }
                }
            }
        }
        "expect_pdf" => crate::pdfcheck::check(ctx, &args)?,
        "expect_png" => crate::pngcheck::check(s, ctx, &args)?,
        "expect_image" => crate::pngcheck::check_file(ctx, &args)?,
        "expect_roundtrip" => roundtrip(s, ctx)?,
        "snapshot" => {
            let page = args.get("page").and_then(|p| p.as_u64()).unwrap_or(0) as usize;
            let name = args.get("name").and_then(|p| p.as_str()).unwrap_or("snapshot");
            let dpi = args.get("dpi").and_then(|p| p.as_f64()).unwrap_or(72.0);
            let png = s.page_png(page, dpi).map_err(|e| anyhow!("{e}"))?;
            let p = ctx.out.join(format!("{name}.png"));
            std::fs::write(&p, png)?;
            let r = ctx.rel(&p);
            ctx.artifacts.push(r);
        }
        _ => {
            let action = to_action(name, &args)?;
            let out = s.run(&action).map_err(|e| anyhow!("{name} failed: {e}"))?;
            // Record exported files as artifacts.
            if matches!(name, "export_pdf" | "export_png" | "save")
                && let Some(p) = args.get("path").and_then(|p| p.as_str())
            {
                let full = ctx.out.join(p);
                let r = ctx.rel(&full);
                ctx.artifacts.push(r);
            }
            match bind {
                Some(Value::String(v)) => {
                    let id = out.created.first().ok_or_else(|| anyhow!("`as: {v}` but {name} created nothing"))?;
                    ctx.vars.insert(v, json!(id.0));
                }
                Some(Value::Array(vs)) => {
                    for (i, v) in vs.iter().enumerate() {
                        let v = v.as_str().ok_or_else(|| anyhow!("`as` names must be strings"))?;
                        let id = out.created.get(i).ok_or_else(|| {
                            anyhow!("`as` binds {} ids but {name} created {}", vs.len(), out.created.len())
                        })?;
                        ctx.vars.insert(v.to_string(), json!(id.0));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn roundtrip(s: &mut Session, ctx: &mut Ctx) -> Result<()> {
    let path = ctx.out.join("roundtrip.npub");
    let before = s.query(&Query::Document).map_err(|e| anyhow!("{e}"))?;
    let assets_before: Vec<(u64, Vec<u8>)> = s.doc().assets.values().map(|a| (a.id.0, a.bytes.to_vec())).collect();
    s.run(&to_action("save", &json!({"path": path.to_string_lossy()}))?).map_err(|e| anyhow!("save: {e}"))?;
    let mut fresh = Session::bundled();
    fresh.base_dir = ctx.out.clone();
    fresh.run(&to_action("open", &json!({"path": path.to_string_lossy()}))?).map_err(|e| anyhow!("open: {e}"))?;
    let after = fresh.query(&Query::Document).map_err(|e| anyhow!("{e}"))?;
    if !json_eq(&before, &after) {
        bail!("document changed across save/open:\n before {}\n after  {}", short(&before), short(&after));
    }
    let assets_after: Vec<(u64, Vec<u8>)> = fresh.doc().assets.values().map(|a| (a.id.0, a.bytes.to_vec())).collect();
    if assets_before != assets_after {
        bail!("asset bytes changed across save/open");
    }
    let r = ctx.rel(&path);
    ctx.artifacts.push(r);
    Ok(())
}

/// Runs a whole headless journey script.
pub fn run_journey(script: &Value, root: &Path, results_root: &Path) -> JourneyResult {
    let id = script.get("id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
    let title = script.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let covers: Vec<String> = script
        .get("covers")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let ui = script.get("ui").and_then(|v| v.as_bool()).unwrap_or(false);
    let out = results_root.join(&id);
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::create_dir_all(&out);
    let mut ctx = Ctx {
        id: id.clone(),
        root: root.to_path_buf(),
        out: out.clone(),
        results_root: results_root.to_path_buf(),
        vars: HashMap::new(),
        artifacts: vec![],
        needs_approval: vec![],
    };
    let start = Instant::now();
    let steps: Vec<Value> = script.get("steps").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut result = JourneyResult {
        id: id.clone(),
        title,
        covers,
        ui,
        status: Status::Pass,
        message: String::new(),
        failed_step: None,
        duration_ms: 0,
        artifacts: vec![],
        screenshots: vec![],
    };
    let mut session = Session::bundled();
    session.base_dir = out.clone();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), (usize, anyhow::Error)> {
        if ui {
            crate::ui::run_ui_journey(&steps, &mut ctx).map_err(|e| (e.0, e.1))?;
            return Ok(());
        }
        for (i, step) in steps.iter().enumerate() {
            run_step(&mut session, &mut ctx, step).map_err(|e| (i + 1, e))?;
        }
        Ok(())
    }));
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err((i, e))) => {
            result.status = Status::Fail;
            result.failed_step = Some(i);
            result.message = format!("step {i}: {e:#}");
        }
        Err(p) => {
            result.status = Status::Error;
            result.message = format!(
                "panic: {}",
                p.downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            );
        }
    }
    if matches!(result.status, Status::Pass) && !ctx.needs_approval.is_empty() {
        result.status = Status::NeedsApproval;
        result.message = format!("golden images awaiting approval: {}", ctx.needs_approval.join(", "));
    }
    // Screenshots of the final state for the dashboard gallery.
    if ui {
        if let Ok(rd) = std::fs::read_dir(out.join("screens")) {
            let mut files: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
            files.sort();
            result.screenshots = files.iter().map(|f| ctx.rel(f)).collect();
        }
    } else {
        let shots = out.join("screens");
        let _ = std::fs::create_dir_all(&shots);
        let n = session.doc().pages.len().min(8);
        for p in 0..n {
            if let Ok(png) = session.page_png(p, 48.0) {
                let f = shots.join(format!("page-{}.png", p + 1));
                if std::fs::write(&f, png).is_ok() {
                    result.screenshots.push(ctx.rel(&f));
                }
            }
        }
    }
    result.artifacts = ctx.artifacts.clone();
    result.duration_ms = start.elapsed().as_millis();
    result
}
