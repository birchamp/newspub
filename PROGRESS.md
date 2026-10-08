# PROGRESS

Build log for newpub-rs. Each entry records what merged, what failed, and the decisions made.
The dashboard parses this file:
- Bullets under **Open blockers** are pinned at the top of the dashboard. Each starts with an ID in brackets.
- Each `###` heading under **Log** is one entry. Bullets containing `ESCALATION:` feed the escalation log.

## Open blockers (need human input)

- [B-001] Partly answered: the existing source is birchamp/newspub (Electron + React + TypeScript on branches `master` and `claude/ui-testing-responsiveness-smep9u`). Target users are now taken from the predecessor's spec (non-technical users making multi-page newsletters). Still open: the dashboard location (assumed: GitHub Pages, which already deploys from CI). Reply only to change it.

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
