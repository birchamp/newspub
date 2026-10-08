//! Guide commands (GD-01, GD-02).

use crate::{Applied, Command, CoreError, Document, GridGuides, Guide, Id};

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    match cmd {
        Command::AddGuide { page, master, orientation, pos } => {
            let (page_id, master_id) = match (page, master) {
                (Some(i), None) => (Some(doc.pages.get(*i).ok_or(CoreError::NoSuchPage(*i))?.id), None),
                (None, Some(m)) => {
                    doc.master_index(*m).ok_or(CoreError::NoSuchMaster(*m))?;
                    (None, Some(*m))
                }
                _ => return Err(CoreError::Invalid("a guide belongs to exactly one of a page or a master".into())),
            };
            if !pos.0.is_finite() {
                return Err(CoreError::Invalid("guide position must be a finite number".into()));
            }
            let id = doc.alloc();
            doc.guides.ruler.push(Guide { id, orientation: *orientation, pos: *pos, page: page_id, master: master_id });
            Ok(Applied { created: vec![id] })
        }
        Command::MoveGuide { guide, pos } => {
            if !pos.0.is_finite() {
                return Err(CoreError::Invalid("guide position must be a finite number".into()));
            }
            let g = doc.guides.ruler.iter_mut().find(|g| g.id == *guide).ok_or_else(|| no_guide(*guide))?;
            g.pos = *pos;
            Ok(Applied::default())
        }
        Command::DeleteGuide { guide } => {
            let i = doc.guides.ruler.iter().position(|g| g.id == *guide).ok_or_else(|| no_guide(*guide))?;
            doc.guides.ruler.remove(i);
            Ok(Applied::default())
        }
        Command::SetGridGuides { columns, rows, gutter } => {
            if !gutter.0.is_finite() || gutter.0 < 0.0 {
                return Err(CoreError::Invalid("grid gutter must be zero or more".into()));
            }
            doc.guides.grid = if *columns <= 1 && *rows <= 1 && gutter.0 == 0.0 {
                None
            } else {
                Some(GridGuides { columns: *columns, rows: *rows, gutter: *gutter })
            };
            Ok(Applied::default())
        }
        _ => Err(CoreError::Unsupported(format!("{cmd:?} is not a guide command"))),
    }
}

fn no_guide(id: Id) -> CoreError {
    CoreError::Invalid(format!("no such guide {}", id.0))
}
