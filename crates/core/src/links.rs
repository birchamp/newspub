//! Hyperlinks, bookmarks, reading order (EX-05, AX-03).

use crate::{Applied, Bookmark, Command, CoreError, Document, Link};

fn page_id(doc: &Document, index: usize) -> Result<crate::Id, CoreError> {
    doc.pages.get(index).map(|p| p.id).ok_or(CoreError::NoSuchPage(index))
}

pub(crate) fn apply(doc: &mut Document, cmd: &Command) -> Result<Applied, CoreError> {
    match cmd {
        Command::SetHyperlink { target, start, end, url, page } => {
            let link = match (url, page) {
                (Some(_), Some(_)) => return Err(CoreError::Invalid("give either a url or a page, not both".into())),
                (Some(u), None) if u.trim().is_empty() => return Err(CoreError::Invalid("empty url".into())),
                (Some(u), None) => Some(Link::Url(u.clone())),
                (None, Some(p)) => Some(Link::Page(page_id(doc, *p)?)),
                (None, None) => None,
            };
            let sid = doc.story_of(*target)?;
            let len = doc.story(sid)?.len();
            if start > end || *end > len {
                return Err(CoreError::BadRange { start: *start, end: *end, len });
            }
            let story = doc.story_mut(sid)?;
            let pieces: Vec<_> = story
                .runs()
                .filter_map(|(r, a)| {
                    let (s, e) = (r.start.max(*start), r.end.min(*end));
                    (s < e).then(|| (s..e, a.clone()))
                })
                .collect();
            for (r, mut a) in pieces {
                a.link = link.clone();
                story.set_chars(r, &a)?;
            }
            Ok(Applied::default())
        }
        Command::AddBookmark { title, page } => {
            if title.trim().is_empty() {
                return Err(CoreError::Invalid("empty bookmark title".into()));
            }
            let page = page_id(doc, *page)?;
            doc.bookmarks.push(Bookmark { title: title.clone(), page });
            Ok(Applied::default())
        }
        Command::RemoveBookmark { index } => {
            if *index >= doc.bookmarks.len() {
                return Err(CoreError::Invalid(format!("no bookmark {index}")));
            }
            doc.bookmarks.remove(*index);
            Ok(Applied::default())
        }
        Command::SetReadingOrder { page, ids } => {
            let p = doc.pages.get(*page).ok_or(CoreError::NoSuchPage(*page))?;
            for id in ids {
                if !p.objects.contains(id) {
                    return Err(CoreError::Invalid(format!("{id} is not a top-level object of page {page}")));
                }
            }
            let pid = p.id;
            if ids.is_empty() {
                doc.reading_order.remove(&pid);
            } else {
                doc.reading_order.insert(pid, ids.clone());
            }
            Ok(Applied::default())
        }
        other => Err(CoreError::Unsupported(format!("{other:?} is not a link command"))),
    }
}
