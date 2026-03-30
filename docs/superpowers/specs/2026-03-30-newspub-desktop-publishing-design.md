# NewsPub: Desktop Newsletter Publishing App

## Overview

NewsPub is an Electron desktop app for creating multi-page newsletters. It uses [pretext](https://github.com/chenglou/pretext) as its text measurement and layout engine, rendering documents on an HTML5 Canvas with a React UI shell. Users start from templates, place and edit text in linked frames that flow across pages, position images with text wrap, and export to PDF.

## Goals

- Easy to use newsletter creation for non-technical users
- Precise text layout powered by pretext's Canvas-based measurement
- Professional output: multi-column layouts, text threading across pages, image wrap
- Template-driven workflow with full editability after template application

## Non-Goals (v1)

- HTML/email export (PDF only for v1)
- Contour text wrap around image shapes (rectangle wrap and skip only)
- Collaborative editing
- Plugin/extension system
- Color profile management or CMYK output

---

## 1. Application Architecture

### Platform

Electron desktop app (Mac/Windows/Linux). Electron provides a full Chromium environment where pretext runs natively with consistent Canvas `measureText()` behavior across platforms. This consistency is critical for a publishing app where layout precision matters.

### Process Model

- **Main process:** File I/O (save/load `.newspub` files, PDF export), window management, native menus, recent files list.
- **Renderer process:** React UI shell and Canvas document engine. All layout computation and rendering happens here since pretext is a renderer-process library.

### Rendering Approach

Hybrid Canvas + hidden textarea. The document canvas is an HTML5 `<canvas>` element where pretext measures text and everything is rendered via Canvas 2D API. Text input is captured via a hidden `<textarea>` positioned off-screen that follows the cursor. This gives rendering precision of canvas-based output while inheriting OS-level spellcheck, IME composition, and clipboard handling from the hidden textarea. This is the same approach used by VS Code and Google Docs.

### Module Breakdown

| Module | Responsibility |
|--------|---------------|
| **Document Model** | Source of truth: pages, frames, text content, styles, threading links. Pure data, no rendering. |
| **Layout Engine** | Wraps pretext. Given a document model, computes line breaks, thread overflow, frame-to-frame flow, image skip/wrap regions. |
| **Canvas Renderer** | Reads layout engine output, draws everything to the canvas: text lines, images, frame borders, selection, overflow indicators. |
| **Input Manager** | Hidden textarea + keyboard/mouse event handling. Translates user actions into document model mutations. |
| **History Manager** | Undo/redo stack with action grouping. Captures document model diffs. |
| **Template Manager** | Loads/saves templates, applies template to new document, derives template from existing project. |
| **File Manager** | Serializes/deserializes `.newspub` zip files. Handles embedded image storage. |
| **PDF Exporter** | Walks the layout engine output and writes to PDF via `pdf-lib`. |
| **UI Shell (React)** | Toolbar, property panels, page thumbnail sidebar, dialogs. Communicates with document model via event bus. |

### Data Flow

```
User action → Input Manager → Document Model mutation → Layout Engine recomputes → Canvas Renderer redraws
                                    ↓
                              History Manager records
```

The layout engine only recomputes affected frames when a mutation occurs. Typing in a frame on page 2 reflows that frame's thread, not unrelated frames on page 1.

---

## 2. Document Model

The document model is a plain JSON-serializable data structure. No classes, no methods — just data. This makes undo/redo (snapshot diffs), serialization, and template extraction straightforward.

### Structure

```
Document
├── metadata
│   ├── title, author, created/modified dates
│   └── pageSize: { width, height } (in points)
├── pages[]
│   ├── id
│   ├── backgroundImage? (decoration layer)
│   └── frames[]
│       ├── TextFrame { id, position, size, threadId, threadOrder, styleOverrides }
│       └── ImageFrame { id, position, size, imageAssetId, wrapMode: "skip"|"rect" }
├── threads{}
│   └── [threadId]: { articleContent (rich text as styled runs), styles }
├── assets{}
│   └── [assetId]: { filename, mimeType, data (binary ref into zip) }
└── templates{}
    └── (stored separately, same structure minus content per user choice)
```

### Key Design Decisions

**Text content lives on threads, not frames.** A thread is an article. It contains the full rich text. Frames are windows into a thread — the layout engine decides what portion of the thread's text appears in each frame based on available space. This means:

- Relinking frames to different threads is a pointer swap.
- Thread overflow detection: layout engine ran out of frames before running out of text.
- "Cont. pg X" is derived from the next frame in the thread's ordered frame list.

**Rich text format:** A flat array of styled runs. Each run is `{ text: string, style: { bold?, italic?, fontSize?, fontFamily?, color?, listType?, indent?, ... } }`. Flat runs are simpler to split at line boundaries and easier for pretext to consume than a nested DOM-like tree.

**Frame positioning:** Absolute `{ x, y, width, height }` in points (1/72 inch), relative to the page. Points are the natural unit for print — PDF uses them natively.

**Page sizes (v1):**

| Name | Dimensions |
|------|-----------|
| US Letter | 612 × 792 pt (8.5 × 11") |
| US Legal | 612 × 1008 pt (8.5 × 14") |
| US Tabloid | 792 × 1224 pt (11 × 17") |
| A4 | 595 × 842 pt (210 × 297mm) |
| A5 | 420 × 595 pt (148 × 210mm) |
| Custom | User-defined width × height |

Page size is set at project creation (template defines a default) and changeable afterward via Document Setup. Changing page size reflows all text and scales frame positions proportionally by default, with an option to keep absolute positions.

The UI shows inches or millimeters based on a user preference.

**Spread awareness:** The document model does not know about spreads. Pages are ordered, and the UI pairs them into spreads (page 1 solo right-hand, 2+3 spread, 4 solo left-hand, etc.).

---

## 3. Layout Engine

The layout engine bridges the document model and rendering. Given a thread's rich text content and an ordered list of text frames (potentially across multiple pages), it computes exactly which text appears in each frame, where each line sits, and whether there's overflow.

### Pipeline

```
Thread content (styled runs)
    ↓
1. Convert styled runs → pretext segments
   (group by font/size/style, call prepare() or prepareWithSegments() for each)
    ↓
2. For each frame in thread order:
   a. Compute available region (frame rect minus padding)
   b. Compute exclusion zones (overlapping ImageFrames with "rect" wrap)
   c. For "skip" mode: if image overlaps, reduce frame height around the gap
   d. For "rect" mode: use layoutNextLine() with varying width per line
   e. Fill lines until frame is full or text runs out
   f. Record cursor position (where we stopped in the thread)
    ↓
3. Pass cursor to next frame, continue from where we left off
    ↓
4. After last frame: if text remains → overflow = true
```

### Why `layoutNextLine()` Is Key

pretext's `layoutNextLine()` function accepts a different `maxWidth` for each line. When an image overlaps lines 5–12 of a frame, those lines get a narrower width. Lines before and after get the full frame width. This is how professional DTP text wrap works.

### Text Threading and Continuation Markers

When the layout engine moves from one frame to the next and those frames are on different pages, it emits a continuation marker: `{ fromPage: 1, toPage: 4 }`. The renderer draws "Cont. pg 4" at the bottom of the first frame and optionally "Cont. from pg 1" at the top of the second.

### Overflow Detection

When the last frame in a thread can't fit all remaining text, the layout engine sets `overflow: true` on that frame. The renderer draws a visible overflow indicator (red icon on the frame edge) telling the user they need to link another frame, edit for length, or resize.

### Incremental Reflow

When text changes in a thread, only the first affected frame forward in that thread is re-laid out. Frames belonging to other threads are untouched. This keeps editing responsive in longer newsletters.

---

## 4. Canvas Renderer

The renderer takes layout engine output and draws everything to an HTML5 Canvas.

### Render Order (back to front)

1. **Page background** — white rectangle or custom page color
2. **Background/decoration images** — wreath borders, watermarks, banners. Sit behind everything, no text interaction.
3. **Image frames** — placed images with optional borders/shadows
4. **Text lines** — drawn with `ctx.fillText()` using positions from the layout engine. Each styled run gets its own draw call.
5. **Drop caps, pull quotes** — special rendering for decorative text elements
6. **Continuation markers** — "Cont. pg 4" / "Cont. from pg 1" at frame edges
7. **Overflow indicator** — red icon on the last frame's edge when text overflows the thread
8. **Selection layer** — blue highlight rectangles behind selected text
9. **Frame chrome** — borders, resize handles, selection outlines (edit mode only)
10. **Cursor** — blinking caret positioned using pretext's character measurement

### Rendering Modes

| Mode | Purpose | Details |
|------|---------|---------|
| **Edit mode** | On-screen editing | Shows frame borders, handles, cursor, overflow indicators, selection, snap guides |
| **Export mode** | PDF generation | No chrome, no cursor, no handles. Clean output only. |

The export mode renderer writes to a `pdf-lib` document instead of a screen canvas but walks the same layout data. One layout computation, two render targets.

### Viewport and Zoom

- A camera transform (`scale`, `translateX`, `translateY`) maps document coordinates (points) to screen pixels
- Zoom range: 25%–400%, with common stops at 50%, 75%, 100%, 150%
- Spread view shows two pages side by side, fitting them to the viewport width

---

## 5. Input Manager and Text Editing

### Architecture

```
Hidden <textarea>        Canvas
(off-screen, focused)    (visible to user)
       │                      │
       ├─ keydown/input ──→ Document Model mutation ──→ Re-layout ──→ Re-render
       ├─ compositionstart ──→ Show IME inline on canvas
       ├─ paste ──→ Parse clipboard (plain text + basic HTML) ──→ Insert styled runs
       │
Mouse/touch on canvas ──→ Hit test (which frame? which character?) ──→ Update selection
                          ──→ Reposition hidden textarea to follow cursor
```

### Hit Testing

When the user clicks on the canvas, the layout engine's output provides every line's position and which thread/run it belongs to. Walk the lines in the clicked frame, find the matching line by Y coordinate, then use pretext's character widths to find the exact character offset by X coordinate.

### Selection Model

- Selection stored as `{ threadId, anchor: offset, focus: offset }` — two positions in the thread's flat text.
- Selection can span across frames (since frames are windows into the same thread). The renderer draws highlight rectangles in each affected frame.
- Shift+click and shift+arrow extend the selection.
- Double-click selects a word, triple-click selects a paragraph.

### Clipboard

- **Paste:** Accept plain text and basic HTML. Parse HTML into styled runs (map `<b>` → bold, `<i>` → italic, etc.). Strip unsupported formatting.
- **Copy/Cut:** Write both plain text and styled-run HTML to the clipboard so round-tripping within the app preserves formatting.

### What the Hidden Textarea Provides

- OS-level spellcheck and autocorrect
- IME composition for CJK, emoji, etc.
- OS keyboard shortcuts (Cmd+A, Cmd+C, etc.)
- Screen reader text buffer (partial accessibility win)

---

## 6. History Manager (Undo/Redo)

### Approach

Command pattern with grouping. Each user action produces a command object that knows how to apply and reverse itself.

### Command Structure

```
Command {
  type: "text-insert" | "text-delete" | "frame-move" | "frame-resize" |
        "frame-add" | "frame-delete" | "style-change" | "image-place" | ...
  do:   patch to apply (forward)
  undo: patch to reverse
  groupId?: string
  timestamp: number
}
```

### Grouping Rules

- **Dragging a frame:** All intermediate position changes during a single drag share a `groupId` → one undo step.
- **Typing:** Consecutive character inserts within the same frame, without pausing for >1 second, share a `groupId` → one undo step undoes the whole burst.
- **Style changes to a selection:** Bold + color applied together share a `groupId`.
- **Paste:** One undo step regardless of content size.

### Stack Behavior

- Undo pops the most recent group, applies all `undo` patches in reverse order.
- Redo re-applies the group's `do` patches in forward order.
- Any new action after an undo clears the redo stack.

### Why Patches Instead of Snapshots

A newsletter with embedded images could be 50+ MB. Snapshotting the whole document on every keystroke is wasteful. Patches are small — typically a text diff or a position change.

---

## 7. Template System

### What a Template Is

A `.newspub` file with layout and decoration intact but content optionally stripped. Same format, same zip structure.

### Template Sources

| Source | How it works |
|--------|-------------|
| **Preset templates** | Ship with the app in the bundle's `templates/` directory. Classic newsletter layouts (4-page, 8-page, single-column, multi-column, etc.) |
| **Save as template** | User opens an existing project → File > Save as Template → dialog with content options → saved to user templates directory |
| **New pages from template** | When adding a spread, user picks a page-pair template from presets or extracted from existing pages |

### Template Creation Options

When saving as template, a dialog presents three independent checkboxes:

- **Keep article text** — if unchecked, all thread content is stripped (empty text frames remain)
- **Keep placed images** — if unchecked, image frames remain but their content is removed
- **Keep background/decoration images** — checked by default. Preserves page backgrounds, borders, banners (e.g., wreath border, header/footer images)

### Empty Frame Indicators

After applying a template, empty frames show distinct visual indicators:

- **Empty text frames:** Purple dashed border with a text/paragraph icon and label (e.g., "Headline", "Body Text")
- **Empty image frames:** Orange dashed border with a photo icon and "Image" label
- Indicators disappear once content is placed.

### Template Application Flow

1. User creates new document → template picker shows presets + user templates as thumbnails.
2. Selecting a template creates a new document with the template's layout, frames, and any kept content.
3. User fills in content and adjusts layout as needed.

### Page-Level Templates

When adding a spread mid-document, the user picks a 2-page template (or single page if adding to the end). This inserts new pages with pre-placed frames, background images, and style defaults.

### Template Metadata

```json
{
  "name": "Classic 4-Page Newsletter",
  "description": "Front page hero, 2-3 spread with 3 columns, back page contacts",
  "pageSize": { "width": 612, "height": 792 },
  "pageCount": 4,
  "thumbnail": "<generated preview image>"
}
```

---

## 8. File Manager (.newspub Format)

### Format

A standard ZIP file with a `.newspub` extension.

### Internal Structure

```
document.json          ← Document model (pages, frames, threads, styles)
metadata.json          ← Title, author, dates, page size, version
thumbnail.png          ← Auto-generated preview for OS file browsers / recent files
assets/
  ├── img-001.jpg      ← Placed images
  ├── img-002.png      ← Background/decoration images
  └── ...
```

### Operations

| Operation | Behavior |
|-----------|----------|
| **Save** | Serialize document model to JSON, bundle with assets into zip, write atomically (write to `.tmp`, then rename to prevent corruption on crash) |
| **Open** | Unzip to a temp working directory, parse `document.json`, load asset references. Images loaded lazily as pages are viewed. |
| **Auto-save** | Every 60 seconds if dirty. Writes to a `.newspub.autosave` sidecar file. On crash recovery, prompt user to restore. |
| **Save as Template** | Same save flow but applies content stripping options from the template dialog |

### Image Handling

Images are stored at original resolution in the zip. The canvas renderer generates display-resolution thumbnails for editing performance. Full resolution is used for PDF export.

Zip compression is applied but already-compressed formats (JPEG, PNG) are stored without re-compression. `document.json` and `metadata.json` benefit from zip compression as text.

---

## 9. PDF Exporter

### Approach

Walk the same layout engine output that the canvas renderer uses, but write to `pdf-lib` instead of screen canvas. One layout computation, two render targets.

### Page-by-Page Process

1. Create a new PDF page at the document's page size
2. Draw background/decoration images
3. Draw placed image frames
4. Draw text lines via `pdf-lib`'s `drawText()` with font, size, color, position
5. Draw continuation markers ("Cont. pg 4")
6. No frame chrome, cursor, or selection — clean output only

### Font Embedding

PDF requires fonts to be embedded. On export:

- Scan all styled runs for unique font families.
- Load TTF/OTF font files and embed via `pdf-lib`.
- Fall back to standard PDF fonts (Helvetica, Times, Courier) if a system font can't be embedded.
- Show a warning if a font can't be embedded.

### Measurement Consistency

pretext computes line breaks once using Canvas `measureText()`. Both the screen canvas and the PDF use those same break points. What you see is what you get.

### Export Options (v1)

- Output path
- Page range (all pages, or specific pages)

---

## 10. UI Shell (React)

### Layout

```
┌──────────────────────────────────────────────────────┐
│  Menu Bar (File, Edit, View, Insert, Format, Help)   │
├──────────────────────────────────────────────────────┤
│  Toolbar (text formatting, zoom, frame tools)        │
├──────┬──────────────────────────────┬────────────────┤
│      │                              │                │
│  P   │                              │  Properties    │
│  a   │      Document Canvas         │  Panel         │
│  g   │      (spread view)           │  (context-     │
│  e   │                              │   sensitive)   │
│  s   │                              │                │
│      │                              │                │
├──────┴──────────────────────────────┴────────────────┤
│  Status Bar (zoom %, page X of Y, word count)        │
└──────────────────────────────────────────────────────┘
```

### Page Thumbnails Sidebar (left)

- Pages grouped as spreads: page 1 (right-hand solo), pages 2–3 (spread), page 4 (left-hand solo)
- Click to navigate — canvas scrolls to that spread
- Right-click for page operations: add spread, delete spread, duplicate, apply template
- Drag to reorder pages (moves in spread pairs)

### Properties Panel (right, context-sensitive)

| Selection state | Panel contents |
|----------------|----------------|
| Nothing selected | Document settings (page size, margins, guides) |
| Text frame selected | Frame position/size, thread info, linked frames list |
| Editing text | Font, size, bold/italic, color, alignment, list type, indentation, paragraph spacing |
| Image frame selected | Position/size, wrap mode (skip/rect), image fit (fill/fit/stretch), replace image |
| Background image selected | Position, opacity, lock toggle |

### Toolbar

- **Frame tools:** Select/move, draw text frame, draw image frame
- **Text formatting:** Font picker, size, B/I, color, alignment
- **Actions:** Undo/redo, zoom slider, export PDF

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Ctrl/Cmd+S | Save |
| Ctrl/Cmd+Z | Undo |
| Ctrl/Cmd+Shift+Z | Redo |
| Ctrl/Cmd+B | Bold |
| Ctrl/Cmd+I | Italic |
| Delete/Backspace | Remove selected frame or selected text |
| Arrow keys | Nudge selected frame by 1pt |
| Shift+Arrow | Nudge selected frame by 10pt |
| Tab | Cycle between frames in a thread |

---

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Desktop shell | Electron |
| UI framework | React |
| Text layout engine | @chenglou/pretext |
| Document rendering | HTML5 Canvas 2D API |
| PDF generation | pdf-lib |
| File format | ZIP-based .newspub (via fflate or similar) |
| State management | TBD (lightweight — React context or Zustand) |
| Build tooling | TBD (Vite + electron-builder likely) |
