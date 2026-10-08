//! `expect_html`: assertions on exported HTML — `{dir, files, page, title, text_contains, images_with_alt, links_to}`.
//! Checks are on the raw HTML source (text must appear HTML-escaped as the browser would read it).

use crate::runner::Ctx;
use anyhow::{Result, anyhow, bail};
use serde_json::Value;

fn strs(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

/// Values of `attr="…"` on tags named `tag`.
fn attrs(html: &str, tag: &str, attr: &str) -> Vec<String> {
    let mut out = vec![];
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let mut i = 0;
    while let Some(p) = lower[i..].find(&open) {
        let start = i + p;
        let end = lower[start..].find('>').map(|e| start + e).unwrap_or(lower.len());
        let tag_src = &html[start..end];
        let key = format!("{attr}=\"");
        if let Some(a) = tag_src.to_ascii_lowercase().find(&key) {
            let rest = &tag_src[a + key.len()..];
            if let Some(q) = rest.find('"') {
                out.push(rest[..q].to_string());
            }
        }
        i = end.max(start + 1);
    }
    out
}

pub fn check(ctx: &mut Ctx, spec: &Value) -> Result<()> {
    let dir = ctx.out.join(spec.get("dir").and_then(|d| d.as_str()).ok_or_else(|| anyhow!("expect_html needs dir"))?);
    for f in strs(spec.get("files")) {
        if !dir.join(&f).is_file() {
            bail!("HTML export is missing {f}");
        }
    }
    let Some(page) = spec.get("page").and_then(|p| p.as_str()) else { return Ok(()) };
    let path = dir.join(page);
    let html = std::fs::read_to_string(&path).map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    if let Some(t) = spec.get("title").and_then(|t| t.as_str()) {
        let want = format!("<title>{t}</title>");
        if !html.contains(&want) {
            bail!("{page} has no {want}");
        }
    }
    for t in strs(spec.get("text_contains")) {
        if !html.contains(&t) {
            bail!("{page} does not contain {t:?}");
        }
    }
    let alts = attrs(&html, "img", "alt");
    for a in strs(spec.get("images_with_alt")) {
        if !alts.contains(&a) {
            bail!("{page} has no <img alt={a:?}> (alts: {alts:?})");
        }
    }
    let hrefs = attrs(&html, "a", "href");
    for l in strs(spec.get("links_to")) {
        if !hrefs.contains(&l) {
            bail!("{page} has no link to {l:?} (links: {hrefs:?})");
        }
    }
    let rel = ctx.rel(&path);
    ctx.artifacts.push(rel);
    Ok(())
}
