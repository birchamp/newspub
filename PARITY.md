# newpub-rs: Publisher parity checklist

This is the single source of truth for feature parity with Microsoft Publisher (Microsoft 365).
The goal is behavioural parity. We reproduce what Publisher does, never its code, assets, or templates.

**Rules**
- An item is checked (`[x]`) only when every journey listed for it passes in CI on macOS, Windows, and Linux.
- Journeys are written by the lead **before** an item is dispatched. The implementing agent never edits them.
- Priorities: **P0** means newpub is unusable without it. **P1** means everyday work needs it.
  **P2** covers professional and occasional workflows. **P3** is rare, legacy, or research.
- Target users (assumed until told otherwise; see ARCHITECTURE.md §0): small organisations, schools, and
  churches that produce newsletters, bulletins, flyers, and folded booklets and print them in-house or at a print shop.
  Their workflows decide what goes into P0 and P1.
- Journey IDs (`J-…`) refer to files in `journeys/scripts/<ID>.yaml`. A `UI-…` journey drives the real egui app through AccessKit.
- The dashboard parses the tables below. Keep the column layout unchanged: `| ID | P | Feature | Journeys | Done |`.

## PG: Page setup and master pages

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| PG-01 | P0 | New publication from a page size (Letter, A4, A5, custom W×H) and orientation | J-PG-001 | [ ] |
| PG-02 | P0 | Margins (top/bottom/inside/outside) per publication | J-PG-001 | [ ] |
| PG-03 | P0 | Insert, delete, duplicate, and reorder pages | J-PG-002 | [ ] |
| PG-04 | P0 | Master pages: create, apply to page, objects on master appear on pages | J-PG-003 | [ ] |
| PG-05 | P1 | Two-page (facing) masters and spread view; inside/outside margins mirror | J-PG-004 | [ ] |
| PG-06 | P1 | Page numbers, page count, and section fields on masters (auto-updating) | J-PG-005 | [ ] |
| PG-07 | P1 | Ignore master on a page; multiple masters per publication | J-PG-003 | [ ] |
| PG-08 | P1 | Page background colour / fill | J-PG-006 | [ ] |
| PG-09 | P2 | Sections with restart numbering and number format (1, i, I, a, A) | J-PG-005 | [ ] |
| PG-10 | P2 | Change page size of an existing publication (objects stay put, reported off-page) | J-PG-007 | [ ] |
| PG-11 | P3 | Publication types (envelopes, labels, business cards: multiple pages per sheet) | J-PG-008 | [ ] |

## TF: Text frames, linked text, and overflow

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| TF-01 | P0 | Draw text frame, type/insert text, text wraps to frame width | J-TF-001 | [ ] |
| TF-02 | P0 | Overflow detection (text that does not fit is flagged) | J-TF-001 | [ ] |
| TF-03 | P0 | Link frames (also across pages); text flows frame to frame | J-TF-002 | [ ] |
| TF-04 | P0 | Unlink/relink frames; text reflows | J-TF-003 | [ ] |
| TF-05 | P0 | Paste long text and auto-flow: add pages + linked frames until text fits | J-TF-004 | [ ] |
| TF-06 | P1 | Frame columns with gutter; text flows column to column | J-TF-005 | [ ] |
| TF-07 | P1 | Frame internal margins (insets) and vertical alignment (top/middle/bottom) | J-TF-006 | [ ] |
| TF-08 | P1 | Text autofit: shrink text on overflow; best fit; grow frame to fit text | J-TF-007 | [ ] |
| TF-09 | P1 | "Continued on page / from page" notices | J-TF-008 | [ ] |
| TF-10 | P2 | Rotated text frames; vertical text direction | J-TF-009 | [ ] |
| TF-11 | P2 | Import text file (.txt / .docx paragraph text) into a story | J-TF-010 | [ ] |

