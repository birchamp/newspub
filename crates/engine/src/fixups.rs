//! Engine fixups (ARCHITECTURE.md §4): derived geometry written back into the model after every
//! action, inside the same undo step. Lead-owned.
//!
//! - Text autofit (TF-08): `fit_scale` for shrink-on-overflow / best-fit frames, frame height for grow-frame.

use newpub_core::{Autofit, Document, Id, ObjectKind};
use newpub_layout::FontStore;

pub(crate) fn run(doc: &mut Document, fonts: &FontStore) {
    autofit(doc, fonts);
    table_rows(doc, fonts);
}

/// Grows (and shrinks back to their minimum) table rows so every cell's text fits; syncs the table's rect.
fn table_rows(doc: &mut Document, fonts: &FontStore) {
    let ids: Vec<Id> = doc.objects.values().filter(|o| matches!(o.kind, ObjectKind::Table(_))).map(|o| o.id).collect();
    for id in ids {
        let Some(ObjectKind::Table(t)) = doc.objects.get(&id).map(|o| o.kind.clone()) else { continue };
        let mut heights: Vec<f64> = t.min_row_heights.iter().map(|h| h.0).collect();
        // Single-row cells first, then spanning cells add any deficit to their last row.
        for pass in 0..2 {
            for r in 0..t.rows() {
                for c in 0..t.cols() {
                    let Some(cell) = t.cell(r, c) else { continue };
                    if cell.covered || (cell.rowspan > 1) != (pass == 1) {
                        continue;
                    }
                    let need = newpub_layout::cell_natural_height(doc, fonts, &t, r, c);
                    let span = (r..(r + cell.rowspan as usize).min(t.rows())).map(|k| heights[k]).sum::<f64>();
                    if need > span + 1e-6 {
                        let last = (r + cell.rowspan as usize).min(t.rows()) - 1;
                        heights[last] += need - span;
                    }
                }
            }
        }
        if let Some(o) = doc.objects.get_mut(&id)
            && let ObjectKind::Table(tm) = &mut o.kind
        {
            for (k, h) in heights.iter().enumerate() {
                tm.row_heights[k] = newpub_core::Length((h * 100.0).ceil() / 100.0);
            }
            o.rect.w = tm.col_widths.iter().map(|w| w.0).sum();
            o.rect.h = tm.row_heights.iter().map(|h| h.0).sum();
        }
    }
}

fn overflows(doc: &Document, fonts: &FontStore, story: Id) -> bool {
    newpub_layout::layout_one(doc, fonts, story).map(|(sl, _)| sl.overflow_at.is_some()).unwrap_or(false)
}

fn set_scale(doc: &mut Document, frame: Id, scale: f64) {
    if let Some(o) = doc.objects.get_mut(&frame)
        && let ObjectKind::Text(t) = &mut o.kind
    {
        t.fit_scale = scale;
    }
}

/// Largest value in [lo, hi] for which `fits` holds (assumes monotonic), to ~0.5% precision.
fn search(mut lo: f64, mut hi: f64, mut fits: impl FnMut(f64) -> bool) -> f64 {
    if fits(hi) {
        return hi;
    }
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
        if hi - lo < lo.abs() * 0.002 + 1e-4 {
            break;
        }
    }
    lo
}

fn autofit(doc: &mut Document, fonts: &FontStore) {
    // The first frame of each story carries the story's autofit setting.
    let targets: Vec<(Id, Id, Autofit)> = doc
        .stories
        .values()
        .filter_map(|s| {
            let f = *s.frames.first()?;
            match &doc.objects.get(&f)?.kind {
                ObjectKind::Text(t) if t.autofit != Autofit::None || t.fit_scale != 1.0 => Some((s.id, f, t.autofit)),
                _ => None,
            }
        })
        .collect();
    for (story, frame, mode) in targets {
        match mode {
            Autofit::None => set_scale(doc, frame, 1.0),
            Autofit::ShrinkOnOverflow => {
                set_scale(doc, frame, 1.0);
                if overflows(doc, fonts, story) {
                    let mut probe = doc.clone();
                    let s = search(0.05, 1.0, |s| {
                        set_scale(&mut probe, frame, s);
                        !overflows(&probe, fonts, story)
                    });
                    set_scale(doc, frame, s);
                }
            }
            Autofit::BestFit => {
                let mut probe = doc.clone();
                let s = search(0.05, 20.0, |s| {
                    set_scale(&mut probe, frame, s);
                    !overflows(&probe, fonts, story)
                });
                set_scale(doc, frame, s);
            }
            Autofit::GrowFrame => {
                set_scale(doc, frame, 1.0);
                let Some(o) = doc.objects.get(&frame) else { continue };
                let rect = o.rect;
                let page_h = doc.setup.height.0;
                let mut probe = doc.clone();
                let set_h = |d: &mut Document, h: f64| {
                    if let Some(o) = d.objects.get_mut(&frame) {
                        o.rect.h = h;
                    }
                };
                // Smallest height that fits: search on "does not fit" over the inverted range.
                let max_h = (page_h * 4.0).max(rect.h);
                let fits = |p: &mut Document, h: f64| {
                    set_h(p, h);
                    !overflows(p, fonts, story)
                };
                let h = if !fits(&mut probe, max_h) {
                    max_h
                } else {
                    let (mut lo, mut hi) = (1.0, max_h);
                    for _ in 0..30 {
                        let mid = (lo + hi) / 2.0;
                        if fits(&mut probe, mid) {
                            hi = mid;
                        } else {
                            lo = mid;
                        }
                        if hi - lo < 0.05 {
                            break;
                        }
                    }
                    hi
                };
                set_h(doc, (h * 100.0).ceil() / 100.0);
            }
        }
    }
}
