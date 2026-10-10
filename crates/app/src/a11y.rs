//! Screen-reader names for page objects (AX-04).

use newpub_engine::core::{Document, Object, ObjectKind, ShapeKind};

/// Accessible name of a top-level object.
pub fn object_name(doc: &Document, o: &Object) -> String {
    match &o.kind {
        ObjectKind::Text(_) => {
            let story = doc.story_of(o.id).ok().and_then(|s| doc.story(s).ok());
            let text = story.map(|s| s.text.as_str()).unwrap_or_default();
            let first: String = text.chars().take(40).map(|c| if c == '\n' { ' ' } else { c }).collect();
            // A box of a story that runs through several boxes says where it sits in the chain.
            let place = story
                .filter(|s| s.frames.len() > 1)
                .and_then(|s| s.frames.iter().position(|f| *f == o.id).map(|k| (k + 1, s.frames.len())));
            match place {
                Some((k, n)) => format!("Text box {k} of {n}: {first}"),
                None => format!("Text box: {first}"),
            }
        }
        ObjectKind::Image(_) => {
            let alt = o.alt_text.as_deref().map(str::trim).filter(|a| !a.is_empty()).unwrap_or("no alt text");
            format!("Picture: {alt}")
        }
        ObjectKind::Shape(s) => format!("Shape: {}", shape_name(&s.kind)),
        ObjectKind::Table(t) => format!("Table: {}\u{d7}{}", t.row_heights.len(), t.col_widths.len()),
        ObjectKind::WordArt(w) => format!("WordArt: {}", w.text.chars().take(40).collect::<String>()),
        ObjectKind::Group { .. } => "Group".to_string(),
    }
}

fn shape_name(k: &ShapeKind) -> String {
    match k {
        ShapeKind::Rect => "rectangle".into(),
        ShapeKind::RoundRect { .. } => "rounded rectangle".into(),
        ShapeKind::Ellipse => "ellipse".into(),
        ShapeKind::Line => "line".into(),
        ShapeKind::Triangle => "triangle".into(),
        ShapeKind::Star { .. } => "star".into(),
        other => format!("{other:?}").split(|c: char| !c.is_alphanumeric()).next().unwrap_or("shape").to_lowercase(),
    }
}
