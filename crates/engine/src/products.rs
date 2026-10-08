//! Publication types: business cards, labels, envelopes (PG-11).

use crate::{EngineError, Outcome, Query, Session, SessionAction};
use newpub_core::{CoreError, Document, Insets, Length, PageSetup, SheetLayout};
use serde_json::{Value, json};

const IN: f64 = 72.0;
const MM: f64 = 72.0 / 25.4;

struct Product {
    id: &'static str,
    label: &'static str,
    width: f64,
    height: f64,
    margin: f64,
    size_text: &'static str,
    sheet: Option<SheetLayout>,
}

/// A Letter sheet with a grid of items.
fn sheet(columns: u32, rows: u32, left: f64, top: f64, col_gap: f64) -> Option<SheetLayout> {
    Some(SheetLayout {
        width: Length(8.5 * IN),
        height: Length(11.0 * IN),
        columns,
        rows,
        left: Length(left * IN),
        top: Length(top * IN),
        col_gap: Length(col_gap * IN),
        row_gap: Length(0.0),
    })
}

fn products() -> Vec<Product> {
    vec![
        Product {
            id: "business-card",
            label: "Business card",
            width: 3.5 * IN,
            height: 2.0 * IN,
            margin: 0.1 * IN,
            size_text: "3.5 x 2 in",
            sheet: sheet(2, 5, 0.75, 0.5, 0.0),
        },
        Product {
            id: "address-label",
            label: "Address label",
            width: 2.625 * IN,
            height: 1.0 * IN,
            margin: 0.1 * IN,
            size_text: "2.625 x 1 in",
            sheet: sheet(3, 10, 0.1875, 0.5, 0.125),
        },
        Product {
            id: "name-badge",
            label: "Name badge",
            width: 4.0 * IN,
            height: 3.0 * IN,
            margin: 0.1 * IN,
            size_text: "4 x 3 in",
            sheet: sheet(2, 3, 0.25, 1.0, 0.0),
        },
        Product {
            id: "postcard",
            label: "Postcard",
            width: 6.0 * IN,
            height: 4.0 * IN,
            margin: 0.25 * IN,
            size_text: "6 x 4 in",
            sheet: sheet(1, 2, 1.25, 1.5, 0.0),
        },
        Product {
            id: "envelope-10",
            label: "Envelope",
            width: 9.5 * IN,
            height: 4.125 * IN,
            margin: 0.25 * IN,
            size_text: "9.5 x 4.125 in",
            sheet: None,
        },
        Product {
            id: "envelope-dl",
            label: "Envelope DL",
            width: 220.0 * MM,
            height: 110.0 * MM,
            margin: 0.25 * IN,
            size_text: "220 x 110 mm",
            sheet: None,
        },
        Product {
            id: "envelope-c5",
            label: "Envelope C5",
            width: 229.0 * MM,
            height: 162.0 * MM,
            margin: 0.25 * IN,
            size_text: "229 x 162 mm",
            sheet: None,
        },
    ]
}

impl Product {
    fn per_sheet(&self) -> u32 {
        self.sheet.as_ref().map_or(1, |s| s.columns * s.rows)
    }

    fn json(&self) -> Value {
        let name = match &self.sheet {
            Some(_) => format!("{} ({}, {} per sheet)", self.label, self.size_text, self.per_sheet()),
            None => format!("{} ({})", self.label, self.size_text),
        };
        json!({
            "id": self.id,
            "name": name,
            "width": self.width,
            "height": self.height,
            "per_sheet": self.per_sheet(),
            "sheet": self.sheet,
        })
    }
}

fn find(id: &str) -> Result<Product, EngineError> {
    products()
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| EngineError::Other(format!("unknown publication type '{id}'")))
}

impl Session {
    pub(crate) fn products_action(&mut self, a: &SessionAction) -> Result<Outcome, EngineError> {
        let SessionAction::NewFromPublicationType { id } = a else {
            return Err(EngineError::Other(format!("{a:?} is not a products action")));
        };
        let p = find(id)?;
        if p.width <= 0.0 || p.height <= 0.0 {
            return Err(CoreError::Invalid("page size must be positive".into()).into());
        }
        let setup = PageSetup {
            width: Length(p.width),
            height: Length(p.height),
            margins: Insets::uniform(p.margin),
            facing: false,
            bleed: Length(0.0),
        };
        let mut doc = Document::new(setup, 1);
        doc.sheet = p.sheet;
        self.doc = doc;
        self.history = Default::default();
        self.group = None;
        self.path = None;
        self.raster.clear_images();
        self.changed();
        self.dirty = false;
        Ok(Outcome { created: self.doc.pages.iter().map(|p| p.id).collect() })
    }

    pub(crate) fn products_query(&mut self, q: &Query) -> Result<Value, EngineError> {
        match q {
            Query::PublicationTypes => Ok(Value::Array(products().iter().map(Product::json).collect())),
            Query::PublicationType { id } => Ok(find(id)?.json()),
            _ => Err(EngineError::Other(format!("query {q:?} is not a products query"))),
        }
    }
}
