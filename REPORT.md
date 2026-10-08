# newpub-rs: status report

**Date:** 2026-10-08.
**Commit:** 6c64ad8, branch `claude/happy-davinci-r0kfpv`.
**CI:** run 37854377656. Clippy, build and 116/116 journeys pass on Linux, macOS and Windows.

**All 129 PARITY items, P0–P3, pass their journeys on all three OSes.** IM-11 (EMF/WMF) was split out of IM-09 so that each could be checked honestly.

## Parity by area

| Area | Items | Notes |
|------|-------|-------|
| PG page setup, masters, publication types | 11/11 | Cards, labels and badges print several per sheet; envelopes |
| TF text frames | 11/11 | |
| TY typography | 19/19 | Includes text effects (shadow, outline, glow, reflection, emboss) and WordArt with warps |
| SH shapes | 8/8 | Includes freeform and Bézier point editing (API; no on-canvas point-editing tool yet) |
| TB tables | 5/5 | Includes TSV paste |
| IM pictures | 11/11 | PNG, JPEG, GIF, BMP, TIFF, SVG (vector in PDF), EMF/WMF (as SVG); captions |
| GD guides and units | 5/5 | |
| LY arrange, layers, selection pane | 4/4 | |
| BB templates, blocks, schemes, business info | 5/5 | |
| MM mail merge | 6/6 | CSV and .xlsx, filter, sort, pictures, catalog merge |
| PR print production | 9/9 | Includes separations and overprint, and Pack and Go |
| EX export | 6/6 | PDF, PDF/X-4, PDF/UA-1, PNG/JPEG, HTML, EPUB 3 (fixed layout), XPS |
| FI files | 5/5 | |
| PI .pub import | 4/4 | |
| UR, SP, AX, FR, UI | 2/2, 3/3, 4/4, 3/3, 8/8 | |

## Known gaps

Each of these is real behaviour that a journey does not cover, or that is only partly done.

1. **Right-to-left text in PDFs.** The PDF text layer is in visual order, so copying Hebrew or Arabic reverses it. Fix: tag every export and wrap RTL runs in /ActualText spans; krilla allows these only inside a tag tree.
2. **Printing to a real printer.** The print hand-off (`lp`, PowerShell PrintTo) has not run against a real printer; CI checks the spooled PDF.
3. **XPS.** The output is well-formed, but has not been opened in an XPS viewer. GIF and WebP pictures are dropped from XPS. EPUB does not embed .ttc fonts.
4. **EMF/WMF.** Only the common drawing records are converted. Bitmaps, clipping regions, gradients and some text features inside metafiles are skipped. Real-world clip art may need more records.
5. **Separations.** Overprint is ignored on group children. Overprinting images at partial opacity are approximated. Composite PDFs do not carry an overprint flag.
6. **Freeform shapes.** Point editing is available through commands and the API. The canvas has no point-editing handles yet.
7. **Tagged PDF.** No table header cells or scope. veraPDF has not been run on PDF/UA or PDF/X output.
8. **.pub import.** Covers text, frames, pictures, fonts and basic formatting. Masters, tables, shapes and WordArt are not imported.

## Top risks

1. **UI depth.** Every workflow exists, but the panels are simpler than Publisher's ribbon and galleries, and nobody from the target audience has used the app yet.
2. **Performance.** Layout covers the whole document and the display copy is rebuilt per change. Long booklets and large merges are untested.
3. **Core complexity.** The layout engine and the display list (effects, WordArt, tagging, separations) are dense, and only journeys pin them down.
4. **Dependencies.** egui, krilla, krilla-svg, resvg/usvg and calamine are pinned and move quickly; so is the toolchain (1.97.0).
5. **Agent sessions.** Parallel worktrees can exhaust disk. The lead clears the shared target directory after each batch.

## Recommended next steps

1. Fix RTL copy and paste: make every PDF tagged and add ActualText.
2. Add a veraPDF check in CI for PDF/UA and PDF/X-4.
3. On-canvas Bézier point editing, and a freeform drawing tool in the app.
4. Real-device tests: printing on each OS, and opening XPS and EPUB output in viewers.
5. Incremental layout for performance on long documents.
6. Usability testing with the target users (newsletter and bulletin makers), then a polish pass on the UI.
