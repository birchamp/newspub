# PROGRESS

Build log for newpub-rs. Each entry records what merged, what failed, and the decisions made.
The dashboard parses this file:
- Bullets under **Open blockers** are pinned at the top of the dashboard. Each starts with an ID in brackets.
- Each `###` heading under **Log** is one entry. Bullets containing `ESCALATION:` feed the escalation log.

## Open blockers (need human input)

(none; B-001 resolved 2026-10-08: the dashboard is published as a claude.ai page at the user's request, and GitHub Pages also deploys from CI)

## Log

### 2026-10-08 17:40 — Phase 0: foundation (lead)
- Merged: Cargo workspace (core, layout, render, io-pdf, io-native, io-pub, engine, app, journeys); document model; `Command` layer with snapshot undo; own text layout engine (rustybuzz shaping, UAX#14 breaking, hyphenation, columns, linked-frame flow, wrap exclusions, justification, tabs); tiny-skia raster and krilla PDF back ends sharing one display list; `.npub` zip format; engine `Session` (actions + queries); headless journey runner with PDF (lopdf + hayro), PNG (pixel, ink, golden), and round-trip checks.
- Journeys written before dispatch: J-PG-001..003, J-TF-001..004, J-TY-001..004, J-SH-001, J-IM-001..002, J-LY-001, J-PR-001, J-EX-001, J-FI-001, J-UR-001. All 19 pass on Linux locally; CI on all three OSes is pending.
- Decisions: egui/eframe for UI (AccessKit + egui_kittest give headless UI journeys); our own layout engine instead of parley/cosmic-text, because Publisher needs per-line widths and frame chains; snapshot undo instead of inverse commands; journeys load only bundled OFL fonts so output is deterministic; `newpub-engine` crate added so the app and the runner share one dispatch path.
- Found by journeys while building: narrow wrap pieces force-broke words (fixed: only full-width lines may break inside a word); letterboxed images smeared edge pixels (fixed: clip to the visible image rect); PDF text extraction inserts spaces between runs (runner text matching is whitespace-tolerant).
- Risk noted: krilla 0.8 has no PDF/X validator, so EX-03 (PDF/X) needs an extension or post-processing.

### 2026-10-08 18:05 — Predecessor located (lead)
- User: the existing repo is birchamp/newspub. Beyond the initial-commit `main` it has two branches: `master` (design spec, implementation plan, Electron + React + Vite scaffold, model types) and `claude/ui-testing-responsiveness-smep9u` (a fuller TypeScript app: images, typography, snapping, arrange, thumbnails, shared undo). Phase 0 started from `main` alone and missed these.
- Decision: survey the predecessor's document model and UX now, adopt its decisions where they don't conflict with Publisher parity, and record each adoption in ARCHITECTURE.md §0.
- B-002 resolved: GitHub Pages was already enabled; the CI deploy-pages step succeeded.

### 2026-10-08 18:20 — Predecessor surveyed; decisions adopted (lead)
- An Explore agent surveyed the NewsPub TypeScript app (model, UX, shortcuts, snapping, templates, pretext layout). Results and adopt/not-adopt decisions are in ARCHITECTURE.md §12.
- New parity items: FI-05 (open predecessor `.newspub` files), UI-07 (carried-over shortcuts), UI-08 (template picker at startup). Native files use the `.newspub` extension.

### 2026-10-08 18:40 — Batch 1 merged (8 tasks, 14 items)
- Merged: LY-03 lock enforcement (Haiku), PG-05 page-margin/spread queries (Haiku), FI-02 fonts/assets queries (Haiku), EX-02 PNG/JPEG export (Haiku), SH-04 align/distribute (Haiku), SH-03 arrowheads, caps, joins (Haiku), FR-01/02 find, replace, replace all (Sonnet), AX-01/02 + IM-05 accessibility checker (Sonnet), TY-06/07/10 tab leaders, lists, drop caps in the layout engine (Sonnet).
- Every task passed Sonnet review on its first attempt; no escalations.
- Merge conflicts in engine/lib.rs and core/command.rs (several agents split the same placeholder match arms) were resolved by the lead. All 44 Batch 1-era journeys pass locally; 45/97 overall, and the other 52 belong to items not yet dispatched.
- Process issues found:
  - Workflow worktrees start at the repo's initial commit, so agents had to find the base commit themselves (EX-02 stacked on PG-05).
  - Placeholder "not implemented" arms shared by several tasks cause predictable merge conflicts.
- Fix for Batch 2 specs:
  - Name the exact base commit in each spec.
  - Give every task its own placeholder arm.
- Items are checked off in PARITY.md only once CI is green on all three OSes for this merge.

### 2026-10-08 19:50 — Lead layout work during Batch 2
- Done by the lead: fields and sections (PG-06, PG-09); continued notices (TF-09); autofit fixups (TF-08); tables (TB-01..TB-04); vertical text and rotated frames (TF-10); text inside shapes (SH-06); glyph queries (TY-13 ligatures, TY-16 special characters); keep-with-next / keep-together / widow-orphan control with paragraph rollback (TY-15); baseline grid (TY-14); bidi shaping and visual reordering (TY-17).
- Known gap (TY-17): the PDF text layer of right-to-left runs is in visual order, so copy/paste reverses Hebrew/Arabic. Fix: emit /ActualText spans (krilla marked content) — follow-up item.
- Journey corrections (lead's own arithmetic, not behaviour changes): TF-007/TF-008 story lengths now queried; TF-009 frame too short for 24 pt; TY-013 frame b moved so its first baseline can reach the grid; SH-006 shape text wraps; TY-016 char positions compared relatively.
- Decision: widow/orphan control is on by default (Publisher default); every earlier journey still passes.

### 2026-10-08 20:20 — Mail merge (lead)
- Done by the lead: MM-01..MM-04.
  - Engine: CSV data source (`attach_data_source`), merge fields shown as «name» or as preview values, recipient filter and sort, skip-blank-lines, picture fields resolved relative to the CSV's folder.
  - Output: `merge_to_pdf` writes one copy of the publication per record, with images deduplicated. `merge_to_publication` repeats pages per record and detaches the data source.
- Interface changes (ARCHITECTURE §11):
  - core: `merge` module (`substitute_story`, `Document::merged`, `Document::merge_publication`), `MergeData`.
  - engine: Session `view` doc (the preview substitution feeds layout, render and PDF).
- Bug fixed: text typed after a field no longer inherits the field marker (`Story::insert`).
- Local suite: 62/97.

### 2026-10-08 21:00 — Batch 2 merged (12 tasks)
- Approved by Sonnet review on the first attempt (10 tasks):
  - GUIDES (GD-01..05, Sonnet); TEMPLATES (BB-01/02, Sonnet); SPELL (SP-01..03, Sonnet);
  - PICTURES (IM-04/08, PG-10, Haiku); PDF (EX-03/05, PR-0x n-up, links/bookmarks); HTML (EX-04);
  - PUB (PI-01..04, .pub import); MISC (TF-11 text/.docx import, FI-03 autosave); UI (selection handles, nudging, panels, recent files, shortcuts); LEGACY (FI-05 .newspub).
- Not approved, then fixed by the lead:
  - LAYERS (LY-02). Escalation: Haiku failed twice and moved to Sonnet, which also failed twice. Every failure was the same blocker: the Object query omitted `layer` when it was None (serde skip), so the fix lay outside the task's files. Lead decision: the Object query reports `"layer": null`; the file format is unchanged.
  - EFFECTS (SH-05/07, IM-06/07). Blocked by the lead's ShapeKinds stub, which was already on the lead branch, and by a wrong J-IM-006 probe: [216,180] sits on the yellow box's corner. The probe moved to [288,144], image pixel (300,100), which is blue.
- ESCALATION: LAYERS (LY-02). Haiku failed twice, so it moved to Sonnet; Sonnet failed twice; the lead resolved it. Root cause was the out-of-scope Object query field.
- Lead merge work:
  - render `push_object` merged as page-aware plus the screen flag;
  - Table arms in io-html and pdfq;
  - `Story.typing_attrs` in the legacy importer;
  - new core `DuplicateObjects` command; the app's Cmd+D no longer rebuilds objects through individual commands, so groups, tables and shape text now duplicate;
  - a clippy allow in the templates builder.
- Journey corrections (lead's arithmetic): J-TF-010 deletes 112 chars, not 113; J-IM-006 probe as above.
- Disk: the session disk filled up (12 GB target plus agent worktrees) and caused a rustc ICE. Freed by dropping target/debug; agents now share one target dir that the lead clears after each batch.
- Local: 93/97. Remaining P0–P2: GD-04/UI-06 (rulers, zoom, spread view: UI-GD-001 to be written), PR-07 (print: UI-PR-001), AX-04 (keyboard/AccessKit: UI-AX-001), AX-03 (tagged PDF), BB-03/04 (building blocks, schemes), UI-08 (template picker).

### 2026-10-08 21:50 — Lead: BB-04 schemes, AX-03 tagged PDF, Batch 3 dispatched
- BB-04, done by the lead because it touches core colour and font resolution:
  - `Color::Scheme` slots; 7 original colour schemes and 6 font schemes on bundled fonts;
  - `resolve_char` resolves "+major"/"+minor" and scheme text colours; the engine's display document resolves object, page and table scheme colours, used for render, PDF, HTML and merge.
- AX-03 tagged PDF / PDF/UA-1:
  - Structure markers are added to the shared display list: P, or H1–H6 from "Heading N"/"Title" paragraph styles, with the heading text as title; Figure with alt text for pictures and described shapes; Table/TR/TD for cells.
  - The PDF exporter builds krilla's tag tree in each page's reading order. Undescribed, decorative, master-page and crop-mark content is drawn as artifacts. Link annotations are tagged with alt text.
  - krilla's UA-1 validator runs on export. When there are no bookmarks, the outline comes from the headings, or from the pages if there are no headings.
- Batch 3 dispatched (all Sonnet, base 4a2fcc8): BLOCKS (BB-03, on the new core `Fragment` API), PICKPRINT (UI-08, PR-07), VIEWAX (GD-04, UI-06, AX-04).
- Journey fix (lead): UI-AX-001 tool name is "TextBox" (the app's `Tool` debug name), not "Text".
- Local: 95/100.

### 2026-10-08 22:05 — First PARITY check-off (CI addfc94)
- CI run 37839317947 at commit addfc94: clippy and build are green on Linux, macOS and Windows. Journeys pass 93/97 on each OS, and the failing set is identical everywhere: J-AX-003, J-BB-003, J-BB-004, UI-SH-007, all unimplemented at that commit.
- 105 of 128 PARITY items are checked off: those whose listed journeys all passed on every OS.
- Unchecked:
  - Since fixed by the lead and awaiting CI: BB-04, AX-03.
  - In Batch 3: BB-03, UI-08, PR-07, GD-04, UI-06, AX-04.
  - P3 items without implementations: 15.

### 2026-10-08 22:30 — Batch 3 merged (3 tasks, 6 items)
- All three tasks were approved by Sonnet review on the first attempt:
  - BLOCKS (BB-03): a JSON user library with an assets folder, and 7 original built-in blocks;
  - PICKPRINT (UI-08, PR-07): the startup and New template picker, replacing the old New dialog; a print dialog with printer, copies and page range; OS hand-off through `lp` or PowerShell, or a spool folder in journeys;
  - VIEWAX (GD-04, UI-06, AX-04): rulers, a units menu, actual size and fit, wheel scrolling, two-page spread, a keyboard-focusable canvas with Enter to insert a frame, and screen-reader names for objects.
- Merge conflict in app/lib.rs (fields, `view_state`, and `ui()` hooks from both UI tasks) resolved by the lead. VIEWAX made `view_state` report the TextBox tool as "Text" to fit the original journey wording; the merge keeps the uniform debug name "TextBox", which matches the corrected UI-AX-001.
- Local: 100/100 journeys pass, clippy and fmt clean. All P0–P2 items are implemented; check-off waits for CI on all three OSes.
- Untested: the real OS print hand-off (CI has no printer).

### 2026-10-08 23:10 — Stop condition reached: all P0–P2 items pass on all three OSes
- CI run 37842573100 at commit bb529dc: clippy, build and 100/100 journeys pass on Linux, macOS and Windows.
- Checked off: BB-03, BB-04, AX-03, UI-08, PR-07, GD-04, UI-06, AX-04. PARITY now stands at 113/128; P0–P2 at 110/110.
- B-001 closed: the dashboard is the claude.ai page, with GitHub Pages as a CI mirror.
- Final report: REPORT.md (parity by area, known gaps, top risks, P3 recommendations).

### 2026-10-08 22:40 — P3 work started (user: "go ahead with the P3 items")
- Journeys were written first for all 15 P3 items: J-TB-005, J-FR-002, J-IM-008, J-IM-009, UI-LY-001, J-PG-008, J-TY-017, J-TY-018, J-SH-008, J-BB-005, J-MM-004, J-MM-005, J-PR-005, J-PR-006 and J-EX-006.
- New fixtures, all generated and original: halves.gif/.bmp/.tif, badge.svg, members.xlsx, products.csv.
- Runner checks added: `expect_files` and `expect_zip`.
- PARITY: IM-09 is narrowed to SVG, TIFF, GIF and BMP. EMF/WMF becomes a new IM-11 (P3, research: there is no maintained Rust parser; it needs a vector converter). Doing so keeps IM-09 from looking done while EMF/WMF is still missing.
- Interfaces, all lead decisions recorded in ARCHITECTURE §11:
  - text effects through `ResolvedChar` and `GlyphRun`;
  - `Object.hidden` and `Object.overprint`;
  - `ObjectKind::WordArt` and `ShapeKind::Bezier`, with the Bézier geometry rendered by the lead;
  - document sheet layout, business information, catalog areas;
  - `Imposition::DocumentSheet` and `PdfOptions.separations`;
  - engine actions and queries, and the picture-decoding hook.
- Done by the lead (journeys pass locally): MM-05 catalog merge, MM-06 Excel data source (calamine, MIT), BB-05 business information sets.
- Batch 4 dispatched on base f5fa56d (103/115 passing): TABLEPASTE (Haiku), FINDFMT, IMAGES, OBJECTS, PRODUCTS, TEXTART, FREEFORM, SEPARATIONS (Sonnet), PACKGO (Haiku), EXPORT (Sonnet).

### 2026-10-08 23:45 — Batch 4 (P3) merged; IM-11 done by the lead
- All 10 tasks were approved by Sonnet review on the first attempt:
  - TABLEPASTE (TB-05) and PACKGO (PR-09), on Haiku;
  - FINDFMT (FR-03), IMAGES (IM-09), OBJECTS (IM-10, LY-04), PRODUCTS (PG-11), TEXTART (TY-18, TY-19), FREEFORM (SH-08), SEPARATIONS (PR-08) and EXPORT (EX-06: EPUB and XPS), on Sonnet.
- IM-11 (lead): a new `newpub-io-metafile` crate converts EMF and WMF to SVG from the published record layouts, and `decode_picture` stores the result as an SVG picture.
  - It covers shapes, polygons, polylines, Béziers, paths, pens and brushes, text, mapping modes and world transforms.
  - Bitmaps, clipping and gradients inside metafiles are not converted.
  - The fixtures come from `tools/fixtures/make_metafiles.py`, and PIL parses both as valid metafiles.
- Lead fixes at merge:
  - the Object query reports `hidden` and `overprint` (LY-04);
  - `io-native::open` resolves relative picture links against the file's folder (Pack and Go);
  - the runner treats a missing path as null for `is_null`;
  - J-PG-008's `page_size` now uses the `{w, h}` form (my format error);
  - the display list receives the `FontStore`, so WordArt uses the publication's fonts and not just the bundled ones (TEXTART's LEAD-NEEDED);
  - conflicts resolved: render raster (SVG scaling plus TEXTART's refactor), io-pdf Cargo.toml and `Ctx.svgs`, and the workspace members (io-metafile and io-xps).
- Accepted agent notes:
  - IMAGES re-encodes GIF/BMP/TIFF as PNG with a "Source" text chunk recording the original format.
  - The XPS output has not been opened in a real XPS viewer, only checked as well-formed XML.
  - SEPARATIONS ignores overprint on group children.
- Local result: 116/116 journeys pass, and clippy and fmt are clean.
