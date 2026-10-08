//! Linked pictures, relink/embed, off-page report (IM-04, IM-08, PG-10).

use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::{Id, Rect};
use serde_json::{Value, json};
use std::sync::Arc;

impl Session {
    pub(crate) fn pictures_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        use SessionAction::*;
        match a {
            RelinkPicture { asset, path } => {
                let p = self.resolve(path);
                let bytes = std::fs::read(&p)?;
                let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
                    .with_guessed_format()
                    .map_err(|e| EngineError::Image(e.to_string()))?;
                let mime = match reader.format() {
                    Some(image::ImageFormat::Png) => "image/png",
                    Some(image::ImageFormat::Jpeg) => "image/jpeg",
                    other => return Err(EngineError::Image(format!("unsupported picture format {other:?}"))),
                };
                let (pw, ph) = reader.into_dimensions().map_err(|e| EngineError::Image(e.to_string()))?;
                let mut d = self.doc.clone();
                let entry = d.assets.get_mut(asset).ok_or_else(|| no_asset(*asset))?;
                entry.link = Some(p.to_string_lossy().to_string());
                entry.mime = mime.to_string();
                entry.px_w = pw;
                entry.px_h = ph;
                entry.bytes = Arc::from(bytes);
                self.commit(d, None);
                Ok(Outcome::default())
            }
            EmbedPicture { asset } => {
                let mut d = self.doc.clone();
                let entry = d.assets.get_mut(asset).ok_or_else(|| no_asset(*asset))?;
                if entry.link.is_some() && entry.bytes.is_empty() {
                    return Err(EngineError::Other("the picture file is missing; relink it before embedding".into()));
                }
                entry.link = None;
                self.commit(d, None);
                Ok(Outcome::default())
            }
            _ => Err(EngineError::Other(format!("{a:?} is not a picture action"))),
        }
    }

    pub(crate) fn pictures_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        use Query::*;
        match q {
            MissingLinks => Ok(Value::Array(
                self.doc
                    .assets
                    .values()
                    .filter(|a| a.bytes.is_empty())
                    .filter_map(|a| a.link.as_ref().map(|p| json!({"asset": a.id, "path": p})))
                    .collect(),
            )),
            NpubEntries { path } => {
                let p = self.resolve(path);
                let names = newpub_io_native::entries(&p)?;
                Ok(Value::Array(names.into_iter().map(Value::String).collect()))
            }
            OffPageObjects => {
                let (w, h) = (self.doc.setup.width.pt(), self.doc.setup.height.pt());
                let mut out = Vec::new();
                for (index, page) in self.doc.pages.iter().enumerate() {
                    for id in &page.objects {
                        let Some(obj) = self.doc.objects.get(id) else { continue };
                        if entirely_outside(&obj.rect, w, h) {
                            out.push(json!({"id": id, "page": index}));
                        }
                    }
                }
                Ok(Value::Array(out))
            }
            _ => Err(EngineError::Other(format!("query {q:?} is not a picture query"))),
        }
    }
}

fn no_asset(id: Id) -> EngineError {
    EngineError::Other(format!("no picture with id {}", id.0))
}

/// True when the rect has no area in common with the page box `0..w × 0..h`.
fn entirely_outside(r: &Rect, w: f64, h: f64) -> bool {
    r.x + r.w <= 0.0 || r.x >= w || r.y + r.h <= 0.0 || r.y >= h
}
