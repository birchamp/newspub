//! PDF-related queries (PR-04, PR-05, EX-05).

use crate::{EngineError, Query, Session};
use newpub_core::{Color, Link, ObjectKind};
use serde_json::{Value, json};

/// Records `c` once. Spot inks are identified by name alone: tints and CMYK alternates are per-use, not per-ink.
fn note(out: &mut Vec<Value>, c: &Color) {
    let v = match c {
        Color::Spot { name, .. } => json!({ "space": "spot", "name": name }),
        other => json!(other),
    };
    if !out.contains(&v) {
        out.push(v);
    }
}

impl Session {
    pub(crate) fn pdf_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::ColorsUsed => Ok(self.colors_used()),
            Query::NUpLayout { sheet_width, sheet_height, gap } => {
                let (w, h) = (self.doc.setup.width.0, self.doc.setup.height.0);
                let (columns, rows) = newpub_io_pdf::impose::n_up_grid(sheet_width.0, sheet_height.0, w, h, gap.0);
                Ok(json!({ "columns": columns, "rows": rows, "per_sheet": columns * rows }))
            }
            Query::Hyperlinks => Ok(self.hyperlinks()),
            other => Err(EngineError::Other(format!("query {other:?} is not a PDF query"))),
        }
    }

    fn colors_used(&self) -> Value {
        let mut out: Vec<Value> = vec![];
        let doc = &self.doc;
        for p in &doc.pages {
            if let Some(c) = &p.background {
                note(&mut out, c);
            }
        }
        for m in &doc.masters {
            if let Some(c) = &m.background {
                note(&mut out, c);
            }
        }
        for o in doc.objects.values() {
            match &o.kind {
                ObjectKind::Shape(s) => {
                    if let Some(g) = &s.gradient {
                        for st in &g.stops {
                            note(&mut out, &st.color);
                        }
                    } else if let Some(c) = &s.fill {
                        note(&mut out, c);
                    }
                    if let Some(st) = &s.stroke {
                        note(&mut out, &st.color);
                    }
                }
                ObjectKind::Text(t) => {
                    if let Some(c) = &t.fill {
                        note(&mut out, c);
                    }
                    if let Some(st) = &t.stroke {
                        note(&mut out, &st.color);
                    }
                }
                ObjectKind::Image(i) => {
                    if let Some(st) = &i.stroke {
                        note(&mut out, &st.color);
                    }
                }
                ObjectKind::Table(tb) => {
                    for cell in &tb.cells {
                        if let Some(c) = &cell.fill {
                            note(&mut out, c);
                        }
                        let b = &cell.borders;
                        for st in [&b.top, &b.bottom, &b.left, &b.right].into_iter().flatten() {
                            note(&mut out, &st.color);
                        }
                    }
                }
                ObjectKind::WordArt(w) => {
                    if let Some(g) = &w.gradient {
                        for st in &g.stops {
                            note(&mut out, &st.color);
                        }
                    } else {
                        note(&mut out, &w.fill);
                    }
                    if let Some(st) = &w.outline {
                        note(&mut out, &st.color);
                    }
                }
                ObjectKind::Group { .. } => {}
            }
        }
        for story in doc.stories.values() {
            if story.is_empty() {
                continue;
            }
            for (r, run) in story.runs() {
                let para = story.paras.get(story.para_index_at(r.start)).cloned().unwrap_or_default();
                note(&mut out, &doc.resolve_char(&para, run).color);
            }
        }
        Value::Array(out)
    }

    fn hyperlinks(&self) -> Value {
        let doc = &self.doc;
        let mut out = vec![];
        for (sid, story) in &doc.stories {
            // Merge adjacent runs that carry the same link.
            let mut cur: Option<(usize, usize, Link)> = None;
            let flush = |cur: &mut Option<(usize, usize, Link)>, out: &mut Vec<Value>| {
                if let Some((s, e, l)) = cur.take() {
                    out.push(match l {
                        Link::Url(u) => json!({ "story": sid, "start": s, "end": e, "url": u }),
                        Link::Page(p) => {
                            let page = doc.pages.iter().position(|pg| pg.id == p);
                            json!({ "story": sid, "start": s, "end": e, "page": page })
                        }
                    });
                }
            };
            for (r, run) in story.runs() {
                match (&run.link, cur.as_mut()) {
                    (Some(l), Some(c)) if *l == c.2 && c.1 == r.start => c.1 = r.end,
                    (Some(l), _) => {
                        flush(&mut cur, &mut out);
                        cur = Some((r.start, r.end, l.clone()));
                    }
                    (None, _) => flush(&mut cur, &mut out),
                }
            }
            flush(&mut cur, &mut out);
        }
        Value::Array(out)
    }
}