## TY: Typography

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| TY-01 | P0 | Font family, size, bold, italic, underline, colour per character run | J-TY-001 | [ ] |
| TY-02 | P0 | Paragraph alignment: left, centre, right, justify | J-TY-002 | [ ] |
| TY-03 | P0 | Line spacing (multiple and exact pt), space before/after paragraph | J-TY-003 | [ ] |
| TY-04 | P0 | Named paragraph and character styles; apply, modify (text updates), based-on | J-TY-004 | [ ] |
| TY-05 | P1 | Indents: left, right, first-line, hanging | J-TY-005 | [ ] |
| TY-06 | P1 | Tabs: left/centre/right/decimal with leaders | J-TY-006 | [ ] |
| TY-07 | P1 | Bulleted and numbered lists | J-TY-007 | [ ] |
| TY-08 | P1 | Kerning (font kerning on/off, manual pair adjustment) | J-TY-008 | [ ] |
| TY-09 | P1 | Tracking (character spacing) and character scaling | J-TY-008 | [ ] |
| TY-10 | P1 | Drop caps (lines, size, font) | J-TY-009 | [ ] |
| TY-11 | P1 | Hyphenation (auto, hyphenation zone, manual/optional hyphens) | J-TY-010 | [ ] |
| TY-12 | P1 | Superscript, subscript, small caps, all caps, strikethrough | J-TY-011 | [ ] |
| TY-13 | P2 | OpenType ligatures (standard, discretionary), stylistic sets, number styles | J-TY-012 | [ ] |
| TY-14 | P2 | Baseline guides / align text to baseline grid | J-TY-013 | [ ] |
| TY-15 | P2 | Keep with next, keep lines together, widow/orphan control | J-TY-014 | [ ] |
| TY-16 | P2 | Special characters: non-breaking space, em/en dash, optional hyphen, line break | J-TY-015 | [ ] |
| TY-17 | P2 | Right-to-left and complex-script shaping (Arabic, Hebrew, Devanagari) | J-TY-016 | [ ] |
| TY-18 | P3 | Text effects (shadow, outline, glow, reflection, emboss on text) | J-TY-017 | [ ] |
| TY-19 | P3 | WordArt-style text objects | J-TY-018 | [ ] |

## SH: Shapes and lines

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| SH-01 | P0 | Rectangle, ellipse, line with fill colour, stroke colour and weight | J-SH-001 | [ ] |
| SH-02 | P1 | Move, resize, rotate, flip objects | J-SH-002 | [ ] |
| SH-03 | P1 | Dash styles, arrowheads, line caps/joins | J-SH-003 | [ ] |
| SH-04 | P1 | Group / ungroup; align and distribute objects | J-SH-004 | [ ] |
| SH-05 | P2 | AutoShapes library (rounded rectangle, star, arrow, callout, polygon, triangle) | J-SH-005 | [ ] |
| SH-06 | P2 | Text inside shapes | J-SH-006 | [ ] |
| SH-07 | P2 | Gradient, pattern, and transparency fills; shape shadow | J-SH-007 | [ ] |
| SH-08 | P3 | Freeform / Bézier drawing and point editing | J-SH-008 | [ ] |

## TB: Tables

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| TB-01 | P1 | Insert table rows×cols; type in cells; text wraps in cell | J-TB-001 | [ ] |
| TB-02 | P1 | Insert/delete rows and columns; resize rows/columns | J-TB-002 | [ ] |
| TB-03 | P1 | Cell borders, fill; merge and split cells | J-TB-003 | [ ] |
| TB-04 | P2 | Table formats (preset styles), header row | J-TB-004 | [ ] |
| TB-05 | P3 | Paste tabular data (TSV) into a table | J-TB-005 | [ ] |

## IM: Images

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| IM-01 | P0 | Insert PNG/JPEG picture; placed at native aspect | J-IM-001 | [ ] |
| IM-02 | P0 | Resize keeps aspect; crop (all sides), fit/fill in frame | J-IM-002 | [ ] |
| IM-03 | P1 | Text wrap around object: square, tight, top-and-bottom, none, through; wrap distance | J-IM-003 | [ ] |
| IM-04 | P1 | Picture placeholder frames; swap picture keeps frame | J-IM-004 | [ ] |
| IM-05 | P1 | Alt text on pictures | J-AX-001 | [ ] |
| IM-06 | P2 | Recolour, brightness, contrast, greyscale | J-IM-005 | [ ] |
| IM-07 | P2 | Picture borders, shapes (crop to shape), shadow, soft edges | J-IM-006 | [ ] |
| IM-08 | P2 | Linked (external) vs embedded pictures; relink | J-IM-007 | [ ] |
| IM-09 | P3 | SVG/EMF/WMF/TIFF/GIF/BMP import | J-IM-008 | [ ] |
| IM-10 | P3 | Captions (caption gallery) grouped to picture | J-IM-009 | [ ] |

