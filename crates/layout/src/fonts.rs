//! Font store: bundled fonts (always), system fonts (app only), and face metrics.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Index of a loaded face in a [`FontStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceId(pub u32);

pub type FontData = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// `has_glyph` results keyed by face data address, length, face index and char.
type GlyphCache = std::collections::HashMap<(usize, usize, u32, char), bool>;

/// A loaded face with metrics normalised to 1 em.
pub struct Face {
    pub id: FaceId,
    pub data: FontData,
    pub index: u32,
    pub family: String,
    pub postscript_name: String,
    pub bold: bool,
    pub italic: bool,
    pub units_per_em: f64,
    /// Typographic metrics in em units (descender is negative).
    pub ascender: f64,
    pub descender: f64,
    pub line_gap: f64,
    pub cap_height: f64,
    pub x_height: f64,
    pub underline_pos: f64,
    pub underline_thickness: f64,
    pub strikeout_pos: f64,
}

impl Face {
    pub fn bytes(&self) -> &[u8] {
        (*self.data).as_ref()
    }
    pub fn ttf(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(self.bytes(), self.index).ok()
    }
    pub fn has_glyph(&self, c: char) -> bool {
        // Cached per face data and char: parsing the face for every lookup dominated layout time.
        static CACHE: std::sync::LazyLock<std::sync::Mutex<GlyphCache>> = std::sync::LazyLock::new(Default::default);
        let key = (self.bytes().as_ptr() as usize, self.bytes().len(), self.index, c);
        if let Some(v) = CACHE.lock().ok().and_then(|m| m.get(&key).copied()) {
            return v;
        }
        let v = self.ttf().and_then(|f| f.glyph_index(c)).is_some();
        if let Ok(mut m) = CACHE.lock() {
            if m.len() > 200_000 {
                m.clear();
            }
            m.insert(key, v);
        }
        v
    }
    /// Natural line height in em.
    pub fn line_height(&self) -> f64 {
        self.ascender - self.descender + self.line_gap
    }
}

macro_rules! bundled {
    ($($file:literal),* $(,)?) => {
        &[$( ($file, include_bytes!(concat!("../../../assets/fonts/", $file)) as &[u8]) ),*]
    };
}

/// Fonts shipped with newpub (OFL / Bitstream Vera licences, see assets/fonts/licenses).
pub const BUNDLED: &[(&str, &[u8])] = bundled![
    "Carlito-Regular.ttf",
    "Carlito-Bold.ttf",
    "Carlito-Italic.ttf",
    "Carlito-BoldItalic.ttf",
    "LiberationSerif-Regular.ttf",
    "LiberationSerif-Bold.ttf",
    "LiberationSerif-Italic.ttf",
    "LiberationSerif-BoldItalic.ttf",
    "LiberationSans-Regular.ttf",
    "LiberationSans-Bold.ttf",
    "LiberationSans-Italic.ttf",
    "LiberationSans-BoldItalic.ttf",
    "DejaVuSans.ttf",
    "DejaVuSans-Bold.ttf",
];

/// Families tried, in order, for characters the requested font lacks.
pub const FALLBACK_FAMILIES: &[&str] = &["Carlito", "DejaVu Sans", "Liberation Sans"];

struct Inner {
    faces: Vec<Arc<Face>>,
    by_db: HashMap<fontdb::ID, FaceId>,
    resolved: HashMap<(String, bool, bool), FaceId>,
}

pub struct FontStore {
    db: fontdb::Database,
    inner: RwLock<Inner>,
}

impl FontStore {
    /// Only the bundled fonts: identical results on every OS (used by journeys).
    pub fn bundled() -> FontStore {
        let mut db = fontdb::Database::new();
        for (_, bytes) in BUNDLED {
            db.load_font_source(fontdb::Source::Binary(Arc::new(*bytes)));
        }
        FontStore::from_db(db)
    }

    /// Bundled fonts plus the system's installed fonts (used by the app).
    pub fn with_system() -> FontStore {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        for (_, bytes) in BUNDLED {
            db.load_font_source(fontdb::Source::Binary(Arc::new(*bytes)));
        }
        FontStore::from_db(db)
    }

    fn from_db(db: fontdb::Database) -> FontStore {
        FontStore { db, inner: RwLock::new(Inner { faces: vec![], by_db: HashMap::new(), resolved: HashMap::new() }) }
    }

    /// Adds font bytes (e.g. a font embedded in a document).
    /// Number of font faces known (changes when fonts are added).
    pub fn face_count(&self) -> usize {
        self.db.len()
    }

    pub fn add_font(&mut self, bytes: Vec<u8>) {
        self.db.load_font_data(bytes);
        if let Ok(mut inner) = self.inner.write() {
            inner.resolved.clear();
        }
    }

    /// Sorted, de-duplicated family names available.
    pub fn families(&self) -> Vec<String> {
        let mut v: Vec<String> = self.db.faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
        v.sort();
        v.dedup();
        v
    }

