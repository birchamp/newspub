//! File and archive checks: `expect_files` (a folder's contents) and `expect_zip` (EPUB, XPS and other zips).

use crate::runner::Ctx;
use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};

fn strs(v: Option<&Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

fn resolve(ctx: &Ctx, p: &str) -> PathBuf {
    let p = Path::new(p);
    if p.is_absolute() { p.to_path_buf() } else { ctx.out.join(p) }
}

/// Every file under `dir`, as `/`-separated paths relative to it.
fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) -> Result<()> {
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let p = e.path();
        if p.is_dir() {
            walk(&p, base, out)?;
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push(rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"));
        }
    }
    Ok(())
}

/// `expect_files: {dir, contains: [paths], not_contains: [paths], ext, count}` (paths relative to `dir`,
/// `/`-separated; `count` counts the files, only those ending in `.<ext>` when `ext` is given). A missing folder
/// counts as empty.
pub fn check_files(ctx: &Ctx, spec: &Value) -> Result<()> {
    let dir = spec.get("dir").and_then(|d| d.as_str()).ok_or_else(|| anyhow!("expect_files needs dir"))?;
    let dir = resolve(ctx, dir);
    let mut files = vec![];
    if dir.is_dir() {
        walk(&dir, &dir, &mut files)?;
    } else if spec.get("count").is_none() {
        bail!("{} is not a folder", dir.display());
    }
    files.sort();
    if let Some(want) = spec.get("count").and_then(|c| c.as_u64()) {
        let ext = spec.get("ext").and_then(|e| e.as_str()).map(|e| format!(".{e}"));
        let n = files.iter().filter(|f| ext.as_ref().is_none_or(|e| f.ends_with(e.as_str()))).count() as u64;
        if n != want {
            bail!(
                "expected {want} files{}, found {n}: {files:?}",
                ext.map(|e| format!(" ending {e}")).unwrap_or_default()
            );
        }
    }
    for want in strs(spec.get("contains")) {
        if !files.contains(&want) {
            bail!("folder has no {want:?} (it has {files:?})");
        }
    }
    for not in strs(spec.get("not_contains")) {
        if files.contains(&not) {
            bail!("folder should not contain {not:?}");
        }
    }
    Ok(())
}

/// `expect_zip: {file, first: name, entries: [names], text: [{entry, equals | contains}]}`.
pub fn check_zip(ctx: &Ctx, spec: &Value) -> Result<()> {
    let file = spec.get("file").and_then(|f| f.as_str()).ok_or_else(|| anyhow!("expect_zip needs file"))?;
    let p = resolve(ctx, file);
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&p)?).map_err(|e| anyhow!("{}: {e}", p.display()))?;
    let names: Vec<String> =
        (0..zip.len()).filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string())).collect();
    if let Some(first) = spec.get("first").and_then(|f| f.as_str()) {
        if names.first().map(String::as_str) != Some(first) {
            bail!("first entry is {:?}, expected {first:?}", names.first());
        }
        let stored = zip.by_index(0).map(|f| f.compression() == zip::CompressionMethod::Stored).unwrap_or(false);
        if !stored {
            bail!("first entry {first:?} must be stored uncompressed");
        }
    }
    for want in strs(spec.get("entries")) {
        if !names.contains(&want) {
            bail!("archive has no entry {want:?} (it has {names:?})");
        }
    }
    if let Some(Value::Array(list)) = spec.get("text") {
        for t in list {
            let entry = t.get("entry").and_then(|e| e.as_str()).ok_or_else(|| anyhow!("text check needs entry"))?;
            let mut f = zip.by_name(entry).map_err(|_| anyhow!("archive has no entry {entry:?}"))?;
            let mut text = String::new();
            f.read_to_string(&mut text).map_err(|e| anyhow!("{entry}: {e}"))?;
            if let Some(eq) = t.get("equals").and_then(|e| e.as_str())
                && text != eq
            {
                bail!("{entry} is {text:?}, expected {eq:?}");
            }
            if let Some(c) = t.get("contains").and_then(|e| e.as_str())
                && !text.contains(c)
            {
                bail!("{entry} does not contain {c:?}");
            }
        }
    }
    Ok(())
}