## GD: Layout guides and snapping

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| GD-01 | P1 | Margin and column/row grid guides | J-GD-001 | [ ] |
| GD-02 | P1 | Ruler guides (add, move, delete) | J-GD-001 | [ ] |
| GD-03 | P1 | Snap to guides, objects, and margins when moving/resizing | J-GD-002 | [ ] |
| GD-04 | P2 | Rulers, zoom, measurement units (in, cm, mm, pt, pi) | J-GD-003, UI-GD-001 | [ ] |
| GD-05 | P2 | Object position/size panel (exact numeric placement) | J-GD-003 | [ ] |

## LY: Layers and z-order

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| LY-01 | P0 | Z-order: bring to front/forward, send to back/backward | J-LY-001 | [ ] |
| LY-02 | P2 | Layers: create, rename, reorder, hide, lock; move object to layer | J-LY-002 | [ ] |
| LY-03 | P2 | Lock object position | J-LY-003 | [ ] |
| LY-04 | P3 | Selection pane (list, rename, hide objects) | UI-LY-001 | [ ] |

## BB: Building blocks and templates

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| BB-01 | P1 | Save publication as template; new publication from template | J-BB-001 | [ ] |
| BB-02 | P1 | Built-in starter templates (original designs: newsletter, flyer, bulletin, booklet) | J-BB-002 | [ ] |
| BB-03 | P2 | Building blocks: save selection, insert block; built-in originals (headings, sidebars, pull quotes) | J-BB-003 | [ ] |
| BB-04 | P2 | Colour schemes and font schemes applied publication-wide | J-BB-004 | [ ] |
| BB-05 | P3 | Business information sets (fields auto-filled into publication) | J-BB-005 | [ ] |

## MM: Mail merge and catalog merge

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| MM-01 | P2 | Mail merge from CSV: insert field codes; preview record N | J-MM-001 | [ ] |
| MM-02 | P2 | Merge to new publication / to PDF (one copy per record) | J-MM-001 | [ ] |
| MM-03 | P2 | Filter/sort recipient list; skip blank fields | J-MM-002 | [ ] |
| MM-04 | P2 | Picture fields in merge | J-MM-003 | [ ] |
| MM-05 | P3 | Catalog merge (repeating area, multiple records per page) | J-MM-004 | [ ] |
| MM-06 | P3 | Excel (.xlsx) data source | J-MM-005 | [ ] |

## PR: Print production

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| PR-01 | P0 | Booklet imposition (fold, page order for saddle stitch, blank padding to ×4) | J-PR-001 | [ ] |
| PR-02 | P1 | Bleed (extend objects past trim) and BleedBox/TrimBox | J-PR-002 | [ ] |
| PR-03 | P1 | Crop marks, registration marks, page information | J-PR-002 | [ ] |
| PR-04 | P1 | Multiple pages per sheet (n-up) and duplex settings | J-PR-003 | [ ] |
| PR-05 | P2 | CMYK colours in the document model and DeviceCMYK output | J-PR-004 | [ ] |
| PR-06 | P2 | Spot colours (Separation colourspace) | J-PR-004 | [ ] |
| PR-07 | P2 | Print to system printer (OS print dialog) | UI-PR-001 | [ ] |
| PR-08 | P3 | Colour separations and overprint control | J-PR-005 | [ ] |
| PR-09 | P3 | Pack and Go (collect fonts/images into a folder) | J-PR-006 | [ ] |

