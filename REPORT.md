# newpub-rs: status report (stop condition reached)

**Date:** 2026-10-08.
**Commit:** bb529dc, branch `claude/happy-davinci-r0kfpv`.
**CI:** run 37842573100. Clippy, build and 100/100 journeys pass on Linux, macOS and Windows.

All P0–P2 parity items pass their journeys on all three OSes: **110/110**. Overall parity is **113/128 (88%)**.
The 15 unchecked items are all P3.

## Parity by area

| Area | All items | P0–P2 | Open P3 |
|------|-----------|-------|---------|
| PG page setup and masters | 10/11 | 10/10 | PG-11 publication types (labels, envelopes) |
| TF text frames | 11/11 | 11/11 | — |
| TY typography | 17/19 | 17/17 | TY-18 text effects, TY-19 WordArt |
| SH shapes | 7/8 | 7/7 | SH-08 freeform/Bézier |
| TB tables | 4/5 | 4/4 | TB-05 paste TSV |
| IM pictures | 8/10 | 8/8 | IM-09 SVG/EMF/WMF/TIFF/GIF/BMP, IM-10 captions |
| GD guides and units | 5/5 | 5/5 | — |
| LY arrange and layers | 3/4 | 3/3 | LY-04 selection pane |
| BB templates, blocks, schemes | 4/5 | 4/4 | BB-05 business information sets |
| MM mail merge | 4/6 | 4/4 | MM-05 catalog merge, MM-06 .xlsx source |
| PR print production | 7/9 | 7/7 | PR-08 separations/overprint, PR-09 Pack and Go |
| EX export | 5/6 | 5/5 | EX-06 XPS/EPUB |
| FI files | 5/5 | 5/5 | — |
| PI .pub import | 4/4 | 1/1 | — |
| UR undo | 2/2 | 2/2 | — |
| SP spelling | 3/3 | 3/3 | — |
| AX accessibility | 4/4 | 4/4 | — |
| FR find/replace | 2/3 | 2/2 | FR-03 formatting and special characters |
| UI app shell | 8/8 | 8/8 | — |

## Known gaps (behaviour a journey does not yet cover)

1. **Right-to-left text in PDFs.** It is shaped and displayed in the right order, but the PDF text layer is in visual order, so copying Hebrew or Arabic reverses it. Fix: wrap RTL runs in /ActualText spans. krilla only allows these inside a tag tree, so either always tag or post-process.
2. **Printing to a real printer.** The journey checks the spooled PDF because CI has no printer. The hand-off to `lp` (Linux/macOS) and PowerShell PrintTo (Windows) has never run against a real printer.
3. **.pub import fidelity.** Text, frame geometry, pictures, fonts and basic formatting import. Masters, tables, shapes and WordArt are not imported (see ARCHITECTURE §9).
4. **PDF/X-4.** The output intent uses the bundled CC0 "CGATS TR 001" CMYK profile only. FOGRA39 and other conditions need a profile we may redistribute. No independent PDF/X validator runs in CI.
5. **Tagged PDF.** It passes krilla's UA-1 validator. Tables get Table/TR/TD but no TH or header scope. Group and master content is drawn as artifacts. veraPDF has not been run.
6. **Building blocks.** Saving a block that contains a picture is not covered by a journey. User and built-in blocks with the same name both appear in the list; the user block wins when inserting.
7. **Rendering.** Cross-OS rendering is deterministic only because journeys use the bundled fonts. With system fonts, output depends on what is installed.

## Top risks

1. **UI depth versus Publisher.** The app has every P0–P2 flow, but its panels are thinner than Publisher's ribbon: no gallery previews, no context menus, limited dialogs. Journeys check behaviour, not polish. Real-user testing with the target audience is the next step.
2. **Performance at scale.** Layout runs over the whole document and recomputes after every change; the view document (merge preview, schemes) clones the publication. That is fine for newsletters (≤ 24 pages) but untested on long booklets or large mail merges. Incremental layout is the main fix.
3. **Single-maintainer core.** The layout engine (bidi, keep rules, tables, fields) is lead-owned and dense. Its behaviour is pinned by journeys only, which by policy include no unit tests. The next contributor needs ARCHITECTURE §5 and the TY/TF journeys.
4. **Dependency churn.** egui changes its API each minor version and krilla is pre-1.0; both are pinned. The Rust toolchain is pinned at 1.97.0 because newer clippy lints broke CI once.
5. **Disk use in agent sessions.** Parallel worktrees plus a shared target directory filled the session disk once and crashed rustc. The lead should clear the agents' shared target directory after each batch.

## Recommended next steps (P3)

Order by value to the target users (newsletters, bulletins, flyers, booklets):

1. **TB-05** paste TSV into a table, and **FR-03** find/replace formatting. Both are small and well-defined Haiku-sized tasks.
2. **IM-09** more image formats. GIF, BMP and TIFF through the `image` crate are easy; SVG through `resvg`/`usvg`. EMF/WMF is a research item.
3. **IM-10** captions and **LY-04** selection pane. Both are UI work for Sonnet.
4. **PG-11** labels, envelopes and business cards. These reuse n-up imposition (PR-03) plus page presets.
5. **MM-06** .xlsx data source (`calamine`, MIT) and **MM-05** catalog merge. The latter needs a repeating-area model, so the lead designs it.
6. **BB-05** business information sets. These map onto fields (PG-06) plus a small store.
7. **PR-09** Pack and Go. It reuses FI-02 and IM-08 asset handling.
8. **TY-18/TY-19** text effects and WordArt, **SH-08** Bézier editing, **PR-08** separations. These are larger rendering and editing features.
9. **EX-06** EPUB (from the HTML export) before XPS. XPS has little demand.
10. Also: RTL /ActualText (gap 1) and a veraPDF run in CI for PDF/UA and PDF/X.

Each item already has a journey ID in PARITY.md. Write the journeys first, as the build loop prescribes.
