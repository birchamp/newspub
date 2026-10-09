//! Tagged PDF (AX-03): the display list's structure markers become a krilla tag tree in reading order.
//!
//! Paragraphs are P or H1–H6 (from "Heading N" / "Title" paragraph styles), pictures and described shapes are
//! Figures with their alt text, table cells are Table/TR/TD. Everything else (decorative objects, backgrounds,
//! master-page content, borders, crop marks) is marked as an artifact.

use krilla::surface::Surface;
use krilla::tagging::{Artifact, ArtifactType, ContentTag, Identifier, Node, SpanTag, Tag, TagGroup, TagKind, TagTree};
use newpub_core::{Document, Id};
use newpub_render::{Role, TagKey};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;

enum Child {
    Node(usize),
    Leaf(Identifier),
}

struct NodeB {
    role: Role,
    children: Vec<Child>,
}

#[derive(Default)]
pub(crate) struct Tagger {
    nodes: Vec<NodeB>,
    index: HashMap<TagKey, usize>,
    /// Root elements: (page index, owning top-level object, node).
    roots: Vec<(usize, Id, usize)>,
    page: usize,
    page_objects: HashSet<Id>,
    /// Inside a marked-content sequence opened by `begin`.
    open: bool,
    /// Structure element receiving the open sequence.
    leaf: Option<usize>,
    /// Headings in document order: (level, text, page index).
    pub(crate) headings: Vec<(u16, String, usize)>,
}

impl Tagger {
    pub(crate) fn set_page(&mut self, doc: &Document, page: usize) {
        self.page = page;
        self.page_objects = doc.pages.get(page).map(|p| p.objects.iter().copied().collect()).unwrap_or_default();
    }

    fn node(&mut self, key: TagKey, role: &Role, parent: Option<usize>, owner: Id) -> usize {
        if let Some(i) = self.index.get(&key) {
            return *i;
        }
        let i = self.nodes.len();
        if let Role::Heading(level, text) = role {
            self.headings.push((*level, text.clone(), self.page));
        }
        self.nodes.push(NodeB { role: role.clone(), children: vec![] });
        self.index.insert(key, i);
        match parent {
            Some(p) => self.nodes[p].children.push(Child::Node(i)),
            None => self.roots.push((self.page, owner, i)),
        }
        i
    }

    /// Opens marked content for the structure element `path` (outermost first). Content of objects that are
    /// not on the page itself (master pages) stays an artifact.
    pub(crate) fn begin(&mut self, s: &mut Surface, owner: Id, path: &[(TagKey, Role)]) {
        self.end(s);
        if path.is_empty() || !self.page_objects.contains(&owner) {
            return;
        }
        let mut parent = None;
        for (key, role) in path {
            parent = Some(self.node(*key, role, parent, owner));
        }
        let Some(leaf) = parent else { return };
        let tag = match self.nodes[leaf].role {
            Role::Figure(_) => ContentTag::Other,
            _ => ContentTag::Span(SpanTag::empty()),
        };
        let id = s.start_tagged(tag);
        self.nodes[leaf].children.push(Child::Leaf(id));
        self.open = true;
        self.leaf = Some(leaf);
    }

    /// Switches the open sequence to a span whose /ActualText is `text` (same structure element).
    pub(crate) fn actual_text(&mut self, s: &mut Surface, text: &str) {
        let Some(leaf) = self.leaf.filter(|_| self.open) else { return };
        s.end_tagged();
        let id = s.start_tagged(ContentTag::Span(SpanTag::empty().with_actual_text(Some(text))));
        self.nodes[leaf].children.push(Child::Leaf(id));
    }

    /// Continues the structure element with a plain span after `actual_text`.
    pub(crate) fn resume(&mut self, s: &mut Surface) {
        let Some(leaf) = self.leaf.filter(|_| self.open) else { return };
        s.end_tagged();
        let id = s.start_tagged(ContentTag::Span(SpanTag::empty()));
        self.nodes[leaf].children.push(Child::Leaf(id));
    }

    pub(crate) fn end(&mut self, s: &mut Surface) {
        if self.open {
            s.end_tagged();
            self.open = false;
            self.leaf = None;
        }
    }

    /// Records a tagged link annotation of object `owner` on the current page.
    pub(crate) fn add_link(&mut self, owner: Id, annot: Identifier) {
        let i = self.nodes.len();
        self.nodes.push(NodeB { role: Role::Link, children: vec![Child::Leaf(annot)] });
        self.roots.push((self.page, owner, i));
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn start_artifact(s: &mut Surface) {
        s.start_tagged(ContentTag::Artifact(Artifact::new(ArtifactType::Other, None)));
    }

    fn group(&self, i: usize) -> TagGroup {
        let n = &self.nodes[i];
        let kind: TagKind = match &n.role {
            Role::Heading(l, text) => {
                Tag::Hn(NonZeroU16::new(*l).unwrap_or(NonZeroU16::MIN), Some(text.clone())).into()
            }
            Role::P => Tag::P.into(),
            Role::Figure(alt) => Tag::Figure(alt.clone()).into(),
            Role::Table => Tag::Table.into(),
            Role::Row => Tag::TR.into(),
            Role::Cell => Tag::TD.into(),
            Role::HeaderCell => Tag::TH(krilla::tagging::TableHeaderScope::Column).into(),
            Role::Link => Tag::Link.into(),
        };
        let mut g = TagGroup::new(kind);
        for c in &n.children {
            match c {
                Child::Leaf(id) => g.push(*id),
                Child::Node(k) => g.push(self.group(*k)),
            }
        }
        g
    }

    /// The tag tree: pages in order; on each page the objects in the page's reading order (objects not
    /// listed follow in drawing order).
    pub(crate) fn tree(self, doc: &Document, lang: Option<String>) -> TagTree {
        let mut tree = TagTree::new().with_lang(lang);
        let mut pages: Vec<usize> = self.roots.iter().map(|r| r.0).collect();
        pages.dedup();
        let mut seen_pages = HashSet::new();
        for p in pages {
            if !seen_pages.insert(p) {
                continue;
            }
            let order: Vec<Id> =
                doc.pages.get(p).and_then(|pg| doc.reading_order.get(&pg.id).cloned()).unwrap_or_default();
            let rank = |o: &Id| order.iter().position(|x| x == o).unwrap_or(usize::MAX);
            let mut roots: Vec<(usize, &(usize, Id, usize))> =
                self.roots.iter().enumerate().filter(|(_, r)| r.0 == p).collect();
            // Stable: reading-order rank, then first appearance.
            roots.sort_by_key(|(i, r)| (rank(&r.1), *i));
            for (_, r) in roots {
                tree.push(Node::from(self.group(r.2)));
            }
        }
        tree
    }
}
