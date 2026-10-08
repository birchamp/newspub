# PROGRESS

Build log for newpub-rs. Each entry records what merged, what failed, and the decisions made.
The dashboard parses this file:
- Bullets under **Open blockers** are pinned at the top of the dashboard. Each starts with an ID in brackets.
- Each `###` heading under **Log** is one entry. Bullets containing `ESCALATION:` feed the escalation log.

## Open blockers (need human input)

- [B-001] Brief placeholders were left blank: the existing newpub repo/stack, the new repo URL, the target users, and where the dashboard is published. Working assumptions are in ARCHITECTURE.md §0 (this repo; small-org newsletters, bulletins, and booklets; GitHub Pages plus a claude.ai artifact). Confirm or correct them.
- [B-002] GitHub Pages must be enabled (Settings → Pages → Source: GitHub Actions) before the CI dashboard deploy step can publish.

## Log

### 2026-10-08 17:40 — Phase 0: foundation (lead)
- Merged: Cargo workspace (core, layout, render, io-pdf, io-native, io-pub, engine, app, journeys); document model; `Command` layer with snapshot undo; own text layout engine (rustybuzz shaping, UAX#14 breaking, hyphenation, columns, linked-frame flow, wrap exclusions, justification, tabs); tiny-skia raster and krilla PDF back ends sharing one display list; `.npub` zip format; engine `Session` (actions + queries); headless journey runner with PDF (lopdf + hayro), PNG (pixel, ink, golden), and round-trip checks.
- Journeys written before dispatch: J-PG-001..003, J-TF-001..004, J-TY-001..004, J-SH-001, J-IM-001..002, J-LY-001, J-PR-001, J-EX-001, J-FI-001, J-UR-001. All 19 pass on Linux locally; CI on all three OSes is pending.
- Decisions: egui/eframe for UI (AccessKit + egui_kittest give headless UI journeys); our own layout engine instead of parley/cosmic-text, because Publisher needs per-line widths and frame chains; snapshot undo instead of inverse commands; journeys load only bundled OFL fonts so output is deterministic; `newpub-engine` crate added so the app and the runner share one dispatch path.
- Found by journeys while building: narrow wrap pieces force-broke words (fixed: only full-width lines may break inside a word); letterboxed images smeared edge pixels (fixed: clip to the visible image rect); PDF text extraction inserts spaces between runs (runner text matching is whitespace-tolerant).
- Risk noted: krilla 0.8 has no PDF/X validator, so EX-03 (PDF/X) needs an extension or post-processing.