## EX: Export

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| EX-01 | P0 | Export PDF: vector text with embedded subset fonts, images, shapes | J-EX-001 | [ ] |
| EX-02 | P1 | Export page as PNG/JPEG at chosen DPI | J-EX-002 | [ ] |
| EX-03 | P2 | Export PDF/X-4 (OutputIntent, boxes, no transparency issues). Risk: krilla 0.8 has no PDF/X validator, so this needs our own post-processing | J-EX-003 | [ ] |
| EX-04 | P2 | Export HTML (one page per page, positioned) | J-EX-004 | [ ] |
| EX-05 | P2 | PDF hyperlinks and bookmarks | J-EX-005 | [ ] |
| EX-06 | P3 | Export to XPS / EPUB | J-EX-006 | [ ] |

## FI: Native file format

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| FI-01 | P0 | Save and open `.npub`; round-trip preserves everything | J-FI-001 | [ ] |
| FI-02 | P1 | Embedded images and fonts list survive round-trip | J-FI-002 | [ ] |
| FI-03 | P2 | Autosave and crash recovery | J-FI-003 | [ ] |
| FI-04 | P2 | File format versioning and forward migration | J-FI-004 | [ ] |
| FI-05 | P1 | Open predecessor NewsPub (Electron) `.newspub` files, converting threads, runs, and assets | J-FI-005 | [ ] |

## PI: .pub import (research track; see ARCHITECTURE.md §9)

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| PI-01 | P2 | Open .pub container (OLE CFB); report streams; reject gracefully if unsupported | J-PI-001 | [ ] |
| PI-02 | P3 | Import text stories (Quill/CONTENTS) as plain text with paragraphs | J-PI-002 | [ ] |
| PI-03 | P3 | Import page size, frame geometry, and pictures | J-PI-003 | [ ] |
| PI-04 | P3 | Import character/paragraph formatting | J-PI-004 | [ ] |

## UR: Undo/redo

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| UR-01 | P0 | Undo/redo of every document command, multi-level | J-UR-001 | [ ] |
| UR-02 | P1 | Typing coalesced into one undo step; undo history survives save (not across sessions) | J-UR-002 | [ ] |

## SP: Spell check

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| SP-01 | P1 | Spell check stories against a dictionary (Hunspell-format, en-US bundled) | J-SP-001 | [ ] |
| SP-02 | P1 | Suggestions, replace, ignore, add to user dictionary | J-SP-001 | [ ] |
| SP-03 | P2 | Per-run language tagging; check other languages if dictionary installed | J-SP-002 | [ ] |

## AX: Accessibility

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| AX-01 | P1 | Alt text on all non-text objects; mark decorative | J-AX-001 | [ ] |
| AX-02 | P1 | Accessibility checker (missing alt text, low contrast, reading order, tiny text) | J-AX-002 | [ ] |
| AX-03 | P2 | Tagged PDF (PDF/UA) with reading order and alt text | J-AX-003 | [ ] |
| AX-04 | P2 | App UI fully operable by keyboard and screen reader (AccessKit tree) | UI-AX-001 | [ ] |

## FR: Find and replace

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| FR-01 | P1 | Find across all stories; match case, whole word | J-FR-001 | [ ] |
| FR-02 | P1 | Replace / replace all (one undo step) | J-FR-001 | [ ] |
| FR-03 | P3 | Find/replace formatting and special characters | J-FR-002 | [ ] |

## UI: Application shell (real-UI journeys)

| ID | P | Feature | Journeys | Done |
|----|---|---------|----------|------|
| UI-01 | P0 | Window with page canvas, page navigator, ribbon/toolbars; launches on all three OSes | UI-SH-001 | [ ] |
| UI-02 | P0 | Create publication, draw text frame, type text, export PDF through the UI | UI-SH-002 | [ ] |
| UI-03 | P1 | Select, move, and resize objects with mouse and keyboard nudges | UI-SH-003 | [ ] |
| UI-04 | P1 | Format panels (font, paragraph, object) wired to commands | UI-SH-004 | [ ] |
| UI-05 | P1 | Open/save dialogs, recent files | UI-SH-005 | [ ] |
| UI-06 | P2 | Zoom, scroll, two-page spread view, rulers | UI-GD-001 | [ ] |
| UI-07 | P1 | Keyboard shortcuts carried over from NewsPub (duplicate, z-order, zoom stops, fit, nudge, Tab through story frames) | UI-SH-006 | [ ] |
| UI-08 | P2 | Template picker at startup; new publication from a built-in or saved template | UI-SH-007 | [ ] |