    pub fn has_family(&self, family: &str) -> bool {
        self.db.faces().any(|f| f.families.iter().any(|(n, _)| n.eq_ignore_ascii_case(family)))
    }

    pub fn face(&self, id: FaceId) -> Arc<Face> {
        let inner = self.inner.read().expect("font store lock");
        inner.faces[id.0 as usize].clone()
    }

    /// Resolves a family and style to a face, falling back to the default font.
    pub fn resolve(&self, family: &str, bold: bool, italic: bool) -> FaceId {
        let key = (family.to_ascii_lowercase(), bold, italic);
        if let Some(id) = self.inner.read().ok().and_then(|i| i.resolved.get(&key).copied()) {
            return id;
        }
        let mut candidates: Vec<&str> = vec![family];
        candidates.extend(FALLBACK_FAMILIES);
        let mut found = None;
        for fam in candidates {
            let q = fontdb::Query {
                families: &[fontdb::Family::Name(fam)],
                weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
                stretch: fontdb::Stretch::Normal,
                style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
            };
            if let Some(dbid) = self.db.query(&q) {
                found = Some(dbid);
                break;
            }
        }
        let dbid = found.or_else(|| self.db.faces().next().map(|f| f.id)).expect("at least one font is bundled");
        let id = self.load(dbid);
        if let Ok(mut inner) = self.inner.write() {
            inner.resolved.insert(key, id);
        }
        id
    }

    /// Finds a face that has a glyph for `c`, preferring the fallback families.
    pub fn fallback_for(&self, c: char, bold: bool, italic: bool) -> Option<FaceId> {
        for fam in FALLBACK_FAMILIES {
            let id = self.resolve(fam, bold, italic);
            if self.face(id).has_glyph(c) {
                return Some(id);
            }
        }
        let ids: Vec<fontdb::ID> = self.db.faces().map(|f| f.id).collect();
        for dbid in ids {
            let hit = self
                .db
                .with_face_data(dbid, |data, index| {
                    ttf_parser::Face::parse(data, index).ok().and_then(|f| f.glyph_index(c)).is_some()
                })
                .unwrap_or(false);
            if hit {
                return Some(self.load(dbid));
            }
        }
        None
    }

    fn load(&self, dbid: fontdb::ID) -> FaceId {
        if let Some(id) = self.inner.read().ok().and_then(|i| i.by_db.get(&dbid).copied()) {
            return id;
        }
        let (source, index) = self.db.face_source(dbid).expect("face exists");
        let data: FontData = match source {
            fontdb::Source::Binary(b) => b,
            fontdb::Source::File(p) => Arc::new(std::fs::read(p).unwrap_or_default()),
            fontdb::Source::SharedFile(_, b) => b,
        };
        let info = self.db.face(dbid).expect("face exists");
        let family = info.families.first().map(|(n, _)| n.clone()).unwrap_or_default();
        let mut inner = self.inner.write().expect("font store lock");
        let id = FaceId(inner.faces.len() as u32);
        let face = build_face(
            id,
            data,
            index,
            family,
            info.post_script_name.clone(),
            info.weight.0 >= 600,
            info.style != fontdb::Style::Normal,
        );
        inner.faces.push(Arc::new(face));
        inner.by_db.insert(dbid, id);
        id
    }
}

fn build_face(id: FaceId, data: FontData, index: u32, family: String, ps: String, bold: bool, italic: bool) -> Face {
    let mut f = Face {
        id,
        data,
        index,
        family,
        postscript_name: ps,
        bold,
        italic,
        units_per_em: 1000.0,
        ascender: 0.8,
        descender: -0.2,
        line_gap: 0.0,
        cap_height: 0.7,
        x_height: 0.5,
        underline_pos: -0.1,
        underline_thickness: 0.05,
        strikeout_pos: 0.3,
    };
    if let Ok(t) = ttf_parser::Face::parse((*f.data).as_ref(), index) {
        let upem = t.units_per_em() as f64;
        f.units_per_em = upem;
        // Prefer the Windows metrics Publisher uses for line height when present.
        let (asc, desc, gap) = match t.tables().os2 {
            Some(os2) if os2.windows_ascender() > 0 => {
                let wa = os2.windows_ascender() as f64;
                let wd = os2.windows_descender() as f64; // positive in OS/2, but ttf-parser negates
                (wa, -wd.abs(), 0.0)
            }
            _ => (t.ascender() as f64, t.descender() as f64, t.line_gap() as f64),
        };
        f.ascender = asc / upem;
        f.descender = desc / upem;
        f.line_gap = gap / upem;
        f.cap_height = t.capital_height().map(|v| v as f64 / upem).unwrap_or(f.ascender * 0.7);
        f.x_height = t.x_height().map(|v| v as f64 / upem).unwrap_or(f.ascender * 0.5);
        if let Some(u) = t.underline_metrics() {
            f.underline_pos = u.position as f64 / upem;
            f.underline_thickness = (u.thickness as f64 / upem).max(0.02);
        }
        if let Some(s) = t.strikeout_metrics() {
            f.strikeout_pos = s.position as f64 / upem;
        }
    }
    f
}
