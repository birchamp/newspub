# newpub-rs: status report

**Date:** 2026-10-09.
**Commit:** 6c25bc8, branch `claude/happy-davinci-r0kfpv`.
**CI:** run 37870194049 is fully green: clippy, build and 122/122 journeys on Linux, macOS and Windows, the Linux conformance job, and the native-print jobs on Windows and macOS.

**All 130 PARITY items, P0–P3 plus PF-01, pass their journeys on all three OSes.** IM-11 (EMF/WMF) was split out of IM-09 so that each could be checked honestly.

## External conformance (CI jobs `conformance` on Linux, `native-print` on Windows and macOS)

| Check | Tool | Result |
|-------|------|--------|
| PDF/UA-1: the accessibility journey, a table with header rows, and the newsletter, flyer, bulletin and booklet templates | veraPDF | compliant |
| EPUB 3 fixed layout | epubcheck 5.1.0 | valid |
| XPS | libgxps `xpstopdf` (independent renderer) and pdftotext | renders, text intact |
| XPS (Windows) | Windows' own XPS reader (`System.Windows.Xps`) | opens, pages counted |
| Print hand-off (Linux) | `lp`, then CUPS, then the cups-pdf virtual printer | job printed, text intact |
| Print (Windows) | native GDI job to "Microsoft Print to PDF" | PDF written |
| Print (macOS) | `lp`, then CUPS with the generic PostScript driver, then a socket queue to a local listener | job printed, text intact (re-encoded by the PostScript conversion) |
| PDF/X-4 | none (no free validator exists) | not externally validated |

## Parity by area

| Area | Items | Notes |
|------|-------|-------|
| PG page setup, masters, publication types | 11/11 | Cards, labels and badges print several per sheet; envelopes |
| TF text frames | 11/11 | |
| TY typography | 19/19 | Includes text effects (shadow, outline, glow, reflection, emboss) and WordArt with warps |
| SH shapes | 8/8 | Includes the freeform tool and on-canvas point editing; older polyline shapes convert to Bézier on first edit |
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
| PF performance | 1/1 | Incremental layout: ~10 ms per keystroke on a 60-page story |

## Known gaps

Each of these is real behaviour that a journey does not cover, or that is only partly done.

1. **PDF/X-4** has no external validator. Our own checks cover the output intent, the ICC profile, the XMP identification and the boxes.
2. **Printing** to a physical printer has not been tried. The CI jobs print to virtual printers only. On Windows, pages print as images at the printer's resolution (up to 600 dpi), not as vector output.
3. **XPS** opens in Windows' XPS reader and renders in libgxps, but nobody has looked at it in XPS Viewer. GIF and WebP pictures are dropped from XPS. EPUB does not embed .ttc fonts.
4. **EMF/WMF** conversion covers the common drawing records only. Bitmaps, clipping and gradients inside metafiles are skipped.
5. **Separations** ignore overprint on group children. Composite PDFs carry no overprint flag.
6. **Tagged PDF**: table headers are always column headers (no row headers). PDF/UA is validated for the built-in templates and the journey documents, not for user publications.
7. **.pub import** covers text, frames, pictures, fonts and basic formatting only.
8. **Incremental layout** does not apply to stories with fields (page numbers, dates, merge fields) or to table cells. These are always laid out in full.

Fixed since the previous report:
- Legacy polyline point editing: the shape converts to Bézier on its first edit.
- TH with column scope in tagged tables; veraPDF runs over every built-in template.
- Truly incremental layout: a story reflows from its first changed paragraph and stops when it rejoins its previous flow.
- Native Windows printing through GDI, checked in CI; XPS checked by Windows' own reader.

## Top risks

1. **UI depth.** Every workflow exists, but the panels are simpler than Publisher's ribbon and galleries, and nobody from the target audience has used the app yet.
2. **Performance.** Layout is incremental per story, but the display copy of the document is still rebuilt on every change. Large merges are untested.
3. **Core complexity.** The layout engine and the display list (effects, WordArt, tagging, separations) are dense, and only journeys pin them down.
4. **Dependencies.** egui, krilla, krilla-svg, resvg/usvg and calamine are pinned and move quickly; so is the toolchain (1.97.0).
5. **Agent sessions.** Parallel worktrees can exhaust disk. The lead clears the shared target directory after each batch.

## Recommended next steps

1. **Usability testing with real target users** (blocker B-003 in PROGRESS.md: it needs people). The kit is in docs/usability-test-plan.md. Then a UI polish pass based on what they find.
2. **A physical-printer check** on Windows and macOS, and a look at the XPS in XPS Viewer, by a person with the hardware.
3. **Vector printing on Windows** (EMF spool or XPS Print API) instead of page images, if print shops ask for it.
4. **Incremental layout for stories with fields**, by keying page-dependent paragraphs on their page.
