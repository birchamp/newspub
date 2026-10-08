# newpub-rs architecture

A fresh session should be able to resume work from this file, PARITY.md, PROGRESS.md, and `journeys/` alone.

## 0. Context and assumptions

- **Predecessor.** `birchamp/newspub` held only a README when the rewrite began ("Simple desktop publishing
  app using Pretext as the layout engine"). There was no document model or UX to carry over. The single
  inherited decision is the **Pretext approach to text layout**: measure text segments once, then lay them
  out with cheap arithmetic, never through a DOM. Our layout engine keeps that approach (§5).
- **Repository.** newpub-rs lives in this repository (`birchamp/newspub`) as a Cargo workspace. The brief's
  `<<repo URL>>` was left blank, and this is the only repository in scope.
- **Target users (assumed).** Small organisations, schools, and churches that make newsletters, bulletins,
  flyers, and folded booklets. P0/P1 ordering in PARITY.md follows from this. Change it here if the
  assumption is wrong.
- **Dashboard publishing.** The dashboard is GitHub Pages, built by CI. It is also mirrored to a claude.ai
  Artifact while sessions run. The brief's `<<GitHub Pages / local path>>` was left blank.

## 1. Crates

| Crate | Path | Role | May depend on |
|-------|------|------|---------------|
| `newpub-core` | crates/core | Document model, units, colours, styles, `Command` enum, `apply`, undo `History`. No IO, no fonts, no UI. | serde |
| `newpub-layout` | crates/layout | Font store, shaping, line breaking, hyphenation, frame flow, columns, wrap. Produces a `DocLayout`. | core |
| `newpub-render` | crates/render | Paints a page (or imposed sheet) from `Document` + `DocLayout` to a raster. Also provides the shared `PageGeometry` and the display-list walk used by PDF. | core, layout |
| `newpub-io-pdf` | crates/io-pdf | PDF export (krilla): boxes, bleed, marks, imposition, CMYK/spot, PDF/X, tagging. | core, layout, render (display list only) |
| `newpub-io-native` | crates/io-native | `.npub` save/open (zip: `document.json` + `media/*`). | core |
| `newpub-io-pub` | crates/io-pub | MS Publisher `.pub` import (research track). | core |
| `newpub-engine` | crates/engine | `Session`: owns the document, history, fonts, and layout cache. Executes `Action`s (commands, undo/redo, save/open, export, autoflow, merge) and answers `Query`s. The app and the journey runner **both** drive the program only through this. | everything except app |
| `newpub-app` | crates/app | egui/eframe desktop app. A thin view over `Session`: every UI gesture becomes an `Action`. Exposes `NewpubApp` as a lib for UI journeys. | engine, core |
| `newpub-journeys` | crates/journeys | Headless runner (`newpub-journeys` binary) and UI-journey harness. Scripts live in `/journeys/scripts`. | engine, app |
| dashboard | tools/dashboard | Python 3 stdlib script that writes `dashboard/index.html`. | — |

`newpub-engine` is an addition to the crate list in the brief. It exists so the app and the headless runner share
one dispatch path (rationale: "every user action is a command" can only be checked if the UI cannot bypass it).

## 2. Crate choices (one line each)

- **UI: egui/eframe 0.36.** Immediate mode keeps the UI a thin view over `Session`. It has built-in AccessKit on all three
  OSes, and `egui_kittest` drives the real UI headlessly through the AccessKit tree, which makes UI journeys possible
  in CI without a display server.
- **Raster rendering: tiny-skia 0.12.** CPU-only and deterministic across OSes, so golden images stay stable. Vello needs a GPU (and
  vello_cpu is still young).
- **Shaping: rustybuzz 0.20 + ttf-parser 0.25.** Same versions krilla uses (one copy of each in the tree); full OpenType
  feature control for kerning, ligatures, and small caps.
- **Text layout: our own engine (`newpub-layout`).** parley and cosmic-text lay out a paragraph at one fixed width. Publisher
  needs a different width per line (wrap around objects, drop caps) and lines that continue across columns and frames.
- **Line breaking: unicode-linebreak** (UAX #14). **Hyphenation: hyphenation** (Knuth–Liang, en-US embedded).
- **Fonts: fontdb 0.24** for system fonts in the app. Journeys load **only** the bundled fonts in `assets/fonts`
  (Carlito ≈ Calibri metrics, Liberation Serif/Sans ≈ Times/Arial metrics, DejaVu Sans as fallback; all OFL or
  Bitstream-Vera licensed) so output is identical on every OS.
- **PDF output: krilla 0.8.** Font subsetting, PDF/A, PDF/UA tagging, and validation for export profiles. Actively maintained.
- **PDF inspection in journeys: lopdf** (structure: pages, boxes, fonts) + **hayro** (rasterise exported PDFs for pixel checks).
- **Images: image 0.25** (PNG/JPEG decode for raster rendering; krilla embeds the original bytes).
- **Native format: zip + serde_json.** Human-diffable JSON, media kept as separate entries.
- **.pub import: cfb 0.15** (OLE compound files); see §9.
- **Spell check: spellbook 0.4** (pure-Rust Hunspell-compatible, MPL-2.0 used as an unmodified dependency) with the bundled SCOWL en_US dictionary (`assets/dict`, permissive SCOWL licence).
- **Toolchain pinned** to Rust 1.97.0 (`rust-toolchain.toml`), so clippy lints are identical locally and in CI.

## 3. Document model (`newpub-core`)

All geometry is in **points** (`f64`, 1/72 in), page coordinates, origin top-left, y down.
All ids are `Id(u64)` from one counter per document (`Document::next_id`), so any object, story, page, master, style, or
asset can be named by one integer.

```
Document
├── setup: PageSetup { width, height, margins{top,bottom,inside,outside}, facing, bleed }
├── pages: Vec<Page { id, master: Option<Id>, ignore_master, background: Option<Color>, objects: Vec<Id> /* back→front */ }>
├── masters: Vec<Master { id, name, objects: Vec<Id> }>
├── objects: BTreeMap<Id, Object { id, name, rect, rotation, kind, wrap, alt_text, decorative, locked, layer }>
│     kind = Text(TextFrame{story, columns, gutter, insets, valign, autofit}) | Shape(Shape{..}) | Image(ImageFrame{asset, crop, fit}) | Table(..) | Group(..)
├── stories: BTreeMap<Id, Story { id, text: String, chars: Vec<CharSpan>, paras: Vec<ParaAttrs>, frames: Vec<Id> }>
├── styles: Styles { para: BTreeMap<Id, ParaStyle>, chars: BTreeMap<Id, CharStyle> }
├── assets: BTreeMap<Id, Asset { name, mime, bytes: Arc<[u8]> /* not in JSON; media/ in .npub */ }>
└── next_id
```

**Stories are attributed strings.** `text` uses `'\n'` as the paragraph separator. `chars` is a contiguous list of
`CharSpan { len, attrs }` (lengths in chars) that covers the text exactly. `paras` has one `ParaAttrs` per paragraph
(always `text.matches('\n').count() + 1`). All text positions in the API are **char indices** into `text`. The model
normalises itself after each edit (merges equal adjacent spans, drops empty ones).

**Formatting resolution** (lowest to highest priority): built-in defaults → paragraph style chain (`based_on`) → that
style's char attrs → character style chain → run overrides. Attrs are structs of `Option<T>`; `None` means inherit.

**Linked frames.** A story owns the ordered chain `frames`. Linking B after A requires B's story to be empty; B's story
is deleted and B joins A's chain. Unlinking A splits the chain after A; the frames after A keep reading A's story
(Publisher behaviour: the text stays in the story and goes to overflow).

## 4. Command layer

- `core::Command` is a serde enum (`#[serde(tag = "cmd")]`, snake_case). It holds every **document mutation**. `Document::apply(&Command) -> Result<Applied, CoreError>`
  where `Applied { created: Vec<Id> }`.
- `engine::Action` = `Command(Command)` plus session-level actions: `undo`, `redo`, `begin_group`/`end_group`,
  `save`, `open`, `export_pdf`, `export_png`, `autoflow`, `new_document`, …
- `engine::Query` holds read-only questions answered as `serde_json::Value`: `page_count`, `story_text`, `frame_text`,
  `overflow`, `object`, `char_attrs_at`, `frame_lines`, … Journeys assert only through queries and exported files.
- Every command is undoable. **Undo model (lead-owned):** snapshot history. Before each top-level action the session
  stores the previous `Document` (cheap: assets are `Arc`, everything else is small). Typing commands with
  `coalesce: true` that continue the previous insert at its end position merge into one step. `begin_group`/`end_group`
  make compound actions such as autoflow and replace-all into one step. Snapshots were chosen over inverse commands
  because every new command is then undoable automatically, with no per-command inverse code to get wrong. History is capped at 200 steps.

**Interface rule:** changes to `Command`, `Action`, `Query`, or the model types in `core` are made by the lead only and
recorded in §11.

## 5. Layout engine (`newpub-layout`, lead-owned)

The Pretext approach applied to frames:
1. **Itemise:** split each paragraph into style runs, then into font-coverage runs (fallback fonts).
2. **Shape** each run once with rustybuzz (features from attrs: `kern`, `liga`, `dlig`, `smcp`, …). Cache by (face, size, features, text).
3. **Segment:** UAX #14 break opportunities give segments; each segment records advance width, trailing-space width,
   and glyph slice. Hyphenation points inside long words become extra optional break points with hyphen width.
4. **Flow:** a `Region` iterator yields line slots: for each frame in the story's chain, each column, the next line top `y`.
   The available x-intervals at `[y, y+lineheight]` come from the column minus wrap exclusions of objects above it in z-order.
   The greedy first-fit line breaker (Publisher's behaviour) fills slots with segments. A line that does not fit vertically moves on to
   the next column, then the next frame. Whatever is left is **overflow** (`StoryLayout.overflow_at: Option<char index>`).
5. **Finish lines:** alignment and justification (space distribution), tabs, drop caps (an exclusion box for N lines),
   then per-frame vertical alignment.

Output: `DocLayout { frames: HashMap<Id, FrameLayout { lines: Vec<Line> }>, stories: HashMap<Id, StoryLayout> }` with glyph
positions in **frame-local** coordinates (the renderer applies the frame transform, so rotation works).

## 6. Rendering and export

`newpub-render` builds a per-page **display list** (`Vec<DisplayItem>`: path fill/stroke, image, glyph run, clip, transform)
from model + layout. Two back ends consume it: tiny-skia (PNG, screen) and krilla (PDF). This keeps screen and PDF
output consistent. Imposition (booklet, n-up) maps pages onto sheets as transforms over display lists.

## 7. Journeys (the only tests)

- Scripts: `journeys/scripts/<ID>.yaml`. Fixtures: `journeys/fixtures/`. Approved goldens: `journeys/goldens/<ID>/<name>.png`.
- Runner: `cargo run -p newpub-journeys --release -- [--filter ID] [--out DIR]`. It writes `DIR/<ID>/…` (exports and
  page screenshots) and `DIR/results.json`.
- Step kinds: an **action** (`- add_text_frame: {...}`), optionally `as: name` to bind the created id, used later as
  `"$name"`; **`expect`** with a `query` and a matcher (`equals`, `contains`, `gt`, `lt`, `approx`, `len`);
  **`expect_pdf`** (pages, text, fonts, boxes, pixel probes via hayro); **`expect_png`** (golden compare with
  tolerance); **`expect_roundtrip`** (save → open → document equality).
- Lengths in scripts may be numbers (pt) or strings with units: `"8.5in"`, `"210mm"`, `"2cm"`, `"12pt"`, `"3pi"`.
- **Golden approval:** a missing golden writes `DIR/<ID>/pending/<name>.png` and fails with `needs-approval`. Only the lead
  copies pending images into `journeys/goldens/`.
- UI journeys (`UI-*.yaml`, `ui: true`) run the real `NewpubApp` in `egui_kittest`, find widgets by AccessKit
  label/role, click and type, then assert through the same queries.
- Agents implementing features **never** add or edit anything under `journeys/` or `crates/journeys/`.

## 8. CI

`.github/workflows/ci.yml` runs on ubuntu-latest, macos-latest, and windows-latest: `fmt --check`, `clippy --all-targets -D warnings`,
build, and the full journey suite. Each OS uploads `journey-results-<os>` (results.json, screenshots, PDFs). A final job merges
the three result sets with per-OS job status, runs `tools/dashboard`, uploads it as an artifact, and deploys it to GitHub Pages.

## 9. .pub import research track

Risk: **high**. The format is undocumented. Known facts: it is an OLE compound file with `Contents`, `Quill/QuillSub/CONTENTS`
(text, in a Word-like piece table), `Escher/*` (Office Drawing records), and an `EnvelopeData` stream.
libmspub (MPL-2.0, LibreOffice) documents much of the structure through its source. **We may read it to understand the format but must
not copy its code**, because MPL is file-level copyleft. Plan: PI-01 (container and stream report) → PI-02 (text) → PI-03 (geometry and
images) → PI-04 (formatting). Stop and ask a human if the approach needs code that would be derived from libmspub.

## 10. Conventions

- Rust 2024, stable. `cargo clippy --all-targets -- -D warnings` must be clean.
- No `unwrap()` on user-controlled data in core, layout, or io. Return `CoreError` / `anyhow` at the edges.
- Feature work must not add placeholders. Return `Err(Unsupported)` until a feature really works.
- No Microsoft code, assets, or templates. Built-in templates and building blocks are original designs.

## 11. Interface change log

| Date | Change | Reason |
|------|--------|--------|
| 2026-10-08 | Initial `Command`/`Action`/`Query` sets defined (see crates/core/src/command.rs, crates/engine/src/action.rs) | Phase 0 |
