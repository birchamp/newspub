# newpub-rs: status report

**Date:** 2026-10-09.
**Commit:** 545d57d, branch `claude/happy-davinci-r0kfpv`.
**CI:** run 37860525724. Clippy, build and 120/120 journeys pass on Linux, macOS and Windows, and the conformance job is green.

**All 130 PARITY items, P0–P3 plus the new PF-01, pass their journeys on all three OSes.** IM-11 (EMF/WMF) was split out of IM-09 so that each could be checked honestly.

## External conformance (CI job `conformance`, Linux)

| Check | Tool | Result |
|-------|------|--------|
| PDF/UA-1, accessibility journey and newsletter exports | veraPDF | compliant |
| EPUB 3 fixed layout | epubcheck 5.1.0 | valid |
| XPS | libgxps `xpstopdf` (independent renderer) and pdftotext | renders, text intact |
| Print hand-off | `lp`, then CUPS, then the cups-pdf virtual printer | job printed, text intact |
| PDF/X-4 | none (no free validator exists) | not externally validated |

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

1. **PDF/X-4** has no external validator. Our own checks cover the output intent, the ICC profile, the XMP identification and the boxes.
2. **Printing** is verified on Linux through CUPS. The macOS (`lp`) and Windows (PowerShell PrintTo) hand-offs have not run against a printer.
3. **XPS** renders in libgxps but has not been tried in Microsoft's XPS Viewer. GIF and WebP pictures are dropped from XPS. EPUB does not embed .ttc fonts.
4. **EMF/WMF** conversion covers the common drawing records only. Bitmaps, clipping and gradients inside metafiles are skipped.
5. **Separations** ignore overprint on group children. Composite PDFs carry no overprint flag.
6. **Tagged PDF**: tables have no TH or scope. PDF/UA is validated only for the two journey documents.
7. **.pub import** covers text, frames, pictures, fonts and basic formatting only.
8. **Freeform point editing** covers Bézier shapes. Older polyline `Path` shapes can enter point editing, but their edits are refused with a status message.

Fixed since the previous report:
- Right-to-left copy and paste: tagged export everywhere, with /ActualText.
- Layout performance: per-keystroke layout of a 20-page story went from 8 s to about 45 ms.
- On-canvas freeform drawing and point editing.

## Top risks

1. **UI depth.** Every workflow exists, but the panels are simpler than Publisher's ribbon and galleries, and nobody from the target audience has used the app yet.
2. **Performance.** Layout covers the whole document and the display copy is rebuilt per change. Long booklets and large merges are untested.
3. **Core complexity.** The layout engine and the display list (effects, WordArt, tagging, separations) are dense, and only journeys pin them down.
4. **Dependencies.** egui, krilla, krilla-svg, resvg/usvg and calamine are pinned and move quickly; so is the toolchain (1.97.0).
5. **Agent sessions.** Parallel worktrees can exhaust disk. The lead clears the shared target directory after each batch.

## Recommended next steps

1. **Usability testing with real target users** (blocker B-003 in PROGRESS.md: needs people). Then a UI polish pass based on what they find.
2. **Print and XPS on Windows and macOS**, checked by hand on a real printer and in XPS Viewer.
3. **Wider PDF/UA coverage:** table headers (TH, scope), and veraPDF across all built-in templates.
4. **Truly incremental layout:** re-flow only from the first changed paragraph. The caches make edits cheap today, but a 100-page story still re-flows in full.
5. **Point editing for legacy polyline shapes:** convert them to Bézier on the first edit.
