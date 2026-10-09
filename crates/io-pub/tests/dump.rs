//! Debug dump of an import: `PUB=path cargo test -p newpub-io-pub --test dump -- --nocapture`.

use std::path::PathBuf;

#[test]
fn dump() {
    let Ok(p) = std::env::var("PUB") else { return };
    let (doc, report) = newpub_io_pub::import(&PathBuf::from(p)).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    println!("page {} x {} pages {}", doc.setup.width.0, doc.setup.height.0, doc.pages.len());
    for (i, pg) in doc.pages.iter().enumerate() {
        println!("-- page {i} master {:?}", pg.master);
        for id in &pg.objects {
            let o = &doc.objects[id];
            let kind = serde_json::to_value(&o.kind).unwrap();
            let t = kind["type"].as_str().unwrap_or("?").to_string();
            let extra = match &o.kind {
                newpub_core::ObjectKind::Text(f) => {
                    let st = &doc.stories[&f.story];
                    format!(
                        "story {} frames {:?} {:?}",
                        f.story.0,
                        st.frames.iter().map(|x| x.0).collect::<Vec<_>>(),
                        st.text.chars().take(40).collect::<String>()
                    )
                }
                newpub_core::ObjectKind::Table(t) => {
                    let cells: Vec<String> = t
                        .cells
                        .iter()
                        .map(|c| {
                            format!(
                                "{:?}/{:?}/{}x{}{}",
                                doc.stories[&c.story].text,
                                doc.stories[&c.story].chars.first().map(|s| (
                                    &s.attrs.color,
                                    &s.attrs.font,
                                    s.attrs.size
                                )),
                                c.rowspan,
                                c.colspan,
                                if c.covered { "c" } else { "" }
                            )
                        })
                        .collect();
                    format!(
                        "table {}x{} cols {:?} rows {:?} cells {:?}",
                        t.rows(),
                        t.cols(),
                        t.col_widths.iter().map(|w| w.0).collect::<Vec<_>>(),
                        t.row_heights.iter().map(|w| w.0).collect::<Vec<_>>(),
                        cells
                    )
                }
                _ => String::new(),
            };
            println!(
                "  #{} {t} x={:.1} y={:.1} w={:.1} h={:.1} rot={} {extra}",
                id.0, o.rect.x, o.rect.y, o.rect.w, o.rect.h, o.rotation
            );
        }
    }
}
