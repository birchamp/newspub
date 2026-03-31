# NewsPub Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a desktop newsletter publishing app using Electron, React, Canvas, and pretext for precise text layout with PDF export.

**Architecture:** Electron app with React UI shell. Documents rendered on HTML5 Canvas using pretext for text measurement. Hidden textarea captures input. Pure-data document model drives layout engine, which feeds both the canvas renderer and PDF exporter. Zustand for state management.

**Tech Stack:** Electron, React, TypeScript, @chenglou/pretext, pdf-lib, fflate, Zustand, Vite, Vitest, electron-builder

**Design Spec:** `docs/superpowers/specs/2026-03-30-newspub-desktop-publishing-design.md`

---

## File Structure

```
package.json
vite.config.ts                          # Vite config for renderer
electron.vite.config.ts                 # Electron-specific vite config
tsconfig.json
tsconfig.node.json

src/
  main/                                 # Electron main process
    main.ts                             # Entry point, window creation
    menu.ts                             # Native menu bar
    ipc-handlers.ts                     # IPC for file I/O, PDF export

  preload/
    preload.ts                          # Context bridge for IPC

  renderer/                             # Electron renderer process
    index.html
    index.tsx                           # React entry point
    App.tsx                             # Root layout component

    model/                              # Document model (pure data + functions)
      types.ts                          # All TypeScript types
      document.ts                       # Document CRUD operations
      thread.ts                         # Thread/styled-run manipulation
      page.ts                           # Page/frame operations
      page-sizes.ts                     # Page size constants

    engine/                             # Layout engine (wraps pretext)
      layout-engine.ts                  # Orchestrates full document layout
      frame-layout.ts                   # Single-frame text layout via pretext
      wrap-calculator.ts                # Image exclusion zone computation
      layout-types.ts                   # Layout output types (LayoutLine, etc.)

    canvas/                             # Canvas rendering
      canvas-renderer.ts                # Main render loop + layer orchestration
      text-painter.ts                   # Text line drawing
      image-painter.ts                  # Image + background drawing
      frame-chrome-painter.ts           # Handles, borders, overflow indicator
      cursor-painter.ts                 # Blinking caret
      viewport.ts                       # Camera transform, zoom, pan

    input/                              # Input handling
      input-manager.ts                  # Hidden textarea coordination
      hit-test.ts                       # Click → character position
      selection.ts                      # Selection state + manipulation
      clipboard.ts                      # Copy/paste/cut with styled runs

    history/                            # Undo/redo
      history-manager.ts                # Command stack with grouping
      commands.ts                       # Command type definitions + factories

    file/                               # File I/O
      file-manager.ts                   # .newspub zip pack/unpack
      serializer.ts                     # Document ↔ JSON
      autosave.ts                       # Dirty tracking + periodic save

    template/                           # Template system
      template-manager.ts              # Load/save/apply/strip templates
      preset-templates.ts              # Built-in template definitions

    export/                             # PDF export
      pdf-exporter.ts                   # Layout → pdf-lib document
      font-resolver.ts                  # CSS family → .ttf/.otf file path

    ui/                                 # React components
      components/
        Toolbar.tsx                     # Top toolbar (frame tools, formatting, actions)
        PageSidebar.tsx                 # Left sidebar with spread thumbnails
        PropertiesPanel.tsx             # Right panel (context-sensitive)
        StatusBar.tsx                   # Bottom bar (zoom, page, word count)
        DocumentCanvas.tsx              # Canvas host + event wiring
        TemplatePickerDialog.tsx        # New document template chooser
        SaveAsTemplateDialog.tsx        # Template save with content options
        DocumentSetupDialog.tsx         # Page size + settings
        ExportDialog.tsx                # PDF export options
      store/
        editor-store.ts                 # Zustand: document, selection, tool, zoom state

tests/
  model/
    document.test.ts
    thread.test.ts
    page.test.ts
  engine/
    layout-engine.test.ts
    frame-layout.test.ts
    wrap-calculator.test.ts
  input/
    hit-test.test.ts
    selection.test.ts
    clipboard.test.ts
  history/
    history-manager.test.ts
  file/
    file-manager.test.ts
    serializer.test.ts
  template/
    template-manager.test.ts
  export/
    pdf-exporter.test.ts
    font-resolver.test.ts

resources/
  templates/                            # Preset .newspub template files
    classic-4page.newspub
    simple-2page.newspub
```

---

## Task 1: Project Scaffolding

**Files:**
- Create: `package.json`
- Create: `tsconfig.json`
- Create: `tsconfig.node.json`
- Create: `electron.vite.config.ts`
- Create: `src/main/main.ts`
- Create: `src/preload/preload.ts`
- Create: `src/renderer/index.html`
- Create: `src/renderer/index.tsx`
- Create: `src/renderer/App.tsx`

- [ ] **Step 1: Initialize the project with electron-vite**

```bash
cd /Users/birch/Development/newspub
npm create @anthropic-ai/electron-vite@latest . -- --template react-ts
```

If the above scaffolder isn't available, initialize manually:

```bash
npm init -y
```

- [ ] **Step 2: Install core dependencies**

```bash
npm install electron electron-builder @electron-toolkit/preload @electron-toolkit/utils react react-dom zustand @chenglou/pretext pdf-lib fflate
npm install -D typescript vite @vitejs/plugin-react electron-vite vitest @testing-library/react @types/react @types/react-dom
```

- [ ] **Step 3: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "baseUrl": ".",
    "paths": {
      "@model/*": ["src/renderer/model/*"],
      "@engine/*": ["src/renderer/engine/*"],
      "@canvas/*": ["src/renderer/canvas/*"],
      "@input/*": ["src/renderer/input/*"],
      "@history/*": ["src/renderer/history/*"],
      "@file/*": ["src/renderer/file/*"],
      "@template/*": ["src/renderer/template/*"],
      "@export/*": ["src/renderer/export/*"],
      "@ui/*": ["src/renderer/ui/*"]
    }
  },
  "include": ["src/renderer/**/*", "tests/**/*"]
}
```

- [ ] **Step 4: Create tsconfig.node.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "noEmit": true
  },
  "include": ["src/main/**/*", "src/preload/**/*", "electron.vite.config.ts"]
}
```

- [ ] **Step 5: Create electron.vite.config.ts**

```typescript
import { defineConfig, externalizeDepsPlugin } from 'electron-vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  main: {
    plugins: [externalizeDepsPlugin()]
  },
  preload: {
    plugins: [externalizeDepsPlugin()]
  },
  renderer: {
    plugins: [react()],
    resolve: {
      alias: {
        '@model': resolve('src/renderer/model'),
        '@engine': resolve('src/renderer/engine'),
        '@canvas': resolve('src/renderer/canvas'),
        '@input': resolve('src/renderer/input'),
        '@history': resolve('src/renderer/history'),
        '@file': resolve('src/renderer/file'),
        '@template': resolve('src/renderer/template'),
        '@export': resolve('src/renderer/export'),
        '@ui': resolve('src/renderer/ui')
      }
    }
  }
})
```

- [ ] **Step 6: Create src/main/main.ts**

```typescript
import { app, BrowserWindow } from 'electron'
import { join } from 'path'
import { electronApp, optimizer } from '@electron-toolkit/utils'

function createWindow(): void {
  const mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    webPreferences: {
      preload: join(__dirname, '../preload/preload.js'),
      sandbox: false
    }
  })

  if (process.env.ELECTRON_RENDERER_URL) {
    mainWindow.loadURL(process.env.ELECTRON_RENDERER_URL)
  } else {
    mainWindow.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

app.whenReady().then(() => {
  electronApp.setAppUserModelId('com.newspub')
  app.on('browser-window-created', (_, window) => {
    optimizer.watchWindowShortcuts(window)
  })
  createWindow()
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
```

- [ ] **Step 7: Create src/preload/preload.ts**

```typescript
import { contextBridge, ipcRenderer } from 'electron'

contextBridge.exposeInMainWorld('electronAPI', {
  saveFile: (data: ArrayBuffer, path: string) =>
    ipcRenderer.invoke('file:save', data, path),
  openFile: (path: string) =>
    ipcRenderer.invoke('file:open', path),
  showSaveDialog: (options: object) =>
    ipcRenderer.invoke('dialog:save', options),
  showOpenDialog: (options: object) =>
    ipcRenderer.invoke('dialog:open', options),
  exportPDF: (data: ArrayBuffer, path: string) =>
    ipcRenderer.invoke('export:pdf', data, path),
  getSystemFontsDir: () =>
    ipcRenderer.invoke('system:fontsDir')
})
```

- [ ] **Step 8: Create src/renderer/index.html**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>NewsPub</title>
</head>
<body>
  <div id="root"></div>
  <script type="module" src="./index.tsx"></script>
</body>
</html>
```

- [ ] **Step 9: Create src/renderer/index.tsx**

```tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
```

- [ ] **Step 10: Create src/renderer/App.tsx**

```tsx
export default function App() {
  return (
    <div style={{ width: '100vw', height: '100vh', display: 'flex', flexDirection: 'column' }}>
      <div style={{ padding: 16, textAlign: 'center', color: '#666' }}>
        NewsPub — scaffolding complete
      </div>
    </div>
  )
}
```

- [ ] **Step 11: Add scripts to package.json**

Ensure `package.json` has these scripts:

```json
{
  "scripts": {
    "dev": "electron-vite dev",
    "build": "electron-vite build",
    "preview": "electron-vite preview",
    "test": "vitest run",
    "test:watch": "vitest"
  }
}
```

- [ ] **Step 12: Verify the app launches**

```bash
npm run dev
```

Expected: Electron window opens showing "NewsPub — scaffolding complete".

- [ ] **Step 13: Commit**

```bash
git add -A
git commit -m "feat: scaffold Electron + React + Vite project"
```

---

## Task 2: Document Model Types

**Files:**
- Create: `src/renderer/model/types.ts`
- Create: `src/renderer/model/page-sizes.ts`
- Test: `tests/model/document.test.ts`

- [ ] **Step 1: Write tests for page size constants**

```typescript
// tests/model/document.test.ts
import { describe, it, expect } from 'vitest'
import { PAGE_SIZES } from '@model/page-sizes'

describe('PAGE_SIZES', () => {
  it('contains US Letter at 612x792 points', () => {
    expect(PAGE_SIZES['US Letter']).toEqual({ width: 612, height: 792 })
  })

  it('contains US Legal at 612x1008 points', () => {
    expect(PAGE_SIZES['US Legal']).toEqual({ width: 612, height: 1008 })
  })

  it('contains US Tabloid at 792x1224 points', () => {
    expect(PAGE_SIZES['US Tabloid']).toEqual({ width: 792, height: 1224 })
  })

  it('contains A4 at 595x842 points', () => {
    expect(PAGE_SIZES['A4']).toEqual({ width: 595, height: 842 })
  })

  it('contains A5 at 420x595 points', () => {
    expect(PAGE_SIZES['A5']).toEqual({ width: 420, height: 595 })
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/model/document.test.ts
```

Expected: FAIL — module not found.

- [ ] **Step 3: Create page-sizes.ts**

```typescript
// src/renderer/model/page-sizes.ts
export interface PageDimensions {
  width: number   // points (1/72 inch)
  height: number  // points
}

export const PAGE_SIZES: Record<string, PageDimensions> = {
  'US Letter': { width: 612, height: 792 },
  'US Legal': { width: 612, height: 1008 },
  'US Tabloid': { width: 792, height: 1224 },
  'A4': { width: 595, height: 842 },
  'A5': { width: 420, height: 595 }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/model/document.test.ts
```

Expected: PASS

- [ ] **Step 5: Create types.ts with all document model types**

```typescript
// src/renderer/model/types.ts

// ---- Geometry ----
export interface Point {
  x: number  // points
  y: number  // points
}

export interface Size {
  width: number   // points
  height: number  // points
}

export interface Rect {
  x: number
  y: number
  width: number
  height: number
}

// ---- Styled Text ----
export interface TextStyle {
  fontFamily?: string
  fontSize?: number      // points
  bold?: boolean
  italic?: boolean
  color?: string         // hex
  listType?: 'bullet' | 'numbered' | null
  indent?: number        // points
  alignment?: 'left' | 'center' | 'right' | 'justify'
  dropCap?: boolean
  pullQuote?: boolean
}

export interface StyledRun {
  text: string
  style: TextStyle
}

// ---- Frames ----
export type WrapMode = 'skip' | 'rect'
export type ImageFit = 'fill' | 'fit' | 'stretch'

export interface TextFrame {
  type: 'text'
  id: string
  rect: Rect
  threadId: string
  threadOrder: number        // position within the thread's frame list
  styleOverrides?: TextStyle // default style for this frame
  label?: string             // template hint: "Headline", "Body Text"
}

export interface ImageFrame {
  type: 'image'
  id: string
  rect: Rect
  imageAssetId: string | null  // null = empty frame
  wrapMode: WrapMode
  imageFit: ImageFit
  label?: string               // template hint: "Image"
}

export type Frame = TextFrame | ImageFrame

// ---- Pages ----
export interface Page {
  id: string
  frames: Frame[]
  backgroundImageAssetId: string | null
}

// ---- Threads ----
export interface Thread {
  id: string
  runs: StyledRun[]
  defaultStyle: TextStyle
}

// ---- Assets ----
export interface Asset {
  id: string
  filename: string
  mimeType: 'image/jpeg' | 'image/png' | 'image/webp'
  data: ArrayBuffer     // in-memory; lazy-loaded from zip
}

// ---- Document Metadata ----
export interface DocumentMetadata {
  title: string
  author: string
  createdAt: string       // ISO 8601
  modifiedAt: string      // ISO 8601
  pageSize: Size
  unitPreference: 'inches' | 'millimeters'
}

// ---- Document (top-level) ----
export interface Document {
  metadata: DocumentMetadata
  pages: Page[]
  threads: Record<string, Thread>
  assets: Record<string, Asset>
}

// ---- Template Metadata ----
export interface TemplateMetadata {
  name: string
  description: string
  pageSize: Size
  pageCount: number
  thumbnail?: string     // base64 data URI
}

export interface TemplateSaveOptions {
  keepArticleText: boolean
  keepPlacedImages: boolean
  keepBackgroundImages: boolean  // default true
}
```

- [ ] **Step 6: Commit**

```bash
git add src/renderer/model/types.ts src/renderer/model/page-sizes.ts tests/model/document.test.ts
git commit -m "feat: define document model types and page size constants"
```

---

## Task 3: Document Model Operations

**Files:**
- Create: `src/renderer/model/document.ts`
- Create: `src/renderer/model/page.ts`
- Create: `src/renderer/model/thread.ts`
- Test: `tests/model/document.test.ts` (extend)
- Test: `tests/model/thread.test.ts`
- Test: `tests/model/page.test.ts`

- [ ] **Step 1: Write tests for document creation**

Append to `tests/model/document.test.ts`:

```typescript
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'

describe('createDocument', () => {
  it('creates a document with metadata and empty pages', () => {
    const doc = createDocument({
      title: 'Test Newsletter',
      author: 'Test Author',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 4
    })

    expect(doc.metadata.title).toBe('Test Newsletter')
    expect(doc.metadata.pageSize).toEqual({ width: 612, height: 792 })
    expect(doc.pages).toHaveLength(4)
    expect(doc.pages[0].frames).toEqual([])
    expect(Object.keys(doc.threads)).toHaveLength(0)
    expect(Object.keys(doc.assets)).toHaveLength(0)
  })

  it('assigns unique IDs to each page', () => {
    const doc = createDocument({
      title: 'Test',
      author: '',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 4
    })

    const ids = doc.pages.map(p => p.id)
    expect(new Set(ids).size).toBe(4)
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/model/document.test.ts
```

Expected: FAIL — `createDocument` not found.

- [ ] **Step 3: Implement document.ts**

```typescript
// src/renderer/model/document.ts
import type { Document, DocumentMetadata, Page, Size } from './types'

let idCounter = 0
export function generateId(prefix: string): string {
  return `${prefix}-${Date.now()}-${++idCounter}`
}

interface CreateDocumentOptions {
  title: string
  author: string
  pageSize: Size
  pageCount: number
}

export function createDocument(options: CreateDocumentOptions): Document {
  const now = new Date().toISOString()
  const pages: Page[] = Array.from({ length: options.pageCount }, () => ({
    id: generateId('page'),
    frames: [],
    backgroundImageAssetId: null
  }))

  return {
    metadata: {
      title: options.title,
      author: options.author,
      createdAt: now,
      modifiedAt: now,
      pageSize: { ...options.pageSize },
      unitPreference: 'inches'
    },
    pages,
    threads: {},
    assets: {}
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/model/document.test.ts
```

Expected: PASS

- [ ] **Step 5: Write tests for thread operations**

```typescript
// tests/model/thread.test.ts
import { describe, it, expect } from 'vitest'
import {
  createThread,
  insertText,
  deleteText,
  applyStyle,
  getPlainText,
  splitRunAtOffset,
  getRunAtOffset
} from '@model/thread'
import type { Thread, TextStyle } from '@model/types'

describe('createThread', () => {
  it('creates an empty thread with default style', () => {
    const thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    expect(thread.runs).toEqual([])
    expect(thread.defaultStyle.fontFamily).toBe('Inter')
  })
})

describe('insertText', () => {
  it('inserts text into an empty thread', () => {
    const thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    const updated = insertText(thread, 0, 'Hello world')
    expect(getPlainText(updated)).toBe('Hello world')
    expect(updated.runs).toHaveLength(1)
  })

  it('inserts text in the middle of a run', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Helloworld')
    thread = insertText(thread, 5, ' ')
    expect(getPlainText(thread)).toBe('Hello world')
  })

  it('inserts styled runs from paste', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello ')
    thread = insertText(thread, 6, 'bold', { bold: true })
    expect(getPlainText(thread)).toBe('Hello bold')
    expect(thread.runs[1].style.bold).toBe(true)
  })
})

describe('deleteText', () => {
  it('deletes a range of text within a single run', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello world')
    thread = deleteText(thread, 5, 11)
    expect(getPlainText(thread)).toBe('Hello')
  })

  it('deletes across run boundaries', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello ')
    thread = insertText(thread, 6, 'bold', { bold: true })
    thread = insertText(thread, 10, ' end')
    // "Hello bold end" → delete "lo bold e" → "Helnd"
    thread = deleteText(thread, 3, 12)
    expect(getPlainText(thread)).toBe('Helnd')
  })
})

describe('applyStyle', () => {
  it('applies bold to a range', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello world')
    thread = applyStyle(thread, 6, 11, { bold: true })
    expect(thread.runs.length).toBeGreaterThanOrEqual(2)
    // "Hello " is not bold, "world" is bold
    const worldRun = thread.runs.find(r => r.text === 'world')
    expect(worldRun?.style.bold).toBe(true)
  })
})

describe('getRunAtOffset', () => {
  it('returns the run and local offset for a global offset', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello ')
    thread = insertText(thread, 6, 'world')
    const result = getRunAtOffset(thread, 8)
    expect(result.runIndex).toBe(1)
    expect(result.localOffset).toBe(2) // "wo|rld"
  })
})
```

- [ ] **Step 6: Run test to verify it fails**

```bash
npx vitest run tests/model/thread.test.ts
```

Expected: FAIL

- [ ] **Step 7: Implement thread.ts**

```typescript
// src/renderer/model/thread.ts
import type { Thread, StyledRun, TextStyle } from './types'
import { generateId } from './document'

export function createThread(defaultStyle: TextStyle): Thread {
  return {
    id: generateId('thread'),
    runs: [],
    defaultStyle: { ...defaultStyle }
  }
}

export function getPlainText(thread: Thread): string {
  return thread.runs.map(r => r.text).join('')
}

export function getRunAtOffset(
  thread: Thread,
  offset: number
): { runIndex: number; localOffset: number } {
  let pos = 0
  for (let i = 0; i < thread.runs.length; i++) {
    const len = thread.runs[i].text.length
    if (offset <= pos + len) {
      return { runIndex: i, localOffset: offset - pos }
    }
    pos += len
  }
  return {
    runIndex: Math.max(0, thread.runs.length - 1),
    localOffset: thread.runs.length > 0
      ? thread.runs[thread.runs.length - 1].text.length
      : 0
  }
}

export function splitRunAtOffset(
  runs: StyledRun[],
  offset: number
): { before: StyledRun[]; after: StyledRun[] } {
  const before: StyledRun[] = []
  const after: StyledRun[] = []
  let pos = 0

  for (const run of runs) {
    const runEnd = pos + run.text.length
    if (runEnd <= offset) {
      before.push(run)
    } else if (pos >= offset) {
      after.push(run)
    } else {
      // Split this run
      const splitAt = offset - pos
      before.push({ text: run.text.slice(0, splitAt), style: { ...run.style } })
      after.push({ text: run.text.slice(splitAt), style: { ...run.style } })
    }
    pos = runEnd
  }

  return { before, after }
}

function mergeAdjacentRuns(runs: StyledRun[]): StyledRun[] {
  if (runs.length === 0) return []
  const result: StyledRun[] = [runs[0]]
  for (let i = 1; i < runs.length; i++) {
    const prev = result[result.length - 1]
    const curr = runs[i]
    if (stylesEqual(prev.style, curr.style)) {
      result[result.length - 1] = {
        text: prev.text + curr.text,
        style: prev.style
      }
    } else {
      result.push(curr)
    }
  }
  return result.filter(r => r.text.length > 0)
}

function stylesEqual(a: TextStyle, b: TextStyle): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

export function insertText(
  thread: Thread,
  offset: number,
  text: string,
  style?: TextStyle
): Thread {
  const resolvedStyle = style
    ? { ...thread.defaultStyle, ...style }
    : { ...thread.defaultStyle }

  const { before, after } = splitRunAtOffset(thread.runs, offset)
  const newRun: StyledRun = { text, style: resolvedStyle }
  const runs = mergeAdjacentRuns([...before, newRun, ...after])

  return { ...thread, runs }
}

export function deleteText(
  thread: Thread,
  from: number,
  to: number
): Thread {
  const { before } = splitRunAtOffset(thread.runs, from)
  const { after } = splitRunAtOffset(thread.runs, to)
  const runs = mergeAdjacentRuns([...before, ...after])
  return { ...thread, runs }
}

export function applyStyle(
  thread: Thread,
  from: number,
  to: number,
  style: Partial<TextStyle>
): Thread {
  const { before, after: rest } = splitRunAtOffset(thread.runs, from)
  const { before: middle, after } = splitRunAtOffset(rest, to - from)

  const styled = middle.map(run => ({
    text: run.text,
    style: { ...run.style, ...style }
  }))

  const runs = mergeAdjacentRuns([...before, ...styled, ...after])
  return { ...thread, runs }
}
```

- [ ] **Step 8: Run test to verify it passes**

```bash
npx vitest run tests/model/thread.test.ts
```

Expected: PASS

- [ ] **Step 9: Write tests for page operations**

```typescript
// tests/model/page.test.ts
import { describe, it, expect } from 'vitest'
import { addTextFrame, addImageFrame, removeFrame, addSpread } from '@model/page'
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'
import type { Document } from '@model/types'

describe('addTextFrame', () => {
  it('adds a text frame to a page', () => {
    const doc = createDocument({ title: 'T', author: '', pageSize: PAGE_SIZES['US Letter'], pageCount: 2 })
    const updated = addTextFrame(doc, doc.pages[0].id, {
      rect: { x: 50, y: 50, width: 200, height: 300 },
      threadId: 'thread-1',
      threadOrder: 0
    })
    expect(updated.pages[0].frames).toHaveLength(1)
    expect(updated.pages[0].frames[0].type).toBe('text')
  })
})

describe('addImageFrame', () => {
  it('adds an image frame with wrap mode', () => {
    const doc = createDocument({ title: 'T', author: '', pageSize: PAGE_SIZES['US Letter'], pageCount: 2 })
    const updated = addImageFrame(doc, doc.pages[0].id, {
      rect: { x: 100, y: 100, width: 150, height: 150 },
      wrapMode: 'rect',
      imageFit: 'fit'
    })
    expect(updated.pages[0].frames).toHaveLength(1)
    expect(updated.pages[0].frames[0].type).toBe('image')
  })
})

describe('removeFrame', () => {
  it('removes a frame by id', () => {
    let doc = createDocument({ title: 'T', author: '', pageSize: PAGE_SIZES['US Letter'], pageCount: 2 })
    doc = addTextFrame(doc, doc.pages[0].id, {
      rect: { x: 50, y: 50, width: 200, height: 300 },
      threadId: 'thread-1',
      threadOrder: 0
    })
    const frameId = doc.pages[0].frames[0].id
    const updated = removeFrame(doc, doc.pages[0].id, frameId)
    expect(updated.pages[0].frames).toHaveLength(0)
  })
})

describe('addSpread', () => {
  it('adds two pages to the document', () => {
    const doc = createDocument({ title: 'T', author: '', pageSize: PAGE_SIZES['US Letter'], pageCount: 4 })
    const updated = addSpread(doc, 4) // insert after page index 3
    expect(updated.pages).toHaveLength(6)
  })
})
```

- [ ] **Step 10: Run test to verify it fails**

```bash
npx vitest run tests/model/page.test.ts
```

Expected: FAIL

- [ ] **Step 11: Implement page.ts**

```typescript
// src/renderer/model/page.ts
import type {
  Document, Page, TextFrame, ImageFrame, Rect, WrapMode, ImageFit
} from './types'
import { generateId } from './document'

interface AddTextFrameOptions {
  rect: Rect
  threadId: string
  threadOrder: number
  label?: string
}

export function addTextFrame(
  doc: Document,
  pageId: string,
  options: AddTextFrameOptions
): Document {
  const frame: TextFrame = {
    type: 'text',
    id: generateId('frame'),
    rect: { ...options.rect },
    threadId: options.threadId,
    threadOrder: options.threadOrder,
    label: options.label
  }

  return updatePage(doc, pageId, page => ({
    ...page,
    frames: [...page.frames, frame]
  }))
}

interface AddImageFrameOptions {
  rect: Rect
  wrapMode: WrapMode
  imageFit: ImageFit
  imageAssetId?: string | null
  label?: string
}

export function addImageFrame(
  doc: Document,
  pageId: string,
  options: AddImageFrameOptions
): Document {
  const frame: ImageFrame = {
    type: 'image',
    id: generateId('frame'),
    rect: { ...options.rect },
    imageAssetId: options.imageAssetId ?? null,
    wrapMode: options.wrapMode,
    imageFit: options.imageFit,
    label: options.label
  }

  return updatePage(doc, pageId, page => ({
    ...page,
    frames: [...page.frames, frame]
  }))
}

export function removeFrame(
  doc: Document,
  pageId: string,
  frameId: string
): Document {
  return updatePage(doc, pageId, page => ({
    ...page,
    frames: page.frames.filter(f => f.id !== frameId)
  }))
}

export function addSpread(doc: Document, afterIndex: number): Document {
  const newPages: Page[] = [
    { id: generateId('page'), frames: [], backgroundImageAssetId: null },
    { id: generateId('page'), frames: [], backgroundImageAssetId: null }
  ]
  const pages = [...doc.pages]
  pages.splice(afterIndex, 0, ...newPages)
  return { ...doc, pages }
}

export function removePage(doc: Document, pageId: string): Document {
  return { ...doc, pages: doc.pages.filter(p => p.id !== pageId) }
}

function updatePage(
  doc: Document,
  pageId: string,
  updater: (page: Page) => Page
): Document {
  return {
    ...doc,
    pages: doc.pages.map(p => p.id === pageId ? updater(p) : p)
  }
}
```

- [ ] **Step 12: Run test to verify it passes**

```bash
npx vitest run tests/model/page.test.ts
```

Expected: PASS

- [ ] **Step 13: Commit**

```bash
git add src/renderer/model/ tests/model/
git commit -m "feat: implement document model operations (document, thread, page)"
```

---

## Task 4: Zustand Editor Store

**Files:**
- Create: `src/renderer/ui/store/editor-store.ts`

- [ ] **Step 1: Implement the Zustand store**

```typescript
// src/renderer/ui/store/editor-store.ts
import { create } from 'zustand'
import type { Document, Frame, TextStyle, Rect } from '@model/types'

export type ToolMode = 'select' | 'draw-text-frame' | 'draw-image-frame'

export interface Selection {
  type: 'frame'
  frameId: string
  pageId: string
} | {
  type: 'text'
  threadId: string
  anchor: number
  focus: number
} | null

export interface EditorState {
  // Document
  document: Document | null
  setDocument: (doc: Document) => void
  updateDocument: (updater: (doc: Document) => Document) => void

  // Selection
  selection: Selection
  setSelection: (sel: Selection) => void

  // Tool
  activeTool: ToolMode
  setActiveTool: (tool: ToolMode) => void

  // Viewport
  zoom: number
  setZoom: (zoom: number) => void
  panX: number
  panY: number
  setPan: (x: number, y: number) => void

  // Current page/spread
  currentPageIndex: number
  setCurrentPageIndex: (index: number) => void

  // Dirty tracking
  isDirty: boolean
  markDirty: () => void
  markClean: () => void

  // File path
  filePath: string | null
  setFilePath: (path: string | null) => void
}

export const useEditorStore = create<EditorState>((set) => ({
  document: null,
  setDocument: (doc) => set({ document: doc }),
  updateDocument: (updater) =>
    set((state) => ({
      document: state.document ? updater(state.document) : null,
      isDirty: true
    })),

  selection: null,
  setSelection: (sel) => set({ selection: sel }),

  activeTool: 'select',
  setActiveTool: (tool) => set({ activeTool: tool }),

  zoom: 1,
  setZoom: (zoom) => set({ zoom: Math.max(0.25, Math.min(4, zoom)) }),
  panX: 0,
  panY: 0,
  setPan: (x, y) => set({ panX: x, panY: y }),

  currentPageIndex: 0,
  setCurrentPageIndex: (index) => set({ currentPageIndex: index }),

  isDirty: false,
  markDirty: () => set({ isDirty: true }),
  markClean: () => set({ isDirty: false }),

  filePath: null,
  setFilePath: (path) => set({ filePath: path })
}))
```

- [ ] **Step 2: Commit**

```bash
git add src/renderer/ui/store/editor-store.ts
git commit -m "feat: add Zustand editor store for app state"
```

---

## Task 5: Layout Engine — Types and Wrap Calculator

**Files:**
- Create: `src/renderer/engine/layout-types.ts`
- Create: `src/renderer/engine/wrap-calculator.ts`
- Test: `tests/engine/wrap-calculator.test.ts`

- [ ] **Step 1: Create layout types**

```typescript
// src/renderer/engine/layout-types.ts
import type { Rect, TextStyle } from '@model/types'

export interface LayoutLine {
  text: string
  width: number
  x: number       // relative to frame
  y: number       // relative to frame
  height: number  // line height
  runStyles: Array<{
    text: string
    style: TextStyle
    x: number     // x offset within line
    width: number
  }>
}

export interface FrameLayout {
  frameId: string
  pageId: string
  lines: LayoutLine[]
  overflow: boolean
  threadCursorEnd: number  // offset in thread where this frame's text ends
  continuationTo?: { pageNumber: number }
  continuationFrom?: { pageNumber: number }
}

export interface ThreadLayout {
  threadId: string
  frameLayouts: FrameLayout[]
  totalTextLength: number
  overflow: boolean
}

export interface DocumentLayout {
  threadLayouts: Record<string, ThreadLayout>
}

export interface ExclusionZone {
  rect: Rect
  mode: 'skip' | 'rect'
}
```

- [ ] **Step 2: Write tests for wrap calculator**

```typescript
// tests/engine/wrap-calculator.test.ts
import { describe, it, expect } from 'vitest'
import {
  computeExclusionZones,
  getAvailableWidthForLine,
  getSkipRange
} from '@engine/wrap-calculator'
import type { Rect } from '@model/types'
import type { ExclusionZone } from '@engine/layout-types'

describe('computeExclusionZones', () => {
  it('returns empty array when no images overlap the frame', () => {
    const frameRect: Rect = { x: 0, y: 0, width: 300, height: 400 }
    const images: Array<{ rect: Rect; wrapMode: 'skip' | 'rect' }> = [
      { rect: { x: 400, y: 0, width: 100, height: 100 }, wrapMode: 'rect' }
    ]
    expect(computeExclusionZones(frameRect, images)).toEqual([])
  })

  it('returns exclusion zone for overlapping image', () => {
    const frameRect: Rect = { x: 0, y: 0, width: 300, height: 400 }
    const images = [
      { rect: { x: 200, y: 50, width: 150, height: 100 }, wrapMode: 'rect' as const }
    ]
    const zones = computeExclusionZones(frameRect, images)
    expect(zones).toHaveLength(1)
    expect(zones[0].mode).toBe('rect')
  })
})

describe('getAvailableWidthForLine', () => {
  it('returns full width when no exclusions overlap the line', () => {
    const width = getAvailableWidthForLine(300, 10, 20, [])
    expect(width).toBe(300)
  })

  it('reduces width for rect-mode exclusion overlapping the line', () => {
    const zones: ExclusionZone[] = [{
      rect: { x: 200, y: 0, width: 100, height: 100 },
      mode: 'rect'
    }]
    // Line at y=10, height=20. Exclusion covers y=0..100. Overlap.
    const width = getAvailableWidthForLine(300, 10, 20, zones)
    expect(width).toBe(200) // 300 - 100 = 200
  })
})

describe('getSkipRange', () => {
  it('returns null when no skip-mode images overlap', () => {
    const range = getSkipRange(300, 400, [])
    expect(range).toBeNull()
  })

  it('returns y range to skip for skip-mode exclusion', () => {
    const zones: ExclusionZone[] = [{
      rect: { x: 50, y: 100, width: 200, height: 80 },
      mode: 'skip'
    }]
    const range = getSkipRange(300, 400, zones)
    expect(range).toEqual({ yStart: 100, yEnd: 180 })
  })
})
```

- [ ] **Step 3: Run test to verify it fails**

```bash
npx vitest run tests/engine/wrap-calculator.test.ts
```

Expected: FAIL

- [ ] **Step 4: Implement wrap-calculator.ts**

```typescript
// src/renderer/engine/wrap-calculator.ts
import type { Rect } from '@model/types'
import type { ExclusionZone } from './layout-types'

function rectsOverlap(a: Rect, b: Rect): boolean {
  return !(
    a.x + a.width <= b.x ||
    b.x + b.width <= a.x ||
    a.y + a.height <= b.y ||
    b.y + b.height <= a.y
  )
}

export function computeExclusionZones(
  frameRect: Rect,
  images: Array<{ rect: Rect; wrapMode: 'skip' | 'rect' }>
): ExclusionZone[] {
  return images
    .filter(img => rectsOverlap(frameRect, img.rect))
    .map(img => {
      // Convert image rect to frame-local coordinates
      const localRect: Rect = {
        x: img.rect.x - frameRect.x,
        y: img.rect.y - frameRect.y,
        width: img.rect.width,
        height: img.rect.height
      }
      return { rect: localRect, mode: img.wrapMode }
    })
}

export function getAvailableWidthForLine(
  frameWidth: number,
  lineY: number,
  lineHeight: number,
  exclusions: ExclusionZone[]
): number {
  let width = frameWidth

  for (const zone of exclusions) {
    if (zone.mode !== 'rect') continue
    // Does this exclusion overlap the line vertically?
    const lineBottom = lineY + lineHeight
    const zoneBottom = zone.rect.y + zone.rect.height
    if (lineY < zoneBottom && lineBottom > zone.rect.y) {
      // Reduce width by the exclusion's width
      // Assume exclusion is on the right side for simplicity;
      // in practice we'd compute left/right reduction based on position
      const rightEdge = zone.rect.x + zone.rect.width
      if (rightEdge > frameWidth) {
        // Exclusion extends past right edge — reduce from right
        width = Math.min(width, zone.rect.x)
      } else if (zone.rect.x <= 0) {
        // Exclusion on left edge
        width = Math.min(width, frameWidth - rightEdge)
      } else {
        // Exclusion in middle — reduce to the larger side
        width = Math.min(width, Math.max(zone.rect.x, frameWidth - rightEdge))
      }
    }
  }

  return Math.max(0, width)
}

export function getSkipRange(
  frameWidth: number,
  frameHeight: number,
  exclusions: ExclusionZone[]
): { yStart: number; yEnd: number } | null {
  for (const zone of exclusions) {
    if (zone.mode !== 'skip') continue
    return {
      yStart: zone.rect.y,
      yEnd: zone.rect.y + zone.rect.height
    }
  }
  return null
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
npx vitest run tests/engine/wrap-calculator.test.ts
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/renderer/engine/ tests/engine/
git commit -m "feat: add layout types and image wrap/skip calculator"
```

---

## Task 6: Layout Engine — Frame Layout with pretext

**Files:**
- Create: `src/renderer/engine/frame-layout.ts`
- Create: `src/renderer/engine/layout-engine.ts`
- Test: `tests/engine/frame-layout.test.ts`
- Test: `tests/engine/layout-engine.test.ts`

- [ ] **Step 1: Write tests for frame layout**

```typescript
// tests/engine/frame-layout.test.ts
import { describe, it, expect, vi } from 'vitest'
import { layoutFrameText } from '@engine/frame-layout'
import type { StyledRun, Rect } from '@model/types'
import type { ExclusionZone } from '@engine/layout-types'

// Mock pretext since it requires a real Canvas context
vi.mock('@chenglou/pretext', () => ({
  prepareWithSegments: vi.fn(() => ({
    segments: [{ text: 'Hello world this is a test of text layout in a frame' }]
  })),
  layoutNextLine: vi.fn()
    .mockReturnValueOnce({ text: 'Hello world this is', width: 180, start: { segmentIndex: 0, graphemeIndex: 0 }, end: { segmentIndex: 0, graphemeIndex: 19 } })
    .mockReturnValueOnce({ text: 'a test of text layout', width: 190, start: { segmentIndex: 0, graphemeIndex: 19 }, end: { segmentIndex: 0, graphemeIndex: 40 } })
    .mockReturnValueOnce({ text: 'in a frame', width: 90, start: { segmentIndex: 0, graphemeIndex: 40 }, end: { segmentIndex: 0, graphemeIndex: 50 } })
    .mockReturnValueOnce(null) // signals end of text
}))

describe('layoutFrameText', () => {
  it('produces lines that fit within the frame', () => {
    const runs: StyledRun[] = [
      { text: 'Hello world this is a test of text layout in a frame', style: { fontFamily: 'Inter', fontSize: 14 } }
    ]
    const frameRect: Rect = { x: 50, y: 50, width: 200, height: 100 }
    const lineHeight = 20

    const result = layoutFrameText({
      runs,
      frameRect,
      lineHeight,
      exclusions: [],
      startOffset: 0
    })

    expect(result.lines.length).toBeGreaterThan(0)
    expect(result.lines[0].text).toBe('Hello world this is')
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/engine/frame-layout.test.ts
```

Expected: FAIL

- [ ] **Step 3: Implement frame-layout.ts**

```typescript
// src/renderer/engine/frame-layout.ts
import { prepareWithSegments, layoutNextLine } from '@chenglou/pretext'
import type { StyledRun, Rect, TextStyle } from '@model/types'
import type { LayoutLine, ExclusionZone } from './layout-types'
import { getAvailableWidthForLine, getSkipRange } from './wrap-calculator'

interface FrameLayoutInput {
  runs: StyledRun[]
  frameRect: Rect
  lineHeight: number
  exclusions: ExclusionZone[]
  startOffset: number // character offset into the thread to start from
  padding?: number
}

interface FrameLayoutResult {
  lines: LayoutLine[]
  endOffset: number    // character offset where we stopped
  overflow: boolean    // true if text remains but frame is full
}

export function layoutFrameText(input: FrameLayoutInput): FrameLayoutResult {
  const {
    runs,
    frameRect,
    lineHeight,
    exclusions,
    startOffset,
    padding = 8
  } = input

  const lines: LayoutLine[] = []
  const availableHeight = frameRect.height - padding * 2
  const baseWidth = frameRect.width - padding * 2

  // Build the full text from runs starting at startOffset
  const fullText = runs.map(r => r.text).join('')
  const textToLayout = fullText.slice(startOffset)

  if (textToLayout.length === 0) {
    return { lines: [], endOffset: startOffset, overflow: false }
  }

  // Determine font from first run (simplified — in production, handle multi-style)
  const firstStyle = runs.length > 0 ? runs[0].style : {}
  const font = `${firstStyle.fontSize ?? 14}px ${firstStyle.fontFamily ?? 'sans-serif'}`

  const prepared = prepareWithSegments(textToLayout, font)
  const skipRange = getSkipRange(baseWidth, availableHeight, exclusions)

  let y = padding
  let cursor: any = { segmentIndex: 0, graphemeIndex: 0 }
  let charOffset = startOffset

  while (y + lineHeight <= availableHeight + padding) {
    // Check skip range
    if (skipRange && y >= skipRange.yStart && y < skipRange.yEnd) {
      y = skipRange.yEnd
      continue
    }

    const lineWidth = getAvailableWidthForLine(baseWidth, y - padding, lineHeight, exclusions)

    const line = layoutNextLine(prepared, cursor, lineWidth)
    if (line === null) break

    const lineChars = line.text.length
    const runStyles = resolveRunStyles(runs, charOffset, charOffset + lineChars, padding)

    lines.push({
      text: line.text,
      width: line.width,
      x: padding,
      y,
      height: lineHeight,
      runStyles
    })

    cursor = line.end
    charOffset += lineChars
    y += lineHeight
  }

  // Check if there's more text that didn't fit
  const overflow = charOffset < fullText.length

  return { lines, endOffset: charOffset, overflow }
}

function resolveRunStyles(
  runs: StyledRun[],
  from: number,
  to: number,
  xOffset: number
): LayoutLine['runStyles'] {
  // Simplified: return one entry per run that overlaps the range
  const result: LayoutLine['runStyles'] = []
  let pos = 0
  let x = xOffset

  for (const run of runs) {
    const runStart = pos
    const runEnd = pos + run.text.length
    pos = runEnd

    if (runEnd <= from || runStart >= to) continue

    const sliceStart = Math.max(from, runStart) - runStart
    const sliceEnd = Math.min(to, runEnd) - runStart
    const text = run.text.slice(sliceStart, sliceEnd)

    result.push({
      text,
      style: run.style,
      x,
      width: 0 // will be computed by renderer using measureText
    })

    x += text.length * ((run.style.fontSize ?? 14) * 0.6) // rough estimate
  }

  return result
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/engine/frame-layout.test.ts
```

Expected: PASS

- [ ] **Step 5: Write tests for layout engine orchestration**

```typescript
// tests/engine/layout-engine.test.ts
import { describe, it, expect, vi } from 'vitest'
import { layoutThread } from '@engine/layout-engine'
import type { Document, Thread, TextFrame, Page } from '@model/types'

// Mock frame-layout module
vi.mock('@engine/frame-layout', () => ({
  layoutFrameText: vi.fn()
    .mockReturnValueOnce({
      lines: [{ text: 'First frame text', width: 100, x: 8, y: 8, height: 20, runStyles: [] }],
      endOffset: 16,
      overflow: true
    })
    .mockReturnValueOnce({
      lines: [{ text: 'Second frame text', width: 100, x: 8, y: 8, height: 20, runStyles: [] }],
      endOffset: 34,
      overflow: false
    })
}))

describe('layoutThread', () => {
  it('flows text across multiple frames', () => {
    const thread: Thread = {
      id: 'thread-1',
      runs: [{ text: 'First frame text Second frame text', style: { fontFamily: 'Inter', fontSize: 14 } }],
      defaultStyle: { fontFamily: 'Inter', fontSize: 14 }
    }

    const frames: Array<{ frame: TextFrame; pageId: string; pageIndex: number }> = [
      {
        frame: { type: 'text', id: 'f1', rect: { x: 0, y: 0, width: 200, height: 100 }, threadId: 'thread-1', threadOrder: 0 },
        pageId: 'p1',
        pageIndex: 0
      },
      {
        frame: { type: 'text', id: 'f2', rect: { x: 0, y: 0, width: 200, height: 100 }, threadId: 'thread-1', threadOrder: 1 },
        pageId: 'p2',
        pageIndex: 1
      }
    ]

    const result = layoutThread(thread, frames, [], 20)

    expect(result.frameLayouts).toHaveLength(2)
    expect(result.frameLayouts[0].lines[0].text).toBe('First frame text')
    expect(result.frameLayouts[0].continuationTo?.pageNumber).toBe(2)
    expect(result.overflow).toBe(false)
  })
})
```

- [ ] **Step 6: Run test to verify it fails**

```bash
npx vitest run tests/engine/layout-engine.test.ts
```

Expected: FAIL

- [ ] **Step 7: Implement layout-engine.ts**

```typescript
// src/renderer/engine/layout-engine.ts
import type { Document, Thread, TextFrame, ImageFrame, Page } from '@model/types'
import type {
  DocumentLayout, ThreadLayout, FrameLayout, ExclusionZone
} from './layout-types'
import { layoutFrameText } from './frame-layout'
import { computeExclusionZones } from './wrap-calculator'

const DEFAULT_LINE_HEIGHT = 20

export function layoutThread(
  thread: Thread,
  frames: Array<{ frame: TextFrame; pageId: string; pageIndex: number }>,
  imageFrames: Array<{ rect: import('@model/types').Rect; wrapMode: 'skip' | 'rect' }>,
  lineHeight: number = DEFAULT_LINE_HEIGHT
): ThreadLayout {
  const frameLayouts: FrameLayout[] = []
  let currentOffset = 0
  const totalTextLength = thread.runs.reduce((sum, r) => sum + r.text.length, 0)

  for (let i = 0; i < frames.length; i++) {
    const { frame, pageId, pageIndex } = frames[i]

    // Compute exclusion zones from overlapping images
    const exclusions = computeExclusionZones(frame.rect, imageFrames)

    const result = layoutFrameText({
      runs: thread.runs,
      frameRect: frame.rect,
      lineHeight,
      exclusions,
      startOffset: currentOffset
    })

    const frameLayout: FrameLayout = {
      frameId: frame.id,
      pageId,
      lines: result.lines,
      overflow: result.overflow && i === frames.length - 1,
      threadCursorEnd: result.endOffset
    }

    // Continuation markers
    if (result.overflow && i < frames.length - 1) {
      const nextPageIndex = frames[i + 1].pageIndex
      if (nextPageIndex !== pageIndex) {
        frameLayout.continuationTo = { pageNumber: nextPageIndex + 1 }
      }
    }
    if (i > 0) {
      const prevPageIndex = frames[i - 1].pageIndex
      if (prevPageIndex !== pageIndex) {
        frameLayout.continuationFrom = { pageNumber: prevPageIndex + 1 }
      }
    }

    frameLayouts.push(frameLayout)
    currentOffset = result.endOffset

    // If all text is consumed, stop
    if (!result.overflow) break
  }

  return {
    threadId: thread.id,
    frameLayouts,
    totalTextLength,
    overflow: currentOffset < totalTextLength
  }
}

export function layoutDocument(doc: Document): DocumentLayout {
  const threadLayouts: Record<string, ThreadLayout> = {}

  // Collect all image frames across pages for exclusion calculation
  const allImageFrames = doc.pages.flatMap(page =>
    page.frames
      .filter((f): f is ImageFrame => f.type === 'image')
      .map(f => ({ rect: f.rect, wrapMode: f.wrapMode }))
  )

  for (const [threadId, thread] of Object.entries(doc.threads)) {
    // Collect all text frames for this thread, ordered by threadOrder
    const frames: Array<{ frame: TextFrame; pageId: string; pageIndex: number }> = []

    doc.pages.forEach((page, pageIndex) => {
      page.frames
        .filter((f): f is TextFrame => f.type === 'text' && f.threadId === threadId)
        .sort((a, b) => a.threadOrder - b.threadOrder)
        .forEach(frame => {
          frames.push({ frame, pageId: page.id, pageIndex })
        })
    })

    // Sort all frames by threadOrder globally
    frames.sort((a, b) => a.frame.threadOrder - b.frame.threadOrder)

    if (frames.length > 0) {
      threadLayouts[threadId] = layoutThread(thread, frames, allImageFrames)
    }
  }

  return { threadLayouts }
}
```

- [ ] **Step 8: Run test to verify it passes**

```bash
npx vitest run tests/engine/layout-engine.test.ts
```

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/renderer/engine/ tests/engine/
git commit -m "feat: implement layout engine with pretext integration and text threading"
```

---

## Task 7: Viewport and Camera Transform

**Files:**
- Create: `src/renderer/canvas/viewport.ts`

- [ ] **Step 1: Implement viewport.ts**

```typescript
// src/renderer/canvas/viewport.ts
import type { Point, Size } from '@model/types'

export interface Camera {
  zoom: number
  panX: number    // in screen pixels
  panY: number    // in screen pixels
}

const ZOOM_MIN = 0.25
const ZOOM_MAX = 4.0
const ZOOM_STOPS = [0.25, 0.5, 0.75, 1, 1.5, 2, 3, 4]

export function clampZoom(zoom: number): number {
  return Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, zoom))
}

export function nearestZoomStop(zoom: number): number {
  return ZOOM_STOPS.reduce((prev, curr) =>
    Math.abs(curr - zoom) < Math.abs(prev - zoom) ? curr : prev
  )
}

/** Convert a document-space point (in points) to screen pixel coordinates */
export function docToScreen(doc: Point, camera: Camera): Point {
  return {
    x: doc.x * camera.zoom + camera.panX,
    y: doc.y * camera.zoom + camera.panY
  }
}

/** Convert a screen pixel coordinate to document-space point */
export function screenToDoc(screen: Point, camera: Camera): Point {
  return {
    x: (screen.x - camera.panX) / camera.zoom,
    y: (screen.y - camera.panY) / camera.zoom
  }
}

/** Get spread layout: which pages are visible and where they sit */
export interface SpreadPosition {
  pageIndex: number
  x: number     // document-space x offset
  y: number     // document-space y offset
}

export function getSpreadPositions(
  pageCount: number,
  pageSize: Size,
  gapBetweenPages: number = 20
): SpreadPosition[] {
  const positions: SpreadPosition[] = []

  if (pageCount === 0) return positions

  // Page 1: right-hand solo (offset to the right in spread)
  positions.push({ pageIndex: 0, x: pageSize.width + gapBetweenPages, y: 0 })

  // Pages 2+3, 4+5, etc: spreads
  let y = pageSize.height + gapBetweenPages * 2
  for (let i = 1; i < pageCount; i += 2) {
    // Left page
    positions.push({ pageIndex: i, x: 0, y })
    // Right page (if exists)
    if (i + 1 < pageCount) {
      positions.push({ pageIndex: i + 1, x: pageSize.width + gapBetweenPages, y })
    }
    y += pageSize.height + gapBetweenPages * 2
  }

  return positions
}

/** Apply camera transform to a canvas context */
export function applyCamera(ctx: CanvasRenderingContext2D, camera: Camera): void {
  ctx.setTransform(camera.zoom, 0, 0, camera.zoom, camera.panX, camera.panY)
}

/** Reset canvas transform */
export function resetTransform(ctx: CanvasRenderingContext2D): void {
  ctx.setTransform(1, 0, 0, 1, 0, 0)
}
```

- [ ] **Step 2: Commit**

```bash
git add src/renderer/canvas/viewport.ts
git commit -m "feat: add viewport camera transform and spread positioning"
```

---

## Task 8: Canvas Renderer

**Files:**
- Create: `src/renderer/canvas/canvas-renderer.ts`
- Create: `src/renderer/canvas/text-painter.ts`
- Create: `src/renderer/canvas/image-painter.ts`
- Create: `src/renderer/canvas/frame-chrome-painter.ts`
- Create: `src/renderer/canvas/cursor-painter.ts`

- [ ] **Step 1: Implement text-painter.ts**

```typescript
// src/renderer/canvas/text-painter.ts
import type { LayoutLine } from '@engine/layout-types'
import type { TextStyle } from '@model/types'

function buildFontString(style: TextStyle): string {
  const weight = style.bold ? 'bold' : 'normal'
  const slant = style.italic ? 'italic' : 'normal'
  const size = style.fontSize ?? 14
  const family = style.fontFamily ?? 'sans-serif'
  return `${slant} ${weight} ${size}px ${family}`
}

export function paintTextLines(
  ctx: CanvasRenderingContext2D,
  lines: LayoutLine[],
  frameX: number,
  frameY: number
): void {
  for (const line of lines) {
    if (line.runStyles.length > 0) {
      for (const runStyle of line.runStyles) {
        ctx.font = buildFontString(runStyle.style)
        ctx.fillStyle = runStyle.style.color ?? '#000000'
        ctx.fillText(runStyle.text, frameX + runStyle.x, frameY + line.y + line.height * 0.8)
      }
    } else {
      // Fallback: draw whole line
      ctx.fillStyle = '#000000'
      ctx.fillText(line.text, frameX + line.x, frameY + line.y + line.height * 0.8)
    }
  }
}

export function paintContinuationMarker(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number
): void {
  ctx.save()
  ctx.font = 'italic 10px sans-serif'
  ctx.fillStyle = '#888888'
  ctx.fillText(text, x, y)
  ctx.restore()
}

export function paintOverflowIndicator(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  size: number = 12
): void {
  ctx.save()
  ctx.fillStyle = '#ef4444'
  ctx.beginPath()
  ctx.arc(x, y, size / 2, 0, Math.PI * 2)
  ctx.fill()
  ctx.fillStyle = '#ffffff'
  ctx.font = `bold ${size - 2}px sans-serif`
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText('+', x, y)
  ctx.restore()
}
```

- [ ] **Step 2: Implement image-painter.ts**

```typescript
// src/renderer/canvas/image-painter.ts
import type { Rect, ImageFit } from '@model/types'

// Cache loaded images by asset ID
const imageCache = new Map<string, HTMLImageElement>()

export function loadImage(assetId: string, data: ArrayBuffer, mimeType: string): Promise<HTMLImageElement> {
  if (imageCache.has(assetId)) return Promise.resolve(imageCache.get(assetId)!)

  return new Promise((resolve, reject) => {
    const blob = new Blob([data], { type: mimeType })
    const url = URL.createObjectURL(blob)
    const img = new Image()
    img.onload = () => {
      imageCache.set(assetId, img)
      resolve(img)
    }
    img.onerror = reject
    img.src = url
  })
}

export function paintImage(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement,
  rect: Rect,
  fit: ImageFit
): void {
  ctx.save()

  let sx = 0, sy = 0, sw = img.width, sh = img.height
  let dx = rect.x, dy = rect.y, dw = rect.width, dh = rect.height

  if (fit === 'fit') {
    const scale = Math.min(rect.width / img.width, rect.height / img.height)
    dw = img.width * scale
    dh = img.height * scale
    dx = rect.x + (rect.width - dw) / 2
    dy = rect.y + (rect.height - dh) / 2
  } else if (fit === 'fill') {
    const scale = Math.max(rect.width / img.width, rect.height / img.height)
    sw = rect.width / scale
    sh = rect.height / scale
    sx = (img.width - sw) / 2
    sy = (img.height - sh) / 2
  }
  // 'stretch' uses rect directly

  ctx.drawImage(img, sx, sy, sw, sh, dx, dy, dw, dh)
  ctx.restore()
}

export function paintBackgroundImage(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement,
  pageWidth: number,
  pageHeight: number
): void {
  ctx.drawImage(img, 0, 0, pageWidth, pageHeight)
}

export function clearImageCache(): void {
  imageCache.clear()
}
```

- [ ] **Step 3: Implement frame-chrome-painter.ts**

```typescript
// src/renderer/canvas/frame-chrome-painter.ts
import type { Rect } from '@model/types'

const HANDLE_SIZE = 8
const TEXT_FRAME_COLOR = '#6366f1'     // purple
const IMAGE_FRAME_COLOR = '#e67e22'    // orange
const SELECTED_COLOR = '#3b82f6'       // blue

export function paintFrameBorder(
  ctx: CanvasRenderingContext2D,
  rect: Rect,
  frameType: 'text' | 'image',
  isSelected: boolean,
  isEmpty: boolean
): void {
  ctx.save()

  const color = isSelected ? SELECTED_COLOR : (frameType === 'text' ? TEXT_FRAME_COLOR : IMAGE_FRAME_COLOR)

  if (isEmpty) {
    // Dashed border for empty frames
    ctx.setLineDash([6, 4])
    ctx.strokeStyle = color
    ctx.lineWidth = 2
    ctx.strokeRect(rect.x, rect.y, rect.width, rect.height)

    // Draw placeholder icon and label
    ctx.fillStyle = color
    ctx.font = '10px sans-serif'
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    const label = frameType === 'text' ? 'Text' : 'Image'
    ctx.fillText(label, rect.x + rect.width / 2, rect.y + rect.height / 2)
  } else {
    // Solid thin border
    ctx.setLineDash([])
    ctx.strokeStyle = color
    ctx.lineWidth = isSelected ? 2 : 1
    ctx.strokeRect(rect.x, rect.y, rect.width, rect.height)
  }

  ctx.restore()
}

export function paintResizeHandles(
  ctx: CanvasRenderingContext2D,
  rect: Rect
): void {
  ctx.save()
  ctx.fillStyle = '#ffffff'
  ctx.strokeStyle = SELECTED_COLOR
  ctx.lineWidth = 1

  const corners = [
    { x: rect.x, y: rect.y },
    { x: rect.x + rect.width, y: rect.y },
    { x: rect.x, y: rect.y + rect.height },
    { x: rect.x + rect.width, y: rect.y + rect.height }
  ]

  const midpoints = [
    { x: rect.x + rect.width / 2, y: rect.y },
    { x: rect.x + rect.width, y: rect.y + rect.height / 2 },
    { x: rect.x + rect.width / 2, y: rect.y + rect.height },
    { x: rect.x, y: rect.y + rect.height / 2 }
  ]

  for (const p of [...corners, ...midpoints]) {
    ctx.fillRect(p.x - HANDLE_SIZE / 2, p.y - HANDLE_SIZE / 2, HANDLE_SIZE, HANDLE_SIZE)
    ctx.strokeRect(p.x - HANDLE_SIZE / 2, p.y - HANDLE_SIZE / 2, HANDLE_SIZE, HANDLE_SIZE)
  }

  ctx.restore()
}

export function paintSelectionHighlight(
  ctx: CanvasRenderingContext2D,
  rects: Rect[]
): void {
  ctx.save()
  ctx.fillStyle = 'rgba(59, 130, 246, 0.3)' // blue highlight
  for (const rect of rects) {
    ctx.fillRect(rect.x, rect.y, rect.width, rect.height)
  }
  ctx.restore()
}
```

- [ ] **Step 4: Implement cursor-painter.ts**

```typescript
// src/renderer/canvas/cursor-painter.ts

let blinkVisible = true
let blinkInterval: ReturnType<typeof setInterval> | null = null

export function startCursorBlink(): void {
  if (blinkInterval) return
  blinkVisible = true
  blinkInterval = setInterval(() => {
    blinkVisible = !blinkVisible
  }, 530)
}

export function stopCursorBlink(): void {
  if (blinkInterval) {
    clearInterval(blinkInterval)
    blinkInterval = null
  }
  blinkVisible = false
}

export function resetCursorBlink(): void {
  // Called on keypress to keep cursor visible while typing
  blinkVisible = true
  if (blinkInterval) {
    clearInterval(blinkInterval)
    blinkInterval = setInterval(() => {
      blinkVisible = !blinkVisible
    }, 530)
  }
}

export function paintCursor(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  height: number
): void {
  if (!blinkVisible) return
  ctx.save()
  ctx.fillStyle = '#000000'
  ctx.fillRect(x, y, 2, height)
  ctx.restore()
}
```

- [ ] **Step 5: Implement canvas-renderer.ts**

```typescript
// src/renderer/canvas/canvas-renderer.ts
import type { Document, Page, TextFrame, ImageFrame, Frame } from '@model/types'
import type { DocumentLayout, FrameLayout } from '@engine/layout-types'
import type { Camera } from './viewport'
import { applyCamera, resetTransform, getSpreadPositions } from './viewport'
import { paintTextLines, paintContinuationMarker, paintOverflowIndicator } from './text-painter'
import { paintImage, paintBackgroundImage } from './image-painter'
import { paintFrameBorder, paintResizeHandles, paintSelectionHighlight } from './frame-chrome-painter'
import { paintCursor } from './cursor-painter'
import type { Selection } from '@ui/store/editor-store'

interface RenderOptions {
  doc: Document
  layout: DocumentLayout
  camera: Camera
  canvas: HTMLCanvasElement
  selection: Selection
  editMode: boolean
  loadedImages: Map<string, HTMLImageElement>
}

export function render(options: RenderOptions): void {
  const { doc, layout, camera, canvas, selection, editMode, loadedImages } = options
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  // Clear
  resetTransform(ctx)
  ctx.fillStyle = '#e5e5e5' // gray workspace background
  ctx.fillRect(0, 0, canvas.width, canvas.height)

  // Apply camera
  applyCamera(ctx, camera)

  const spreadPositions = getSpreadPositions(
    doc.pages.length,
    doc.metadata.pageSize
  )

  // Render each page
  for (const sp of spreadPositions) {
    const page = doc.pages[sp.pageIndex]
    if (!page) continue

    ctx.save()
    ctx.translate(sp.x, sp.y)

    renderPage(ctx, doc, page, layout, {
      editMode,
      selection,
      loadedImages,
      pageWidth: doc.metadata.pageSize.width,
      pageHeight: doc.metadata.pageSize.height
    })

    ctx.restore()
  }
}

interface PageRenderOptions {
  editMode: boolean
  selection: Selection
  loadedImages: Map<string, HTMLImageElement>
  pageWidth: number
  pageHeight: number
}

function renderPage(
  ctx: CanvasRenderingContext2D,
  doc: Document,
  page: Page,
  layout: DocumentLayout,
  options: PageRenderOptions
): void {
  const { editMode, selection, loadedImages, pageWidth, pageHeight } = options

  // 1. Page background
  ctx.fillStyle = '#ffffff'
  ctx.fillRect(0, 0, pageWidth, pageHeight)

  // 2. Background/decoration image
  if (page.backgroundImageAssetId) {
    const img = loadedImages.get(page.backgroundImageAssetId)
    if (img) paintBackgroundImage(ctx, img, pageWidth, pageHeight)
  }

  // 3. Image frames
  for (const frame of page.frames) {
    if (frame.type !== 'image') continue
    const imgFrame = frame as ImageFrame

    if (imgFrame.imageAssetId) {
      const img = loadedImages.get(imgFrame.imageAssetId)
      if (img) paintImage(ctx, img, imgFrame.rect, imgFrame.imageFit)
    }

    if (editMode) {
      const isSelected = selection?.type === 'frame' && selection.frameId === frame.id
      paintFrameBorder(ctx, frame.rect, 'image', isSelected, !imgFrame.imageAssetId)
      if (isSelected) paintResizeHandles(ctx, frame.rect)
    }
  }

  // 4. Text frames
  for (const frame of page.frames) {
    if (frame.type !== 'text') continue
    const textFrame = frame as TextFrame

    // Find layout for this frame
    const threadLayout = layout.threadLayouts[textFrame.threadId]
    const frameLayout = threadLayout?.frameLayouts.find(fl => fl.frameId === frame.id)

    if (frameLayout) {
      paintTextLines(ctx, frameLayout.lines, textFrame.rect.x, textFrame.rect.y)

      // Continuation markers
      if (frameLayout.continuationTo) {
        paintContinuationMarker(
          ctx,
          `Cont. pg ${frameLayout.continuationTo.pageNumber}`,
          textFrame.rect.x + 4,
          textFrame.rect.y + textFrame.rect.height - 4
        )
      }
      if (frameLayout.continuationFrom) {
        paintContinuationMarker(
          ctx,
          `Cont. from pg ${frameLayout.continuationFrom.pageNumber}`,
          textFrame.rect.x + 4,
          textFrame.rect.y + 14
        )
      }

      // Overflow indicator
      if (frameLayout.overflow) {
        paintOverflowIndicator(
          ctx,
          textFrame.rect.x + textFrame.rect.width,
          textFrame.rect.y + textFrame.rect.height
        )
      }
    }

    if (editMode) {
      const isSelected = selection?.type === 'frame' && selection.frameId === frame.id
      const isEmpty = !frameLayout || frameLayout.lines.length === 0
      paintFrameBorder(ctx, frame.rect, 'text', isSelected, isEmpty)
      if (isSelected) paintResizeHandles(ctx, frame.rect)
    }
  }
}
```

- [ ] **Step 6: Commit**

```bash
git add src/renderer/canvas/
git commit -m "feat: implement canvas renderer with text, images, frame chrome, and cursor"
```

---

## Task 9: Input Manager — Hit Testing and Selection

**Files:**
- Create: `src/renderer/input/hit-test.ts`
- Create: `src/renderer/input/selection.ts`
- Test: `tests/input/hit-test.test.ts`
- Test: `tests/input/selection.test.ts`

- [ ] **Step 1: Write tests for hit testing**

```typescript
// tests/input/hit-test.test.ts
import { describe, it, expect } from 'vitest'
import { hitTestFrame, hitTestResizeHandle } from '@input/hit-test'
import type { Rect } from '@model/types'

describe('hitTestFrame', () => {
  it('returns true when point is inside frame', () => {
    const rect: Rect = { x: 50, y: 50, width: 200, height: 300 }
    expect(hitTestFrame({ x: 100, y: 100 }, rect)).toBe(true)
  })

  it('returns false when point is outside frame', () => {
    const rect: Rect = { x: 50, y: 50, width: 200, height: 300 }
    expect(hitTestFrame({ x: 10, y: 10 }, rect)).toBe(false)
  })
})

describe('hitTestResizeHandle', () => {
  it('identifies corner handles', () => {
    const rect: Rect = { x: 50, y: 50, width: 200, height: 300 }
    const result = hitTestResizeHandle({ x: 50, y: 50 }, rect)
    expect(result).toBe('top-left')
  })

  it('returns null when not near a handle', () => {
    const rect: Rect = { x: 50, y: 50, width: 200, height: 300 }
    const result = hitTestResizeHandle({ x: 150, y: 200 }, rect)
    expect(result).toBeNull()
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/input/hit-test.test.ts
```

Expected: FAIL

- [ ] **Step 3: Implement hit-test.ts**

```typescript
// src/renderer/input/hit-test.ts
import type { Point, Rect, Frame, Page } from '@model/types'
import type { DocumentLayout, LayoutLine } from '@engine/layout-types'
import type { SpreadPosition } from '@canvas/viewport'

const HANDLE_TOLERANCE = 6

export type HandlePosition =
  | 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right'
  | 'top' | 'right' | 'bottom' | 'left'

export function hitTestFrame(point: Point, rect: Rect): boolean {
  return (
    point.x >= rect.x &&
    point.x <= rect.x + rect.width &&
    point.y >= rect.y &&
    point.y <= rect.y + rect.height
  )
}

export function hitTestResizeHandle(
  point: Point,
  rect: Rect,
  tolerance: number = HANDLE_TOLERANCE
): HandlePosition | null {
  const corners: Array<{ pos: Point; name: HandlePosition }> = [
    { pos: { x: rect.x, y: rect.y }, name: 'top-left' },
    { pos: { x: rect.x + rect.width, y: rect.y }, name: 'top-right' },
    { pos: { x: rect.x, y: rect.y + rect.height }, name: 'bottom-left' },
    { pos: { x: rect.x + rect.width, y: rect.y + rect.height }, name: 'bottom-right' }
  ]

  const midpoints: Array<{ pos: Point; name: HandlePosition }> = [
    { pos: { x: rect.x + rect.width / 2, y: rect.y }, name: 'top' },
    { pos: { x: rect.x + rect.width, y: rect.y + rect.height / 2 }, name: 'right' },
    { pos: { x: rect.x + rect.width / 2, y: rect.y + rect.height }, name: 'bottom' },
    { pos: { x: rect.x, y: rect.y + rect.height / 2 }, name: 'left' }
  ]

  for (const handle of [...corners, ...midpoints]) {
    const dx = point.x - handle.pos.x
    const dy = point.y - handle.pos.y
    if (Math.abs(dx) <= tolerance && Math.abs(dy) <= tolerance) {
      return handle.name
    }
  }

  return null
}

export interface FrameHit {
  pageId: string
  pageIndex: number
  frame: Frame
}

/** Find which frame (if any) a document-space point hits */
export function hitTestFrames(
  docPoint: Point,
  pages: Page[],
  spreadPositions: SpreadPosition[]
): FrameHit | null {
  // Walk pages in reverse (topmost frames first)
  for (let i = spreadPositions.length - 1; i >= 0; i--) {
    const sp = spreadPositions[i]
    const page = pages[sp.pageIndex]
    if (!page) continue

    const localPoint: Point = {
      x: docPoint.x - sp.x,
      y: docPoint.y - sp.y
    }

    // Walk frames in reverse (topmost first)
    for (let j = page.frames.length - 1; j >= 0; j--) {
      const frame = page.frames[j]
      if (hitTestFrame(localPoint, frame.rect)) {
        return { pageId: page.id, pageIndex: sp.pageIndex, frame }
      }
    }
  }

  return null
}

/** Find the character offset at a point within a text frame's layout */
export function hitTestCharacter(
  localPoint: Point,
  lines: LayoutLine[],
  frameRect: Rect
): number | null {
  const relX = localPoint.x - frameRect.x
  const relY = localPoint.y - frameRect.y

  // Find the line
  let charOffset = 0
  for (const line of lines) {
    if (relY >= line.y && relY < line.y + line.height) {
      // Found the line — now find the character
      // Simple linear scan using average character width
      const avgCharWidth = line.width / Math.max(1, line.text.length)
      const charIndex = Math.min(
        Math.round((relX - line.x) / avgCharWidth),
        line.text.length
      )
      return charOffset + charIndex
    }
    charOffset += line.text.length
  }

  return null
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/input/hit-test.test.ts
```

Expected: PASS

- [ ] **Step 5: Write tests for selection**

```typescript
// tests/input/selection.test.ts
import { describe, it, expect } from 'vitest'
import {
  createTextSelection,
  isCollapsed,
  getSelectionRange,
  expandToWord,
  expandToParagraph
} from '@input/selection'

describe('TextSelection', () => {
  it('creates a collapsed selection', () => {
    const sel = createTextSelection('thread-1', 5, 5)
    expect(isCollapsed(sel)).toBe(true)
  })

  it('reports non-collapsed for range selection', () => {
    const sel = createTextSelection('thread-1', 5, 10)
    expect(isCollapsed(sel)).toBe(false)
  })

  it('normalizes range (start <= end)', () => {
    const sel = createTextSelection('thread-1', 10, 5)
    const range = getSelectionRange(sel)
    expect(range.start).toBe(5)
    expect(range.end).toBe(10)
  })
})

describe('expandToWord', () => {
  it('selects the word at offset', () => {
    const text = 'Hello world test'
    const result = expandToWord(text, 7)
    expect(result).toEqual({ start: 6, end: 11 }) // "world"
  })
})

describe('expandToParagraph', () => {
  it('selects the paragraph at offset', () => {
    const text = 'First paragraph.\nSecond paragraph.\nThird.'
    const result = expandToParagraph(text, 20)
    expect(result).toEqual({ start: 17, end: 34 }) // "Second paragraph."
  })
})
```

- [ ] **Step 6: Run test to verify it fails**

```bash
npx vitest run tests/input/selection.test.ts
```

Expected: FAIL

- [ ] **Step 7: Implement selection.ts**

```typescript
// src/renderer/input/selection.ts

export interface TextSelection {
  threadId: string
  anchor: number
  focus: number
}

export function createTextSelection(
  threadId: string,
  anchor: number,
  focus: number
): TextSelection {
  return { threadId, anchor, focus }
}

export function isCollapsed(sel: TextSelection): boolean {
  return sel.anchor === sel.focus
}

export function getSelectionRange(sel: TextSelection): { start: number; end: number } {
  return {
    start: Math.min(sel.anchor, sel.focus),
    end: Math.max(sel.anchor, sel.focus)
  }
}

export function expandToWord(
  text: string,
  offset: number
): { start: number; end: number } {
  const wordBreak = /[\s.,!?;:'"()\[\]{}<>\/\\]/

  let start = offset
  while (start > 0 && !wordBreak.test(text[start - 1])) {
    start--
  }

  let end = offset
  while (end < text.length && !wordBreak.test(text[end])) {
    end++
  }

  return { start, end }
}

export function expandToParagraph(
  text: string,
  offset: number
): { start: number; end: number } {
  let start = offset
  while (start > 0 && text[start - 1] !== '\n') {
    start--
  }

  let end = offset
  while (end < text.length && text[end] !== '\n') {
    end++
  }

  return { start, end }
}
```

- [ ] **Step 8: Run test to verify it passes**

```bash
npx vitest run tests/input/selection.test.ts
```

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/renderer/input/ tests/input/
git commit -m "feat: add hit testing and selection model"
```

---

## Task 10: Input Manager — Hidden Textarea and Clipboard

**Files:**
- Create: `src/renderer/input/input-manager.ts`
- Create: `src/renderer/input/clipboard.ts`
- Test: `tests/input/clipboard.test.ts`

- [ ] **Step 1: Write tests for clipboard parsing**

```typescript
// tests/input/clipboard.test.ts
import { describe, it, expect } from 'vitest'
import { parseHtmlToRuns, runsToHtml, runsToPlainText } from '@input/clipboard'
import type { StyledRun, TextStyle } from '@model/types'

describe('parseHtmlToRuns', () => {
  it('parses plain text', () => {
    const runs = parseHtmlToRuns('Hello world')
    expect(runs).toHaveLength(1)
    expect(runs[0].text).toBe('Hello world')
  })

  it('parses bold text', () => {
    const runs = parseHtmlToRuns('Hello <b>bold</b> world')
    expect(runs.length).toBeGreaterThanOrEqual(2)
    const boldRun = runs.find(r => r.text.trim() === 'bold')
    expect(boldRun?.style.bold).toBe(true)
  })

  it('parses italic text', () => {
    const runs = parseHtmlToRuns('Hello <i>italic</i> world')
    const italicRun = runs.find(r => r.text.trim() === 'italic')
    expect(italicRun?.style.italic).toBe(true)
  })
})

describe('runsToPlainText', () => {
  it('concatenates run text', () => {
    const runs: StyledRun[] = [
      { text: 'Hello ', style: {} },
      { text: 'world', style: { bold: true } }
    ]
    expect(runsToPlainText(runs)).toBe('Hello world')
  })
})

describe('runsToHtml', () => {
  it('wraps bold runs in <b> tags', () => {
    const runs: StyledRun[] = [
      { text: 'Hello ', style: {} },
      { text: 'bold', style: { bold: true } }
    ]
    const html = runsToHtml(runs)
    expect(html).toContain('<b>bold</b>')
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/input/clipboard.test.ts
```

Expected: FAIL

- [ ] **Step 3: Implement clipboard.ts**

```typescript
// src/renderer/input/clipboard.ts
import type { StyledRun, TextStyle } from '@model/types'

export function parseHtmlToRuns(html: string): StyledRun[] {
  // Simple parser — handles <b>, <strong>, <i>, <em>, <span style="...">
  // Uses DOMParser when available (Electron renderer has it)
  if (typeof DOMParser === 'undefined') {
    return [{ text: html.replace(/<[^>]*>/g, ''), style: {} }]
  }

  const parser = new DOMParser()
  const doc = parser.parseFromString(html, 'text/html')
  const runs: StyledRun[] = []

  function walk(node: Node, inheritedStyle: TextStyle): void {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent ?? ''
      if (text.length > 0) {
        runs.push({ text, style: { ...inheritedStyle } })
      }
      return
    }

    if (node.nodeType !== Node.ELEMENT_NODE) return
    const el = node as Element
    const style = { ...inheritedStyle }

    const tag = el.tagName.toLowerCase()
    if (tag === 'b' || tag === 'strong') style.bold = true
    if (tag === 'i' || tag === 'em') style.italic = true

    for (const child of Array.from(node.childNodes)) {
      walk(child, style)
    }
  }

  walk(doc.body, {})
  return runs.length > 0 ? runs : [{ text: html, style: {} }]
}

export function runsToPlainText(runs: StyledRun[]): string {
  return runs.map(r => r.text).join('')
}

export function runsToHtml(runs: StyledRun[]): string {
  return runs.map(run => {
    let text = escapeHtml(run.text)
    if (run.style.bold) text = `<b>${text}</b>`
    if (run.style.italic) text = `<i>${text}</i>`
    return text
  }).join('')
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/input/clipboard.test.ts
```

Expected: PASS

- [ ] **Step 5: Implement input-manager.ts**

```typescript
// src/renderer/input/input-manager.ts
import type { Point } from '@model/types'
import { parseHtmlToRuns, runsToHtml, runsToPlainText } from './clipboard'
import type { TextSelection } from './selection'
import { createTextSelection, getSelectionRange, isCollapsed } from './selection'

export interface InputManagerCallbacks {
  onTextInsert: (text: string, offset: number) => void
  onTextDelete: (from: number, to: number) => void
  onPaste: (runs: ReturnType<typeof parseHtmlToRuns>, offset: number) => void
  onSelectionChange: (selection: TextSelection | null) => void
  requestRender: () => void
}

export class InputManager {
  private textarea: HTMLTextAreaElement
  private callbacks: InputManagerCallbacks
  private currentSelection: TextSelection | null = null

  constructor(
    container: HTMLElement,
    callbacks: InputManagerCallbacks
  ) {
    this.callbacks = callbacks

    // Create hidden textarea
    this.textarea = document.createElement('textarea')
    this.textarea.style.cssText = `
      position: fixed;
      top: -9999px;
      left: -9999px;
      width: 1px;
      height: 1px;
      opacity: 0;
      pointer-events: none;
    `
    container.appendChild(this.textarea)

    this.textarea.addEventListener('input', this.handleInput)
    this.textarea.addEventListener('compositionstart', this.handleCompositionStart)
    this.textarea.addEventListener('compositionend', this.handleCompositionEnd)
    this.textarea.addEventListener('paste', this.handlePaste)
    this.textarea.addEventListener('copy', this.handleCopy)
    this.textarea.addEventListener('cut', this.handleCut)
  }

  focus(): void {
    this.textarea.focus()
  }

  setSelection(sel: TextSelection | null): void {
    this.currentSelection = sel
  }

  /** Position textarea near the cursor for IME popup placement */
  positionNearCursor(screenX: number, screenY: number): void {
    this.textarea.style.top = `${screenY}px`
    this.textarea.style.left = `${screenX}px`
  }

  destroy(): void {
    this.textarea.removeEventListener('input', this.handleInput)
    this.textarea.removeEventListener('paste', this.handlePaste)
    this.textarea.removeEventListener('copy', this.handleCopy)
    this.textarea.removeEventListener('cut', this.handleCut)
    this.textarea.remove()
  }

  private handleInput = (e: Event): void => {
    const inputEvent = e as InputEvent
    if (!this.currentSelection) return

    const text = inputEvent.data ?? ''
    if (text.length === 0) return

    const sel = this.currentSelection
    if (!isCollapsed(sel)) {
      const range = getSelectionRange(sel)
      this.callbacks.onTextDelete(range.start, range.end)
      this.callbacks.onTextInsert(text, range.start)
    } else {
      this.callbacks.onTextInsert(text, sel.anchor)
    }

    // Clear textarea for next input
    this.textarea.value = ''
    this.callbacks.requestRender()
  }

  private handleCompositionStart = (): void => {
    // IME composition started — don't process input events until end
  }

  private handleCompositionEnd = (e: CompositionEvent): void => {
    // Final composed text
    if (!this.currentSelection) return
    const text = e.data ?? ''
    if (text.length > 0) {
      this.callbacks.onTextInsert(text, this.currentSelection.anchor)
      this.textarea.value = ''
      this.callbacks.requestRender()
    }
  }

  private handlePaste = (e: ClipboardEvent): void => {
    e.preventDefault()
    if (!this.currentSelection) return

    const html = e.clipboardData?.getData('text/html')
    const plain = e.clipboardData?.getData('text/plain')

    if (html) {
      const runs = parseHtmlToRuns(html)
      this.callbacks.onPaste(runs, this.currentSelection.anchor)
    } else if (plain) {
      this.callbacks.onTextInsert(plain, this.currentSelection.anchor)
    }

    this.callbacks.requestRender()
  }

  private handleCopy = (e: ClipboardEvent): void => {
    // Copy is handled by the store — this is a hook point
    e.preventDefault()
  }

  private handleCut = (e: ClipboardEvent): void => {
    e.preventDefault()
    if (!this.currentSelection || isCollapsed(this.currentSelection)) return
    const range = getSelectionRange(this.currentSelection)
    this.callbacks.onTextDelete(range.start, range.end)
    this.callbacks.requestRender()
  }
}
```

- [ ] **Step 6: Commit**

```bash
git add src/renderer/input/ tests/input/
git commit -m "feat: add input manager with hidden textarea and clipboard handling"
```

---

## Task 11: History Manager (Undo/Redo)

**Files:**
- Create: `src/renderer/history/commands.ts`
- Create: `src/renderer/history/history-manager.ts`
- Test: `tests/history/history-manager.test.ts`

- [ ] **Step 1: Write tests for history manager**

```typescript
// tests/history/history-manager.test.ts
import { describe, it, expect, beforeEach } from 'vitest'
import { HistoryManager } from '@history/history-manager'
import type { Command } from '@history/commands'

describe('HistoryManager', () => {
  let history: HistoryManager
  let value: number

  beforeEach(() => {
    history = new HistoryManager()
    value = 0
  })

  function makeCommand(delta: number, groupId?: string): Command {
    return {
      type: 'test',
      apply: () => { value += delta },
      reverse: () => { value -= delta },
      groupId,
      timestamp: Date.now()
    }
  }

  it('undoes a single command', () => {
    const cmd = makeCommand(10)
    cmd.apply()
    history.push(cmd)
    expect(value).toBe(10)

    history.undo()
    expect(value).toBe(0)
  })

  it('redoes after undo', () => {
    const cmd = makeCommand(10)
    cmd.apply()
    history.push(cmd)
    history.undo()
    history.redo()
    expect(value).toBe(10)
  })

  it('clears redo stack on new command', () => {
    const cmd1 = makeCommand(10)
    cmd1.apply()
    history.push(cmd1)

    history.undo()
    expect(value).toBe(0)

    const cmd2 = makeCommand(5)
    cmd2.apply()
    history.push(cmd2)

    history.redo() // should do nothing
    expect(value).toBe(5)
  })

  it('undoes grouped commands as one step', () => {
    const group = 'drag-1'
    for (let i = 0; i < 5; i++) {
      const cmd = makeCommand(1, group)
      cmd.apply()
      history.push(cmd)
    }
    expect(value).toBe(5)

    history.undo() // should undo all 5
    expect(value).toBe(0)
  })

  it('treats different groups as separate undo steps', () => {
    const cmd1 = makeCommand(10, 'group-a')
    cmd1.apply()
    history.push(cmd1)

    const cmd2 = makeCommand(20, 'group-b')
    cmd2.apply()
    history.push(cmd2)

    expect(value).toBe(30)
    history.undo() // undo group-b
    expect(value).toBe(10)
    history.undo() // undo group-a
    expect(value).toBe(0)
  })

  it('reports canUndo and canRedo', () => {
    expect(history.canUndo()).toBe(false)
    expect(history.canRedo()).toBe(false)

    const cmd = makeCommand(10)
    cmd.apply()
    history.push(cmd)

    expect(history.canUndo()).toBe(true)
    expect(history.canRedo()).toBe(false)

    history.undo()
    expect(history.canUndo()).toBe(false)
    expect(history.canRedo()).toBe(true)
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/history/history-manager.test.ts
```

Expected: FAIL

- [ ] **Step 3: Implement commands.ts**

```typescript
// src/renderer/history/commands.ts

export interface Command {
  type: string
  apply: () => void
  reverse: () => void
  groupId?: string
  timestamp: number
}
```

- [ ] **Step 4: Implement history-manager.ts**

```typescript
// src/renderer/history/history-manager.ts
import type { Command } from './commands'

export class HistoryManager {
  private undoStack: Command[] = []
  private redoStack: Command[] = []

  push(command: Command): void {
    this.undoStack.push(command)
    this.redoStack = [] // new action clears redo
  }

  undo(): void {
    if (!this.canUndo()) return

    const last = this.undoStack[this.undoStack.length - 1]
    const groupId = last.groupId

    if (groupId) {
      // Pop all commands in this group
      const group: Command[] = []
      while (
        this.undoStack.length > 0 &&
        this.undoStack[this.undoStack.length - 1].groupId === groupId
      ) {
        group.push(this.undoStack.pop()!)
      }
      // Reverse in order (newest first)
      for (const cmd of group) {
        cmd.reverse()
      }
      this.redoStack.push(...group.reverse())
    } else {
      const cmd = this.undoStack.pop()!
      cmd.reverse()
      this.redoStack.push(cmd)
    }
  }

  redo(): void {
    if (!this.canRedo()) return

    const next = this.redoStack[this.redoStack.length - 1]
    const groupId = next.groupId

    if (groupId) {
      const group: Command[] = []
      while (
        this.redoStack.length > 0 &&
        this.redoStack[this.redoStack.length - 1].groupId === groupId
      ) {
        group.push(this.redoStack.pop()!)
      }
      for (const cmd of group) {
        cmd.apply()
      }
      this.undoStack.push(...group.reverse())
    } else {
      const cmd = this.redoStack.pop()!
      cmd.apply()
      this.undoStack.push(cmd)
    }
  }

  canUndo(): boolean {
    return this.undoStack.length > 0
  }

  canRedo(): boolean {
    return this.redoStack.length > 0
  }

  clear(): void {
    this.undoStack = []
    this.redoStack = []
  }
}
```

- [ ] **Step 5: Run test to verify it passes**

```bash
npx vitest run tests/history/history-manager.test.ts
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/renderer/history/ tests/history/
git commit -m "feat: implement undo/redo history manager with command grouping"
```

---

## Task 12: File Manager (.newspub format)

**Files:**
- Create: `src/renderer/file/serializer.ts`
- Create: `src/renderer/file/file-manager.ts`
- Create: `src/renderer/file/autosave.ts`
- Test: `tests/file/serializer.test.ts`
- Test: `tests/file/file-manager.test.ts`

- [ ] **Step 1: Write tests for serializer**

```typescript
// tests/file/serializer.test.ts
import { describe, it, expect } from 'vitest'
import { serializeDocument, deserializeDocument } from '@file/serializer'
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'

describe('serializer', () => {
  it('round-trips a document through JSON', () => {
    const doc = createDocument({
      title: 'Test Newsletter',
      author: 'Author',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 4
    })

    const json = serializeDocument(doc)
    const parsed = deserializeDocument(json)

    expect(parsed.metadata.title).toBe('Test Newsletter')
    expect(parsed.pages).toHaveLength(4)
    expect(parsed.metadata.pageSize).toEqual({ width: 612, height: 792 })
  })

  it('preserves thread data', () => {
    const doc = createDocument({
      title: 'Test',
      author: '',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 2
    })
    doc.threads['thread-1'] = {
      id: 'thread-1',
      runs: [{ text: 'Hello world', style: { bold: true, fontFamily: 'Inter', fontSize: 14 } }],
      defaultStyle: { fontFamily: 'Inter', fontSize: 14 }
    }

    const json = serializeDocument(doc)
    const parsed = deserializeDocument(json)

    expect(parsed.threads['thread-1'].runs[0].text).toBe('Hello world')
    expect(parsed.threads['thread-1'].runs[0].style.bold).toBe(true)
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/file/serializer.test.ts
```

Expected: FAIL

- [ ] **Step 3: Implement serializer.ts**

```typescript
// src/renderer/file/serializer.ts
import type { Document } from '@model/types'

interface SerializedDocument {
  metadata: Document['metadata']
  pages: Array<{
    id: string
    frames: Document['pages'][number]['frames']
    backgroundImageAssetId: string | null
  }>
  threads: Record<string, {
    id: string
    runs: Array<{ text: string; style: Record<string, unknown> }>
    defaultStyle: Record<string, unknown>
  }>
  // Assets are stored separately in the zip, not in JSON
  assetManifest: Array<{
    id: string
    filename: string
    mimeType: string
  }>
}

export function serializeDocument(doc: Document): string {
  const serialized: SerializedDocument = {
    metadata: doc.metadata,
    pages: doc.pages.map(p => ({
      id: p.id,
      frames: p.frames,
      backgroundImageAssetId: p.backgroundImageAssetId
    })),
    threads: Object.fromEntries(
      Object.entries(doc.threads).map(([id, thread]) => [
        id,
        {
          id: thread.id,
          runs: thread.runs.map(r => ({
            text: r.text,
            style: r.style as Record<string, unknown>
          })),
          defaultStyle: thread.defaultStyle as Record<string, unknown>
        }
      ])
    ),
    assetManifest: Object.values(doc.assets).map(a => ({
      id: a.id,
      filename: a.filename,
      mimeType: a.mimeType
    }))
  }

  return JSON.stringify(serialized, null, 2)
}

export function deserializeDocument(json: string): Document {
  const data: SerializedDocument = JSON.parse(json)

  return {
    metadata: data.metadata,
    pages: data.pages.map(p => ({
      id: p.id,
      frames: p.frames,
      backgroundImageAssetId: p.backgroundImageAssetId
    })),
    threads: Object.fromEntries(
      Object.entries(data.threads).map(([id, thread]) => [
        id,
        {
          id: thread.id,
          runs: thread.runs.map(r => ({
            text: r.text,
            style: r.style
          })),
          defaultStyle: thread.defaultStyle
        }
      ])
    ) as Document['threads'],
    assets: {} // Assets loaded separately from zip
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/file/serializer.test.ts
```

Expected: PASS

- [ ] **Step 5: Write tests for file manager**

```typescript
// tests/file/file-manager.test.ts
import { describe, it, expect } from 'vitest'
import { packNewspub, unpackNewspub } from '@file/file-manager'
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'

describe('file-manager', () => {
  it('round-trips a document through .newspub zip format', async () => {
    const doc = createDocument({
      title: 'Zip Test',
      author: 'Test',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 4
    })
    doc.threads['t1'] = {
      id: 't1',
      runs: [{ text: 'Article text', style: { fontFamily: 'Inter', fontSize: 14 } }],
      defaultStyle: { fontFamily: 'Inter', fontSize: 14 }
    }

    const zipBuffer = await packNewspub(doc)
    expect(zipBuffer.byteLength).toBeGreaterThan(0)

    const restored = await unpackNewspub(zipBuffer)
    expect(restored.metadata.title).toBe('Zip Test')
    expect(restored.pages).toHaveLength(4)
    expect(restored.threads['t1'].runs[0].text).toBe('Article text')
  })

  it('includes assets in the zip', async () => {
    const doc = createDocument({
      title: 'Asset Test',
      author: '',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 1
    })
    // Add a fake image asset
    const fakeImage = new Uint8Array([0xFF, 0xD8, 0xFF, 0xE0]).buffer
    doc.assets['img-1'] = {
      id: 'img-1',
      filename: 'photo.jpg',
      mimeType: 'image/jpeg',
      data: fakeImage
    }

    const zipBuffer = await packNewspub(doc)
    const restored = await unpackNewspub(zipBuffer)

    expect(restored.assets['img-1']).toBeDefined()
    expect(restored.assets['img-1'].filename).toBe('photo.jpg')
    expect(restored.assets['img-1'].data.byteLength).toBe(4)
  })
})
```

- [ ] **Step 6: Run test to verify it fails**

```bash
npx vitest run tests/file/file-manager.test.ts
```

Expected: FAIL

- [ ] **Step 7: Implement file-manager.ts**

```typescript
// src/renderer/file/file-manager.ts
import { zipSync, unzipSync, strToU8, strFromU8 } from 'fflate'
import type { Document, Asset } from '@model/types'
import { serializeDocument, deserializeDocument } from './serializer'

export async function packNewspub(doc: Document): Promise<ArrayBuffer> {
  const files: Record<string, Uint8Array> = {}

  // document.json
  const docJson = serializeDocument(doc)
  files['document.json'] = strToU8(docJson)

  // metadata.json
  const metadata = {
    title: doc.metadata.title,
    author: doc.metadata.author,
    createdAt: doc.metadata.createdAt,
    modifiedAt: new Date().toISOString(),
    pageSize: doc.metadata.pageSize,
    version: '1.0.0'
  }
  files['metadata.json'] = strToU8(JSON.stringify(metadata, null, 2))

  // Assets
  for (const [id, asset] of Object.entries(doc.assets)) {
    const path = `assets/${asset.filename}`
    files[path] = new Uint8Array(asset.data)
  }

  // Zip with appropriate compression
  const zipOptions: Record<string, { level: number }> = {}
  for (const key of Object.keys(files)) {
    if (key.endsWith('.json')) {
      zipOptions[key] = { level: 6 } // compress JSON
    } else {
      zipOptions[key] = { level: 0 } // store images as-is (already compressed)
    }
  }

  const zipped = zipSync(files, { level: 0 })
  return zipped.buffer
}

export async function unpackNewspub(buffer: ArrayBuffer): Promise<Document> {
  const data = new Uint8Array(buffer)
  const unzipped = unzipSync(data)

  // Parse document.json
  const docJson = strFromU8(unzipped['document.json'])
  const doc = deserializeDocument(docJson)

  // Load assets
  for (const [path, fileData] of Object.entries(unzipped)) {
    if (!path.startsWith('assets/')) continue
    const filename = path.slice('assets/'.length)

    // Find matching asset in manifest
    const assetEntry = Object.values(doc.assets).find(a => a.filename === filename)

    // Determine mime type from extension
    const ext = filename.split('.').pop()?.toLowerCase()
    const mimeMap: Record<string, Asset['mimeType']> = {
      jpg: 'image/jpeg',
      jpeg: 'image/jpeg',
      png: 'image/png',
      webp: 'image/webp'
    }
    const mimeType = mimeMap[ext ?? ''] ?? 'image/jpeg'

    // Find asset ID from the serialized manifest
    const manifestJson = docJson
    const manifest = JSON.parse(manifestJson)
    const manifestEntry = manifest.assetManifest?.find(
      (a: { filename: string }) => a.filename === filename
    )

    if (manifestEntry) {
      doc.assets[manifestEntry.id] = {
        id: manifestEntry.id,
        filename,
        mimeType,
        data: fileData.buffer
      }
    }
  }

  return doc
}
```

- [ ] **Step 8: Run test to verify it passes**

```bash
npx vitest run tests/file/file-manager.test.ts
```

Expected: PASS

- [ ] **Step 9: Implement autosave.ts**

```typescript
// src/renderer/file/autosave.ts
import type { Document } from '@model/types'
import { packNewspub } from './file-manager'

const AUTOSAVE_INTERVAL_MS = 60_000 // 60 seconds

export class AutosaveManager {
  private timer: ReturnType<typeof setInterval> | null = null
  private isDirty = false
  private getDocument: () => Document | null
  private saveFn: (data: ArrayBuffer, path: string) => Promise<void>
  private getFilePath: () => string | null

  constructor(
    getDocument: () => Document | null,
    getFilePath: () => string | null,
    saveFn: (data: ArrayBuffer, path: string) => Promise<void>
  ) {
    this.getDocument = getDocument
    this.getFilePath = getFilePath
    this.saveFn = saveFn
  }

  start(): void {
    this.stop()
    this.timer = setInterval(() => this.tick(), AUTOSAVE_INTERVAL_MS)
  }

  stop(): void {
    if (this.timer) {
      clearInterval(this.timer)
      this.timer = null
    }
  }

  markDirty(): void {
    this.isDirty = true
  }

  markClean(): void {
    this.isDirty = false
  }

  private async tick(): Promise<void> {
    if (!this.isDirty) return
    const doc = this.getDocument()
    const filePath = this.getFilePath()
    if (!doc || !filePath) return

    try {
      const buffer = await packNewspub(doc)
      await this.saveFn(buffer, filePath + '.autosave')
      this.isDirty = false
    } catch (err) {
      console.error('Autosave failed:', err)
    }
  }
}
```

- [ ] **Step 10: Commit**

```bash
git add src/renderer/file/ tests/file/
git commit -m "feat: implement .newspub zip file manager with autosave"
```

---

## Task 13: Template Manager

**Files:**
- Create: `src/renderer/template/template-manager.ts`
- Create: `src/renderer/template/preset-templates.ts`
- Test: `tests/template/template-manager.test.ts`

- [ ] **Step 1: Write tests for template manager**

```typescript
// tests/template/template-manager.test.ts
import { describe, it, expect } from 'vitest'
import { stripToTemplate } from '@template/template-manager'
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'
import { addTextFrame, addImageFrame } from '@model/page'
import type { Document, TemplateSaveOptions } from '@model/types'

function makeTestDoc(): Document {
  let doc = createDocument({
    title: 'Test Newsletter',
    author: 'Author',
    pageSize: PAGE_SIZES['US Letter'],
    pageCount: 4
  })

  // Add a thread with content
  doc.threads['t1'] = {
    id: 't1',
    runs: [{ text: 'Article body', style: { fontFamily: 'Inter', fontSize: 14 } }],
    defaultStyle: { fontFamily: 'Inter', fontSize: 14 }
  }

  // Add frames
  doc = addTextFrame(doc, doc.pages[0].id, {
    rect: { x: 50, y: 50, width: 200, height: 300 },
    threadId: 't1',
    threadOrder: 0,
    label: 'Body Text'
  })

  doc = addImageFrame(doc, doc.pages[0].id, {
    rect: { x: 300, y: 50, width: 150, height: 150 },
    wrapMode: 'rect',
    imageFit: 'fit',
    imageAssetId: 'img-1'
  })

  // Add an asset
  doc.assets['img-1'] = {
    id: 'img-1',
    filename: 'photo.jpg',
    mimeType: 'image/jpeg',
    data: new ArrayBuffer(10)
  }

  // Add background image
  doc.pages[0].backgroundImageAssetId = 'bg-1'
  doc.assets['bg-1'] = {
    id: 'bg-1',
    filename: 'border.png',
    mimeType: 'image/png',
    data: new ArrayBuffer(20)
  }

  return doc
}

describe('stripToTemplate', () => {
  it('strips text when keepArticleText is false', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: false,
      keepPlacedImages: true,
      keepBackgroundImages: true
    })
    expect(template.threads['t1'].runs).toEqual([])
  })

  it('keeps text when keepArticleText is true', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: true,
      keepPlacedImages: true,
      keepBackgroundImages: true
    })
    expect(template.threads['t1'].runs[0].text).toBe('Article body')
  })

  it('strips placed images when keepPlacedImages is false', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: false,
      keepPlacedImages: false,
      keepBackgroundImages: true
    })
    const imgFrame = template.pages[0].frames.find(f => f.type === 'image')
    expect(imgFrame).toBeDefined()
    expect((imgFrame as any).imageAssetId).toBeNull()
    expect(template.assets['img-1']).toBeUndefined()
  })

  it('keeps background images when keepBackgroundImages is true', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: false,
      keepPlacedImages: false,
      keepBackgroundImages: true
    })
    expect(template.pages[0].backgroundImageAssetId).toBe('bg-1')
    expect(template.assets['bg-1']).toBeDefined()
  })

  it('strips background images when keepBackgroundImages is false', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: false,
      keepPlacedImages: false,
      keepBackgroundImages: false
    })
    expect(template.pages[0].backgroundImageAssetId).toBeNull()
    expect(template.assets['bg-1']).toBeUndefined()
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/template/template-manager.test.ts
```

Expected: FAIL

- [ ] **Step 3: Implement template-manager.ts**

```typescript
// src/renderer/template/template-manager.ts
import type { Document, TemplateSaveOptions, ImageFrame, TemplateMetadata } from '@model/types'

export function stripToTemplate(
  doc: Document,
  options: TemplateSaveOptions
): Document {
  const template = structuredClone(doc)
  const usedAssetIds = new Set<string>()

  // Strip or keep thread content
  for (const thread of Object.values(template.threads)) {
    if (!options.keepArticleText) {
      thread.runs = []
    }
  }

  // Strip or keep placed images
  for (const page of template.pages) {
    for (let i = 0; i < page.frames.length; i++) {
      const frame = page.frames[i]
      if (frame.type === 'image') {
        const imgFrame = frame as ImageFrame
        if (options.keepPlacedImages && imgFrame.imageAssetId) {
          usedAssetIds.add(imgFrame.imageAssetId)
        } else {
          imgFrame.imageAssetId = null
        }
      }
    }

    // Background images
    if (options.keepBackgroundImages && page.backgroundImageAssetId) {
      usedAssetIds.add(page.backgroundImageAssetId)
    } else {
      page.backgroundImageAssetId = null
    }
  }

  // Remove unused assets
  const newAssets: typeof template.assets = {}
  for (const [id, asset] of Object.entries(template.assets)) {
    if (usedAssetIds.has(id)) {
      newAssets[id] = asset
    }
  }
  template.assets = newAssets

  return template
}

export function getTemplateMetadata(doc: Document, name: string, description: string): TemplateMetadata {
  return {
    name,
    description,
    pageSize: { ...doc.metadata.pageSize },
    pageCount: doc.pages.length
  }
}

export function applyTemplate(template: Document): Document {
  // Create a fresh document from the template
  const doc = structuredClone(template)
  const now = new Date().toISOString()
  doc.metadata.createdAt = now
  doc.metadata.modifiedAt = now
  return doc
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/template/template-manager.test.ts
```

Expected: PASS

- [ ] **Step 5: Implement preset-templates.ts**

```typescript
// src/renderer/template/preset-templates.ts
import type { Document, TextFrame, Page } from '@model/types'
import { createDocument, generateId } from '@model/document'
import { createThread } from '@model/thread'
import { PAGE_SIZES } from '@model/page-sizes'
import type { TemplateMetadata } from '@model/types'

export interface PresetTemplate {
  metadata: TemplateMetadata
  build: () => Document
}

export const presetTemplates: PresetTemplate[] = [
  {
    metadata: {
      name: 'Classic 4-Page Newsletter',
      description: 'Front page hero, 2-3 spread with 3 columns, back page contacts',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 4
    },
    build: () => buildClassic4Page()
  },
  {
    metadata: {
      name: 'Simple 2-Page',
      description: 'Single front and back with 2 columns',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 2
    },
    build: () => buildSimple2Page()
  }
]

function buildClassic4Page(): Document {
  const doc = createDocument({
    title: 'Untitled Newsletter',
    author: '',
    pageSize: PAGE_SIZES['US Letter'],
    pageCount: 4
  })

  const margin = 36 // 0.5 inch
  const pw = 612
  const ph = 792
  const contentW = pw - margin * 2
  const colW = (contentW - 12) / 2 // 2 columns with 12pt gutter

  // Page 1: headline + 2-column intro
  const headlineThread = createThread({ fontFamily: 'Georgia', fontSize: 28, bold: true })
  doc.threads[headlineThread.id] = headlineThread

  const bodyThread = createThread({ fontFamily: 'Inter', fontSize: 11 })
  doc.threads[bodyThread.id] = bodyThread

  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: contentW, height: 60 },
    threadId: headlineThread.id, threadOrder: 0, label: 'Headline'
  })
  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin + 72, width: colW, height: ph - margin * 2 - 72 },
    threadId: bodyThread.id, threadOrder: 0, label: 'Body Text'
  })
  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin + colW + 12, y: margin + 72, width: colW, height: ph - margin * 2 - 72 },
    threadId: bodyThread.id, threadOrder: 1, label: 'Body Text'
  })

  // Pages 2-3: continued articles
  doc.pages[1].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: colW, height: ph - margin * 2 },
    threadId: bodyThread.id, threadOrder: 2, label: 'Body Text'
  })
  doc.pages[1].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin + colW + 12, y: margin, width: colW, height: ph - margin * 2 },
    threadId: bodyThread.id, threadOrder: 3, label: 'Body Text'
  })

  // Page 2 right side
  doc.pages[2].frames.push({
    type: 'image', id: generateId('frame'),
    rect: { x: margin, y: margin, width: contentW, height: 200 },
    imageAssetId: null, wrapMode: 'skip', imageFit: 'fill', label: 'Image'
  })
  doc.pages[2].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin + 212, width: contentW, height: ph - margin * 2 - 212 },
    threadId: bodyThread.id, threadOrder: 4, label: 'Body Text'
  })

  // Page 4: back page
  const contactThread = createThread({ fontFamily: 'Inter', fontSize: 10 })
  doc.threads[contactThread.id] = contactThread

  doc.pages[3].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: contentW, height: ph - margin * 2 },
    threadId: contactThread.id, threadOrder: 0, label: 'Contact Info'
  })

  return doc
}

function buildSimple2Page(): Document {
  const doc = createDocument({
    title: 'Untitled Newsletter',
    author: '',
    pageSize: PAGE_SIZES['US Letter'],
    pageCount: 2
  })

  const margin = 36
  const contentW = 612 - margin * 2
  const colW = (contentW - 12) / 2

  const thread = createThread({ fontFamily: 'Inter', fontSize: 11 })
  doc.threads[thread.id] = thread

  // Page 1
  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: colW, height: 720 },
    threadId: thread.id, threadOrder: 0, label: 'Body Text'
  })
  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin + colW + 12, y: margin, width: colW, height: 720 },
    threadId: thread.id, threadOrder: 1, label: 'Body Text'
  })

  // Page 2
  doc.pages[1].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: colW, height: 720 },
    threadId: thread.id, threadOrder: 2, label: 'Body Text'
  })
  doc.pages[1].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin + colW + 12, y: margin, width: colW, height: 720 },
    threadId: thread.id, threadOrder: 3, label: 'Body Text'
  })

  return doc
}
```

- [ ] **Step 6: Commit**

```bash
git add src/renderer/template/ tests/template/
git commit -m "feat: implement template manager with strip/apply and preset templates"
```

---

## Task 14: PDF Exporter

**Files:**
- Create: `src/renderer/export/font-resolver.ts`
- Create: `src/renderer/export/pdf-exporter.ts`
- Test: `tests/export/font-resolver.test.ts`
- Test: `tests/export/pdf-exporter.test.ts`

- [ ] **Step 1: Write tests for font resolver**

```typescript
// tests/export/font-resolver.test.ts
import { describe, it, expect, vi } from 'vitest'
import { getStandardFontName, isStandardFont } from '@export/font-resolver'

describe('font-resolver', () => {
  it('maps Helvetica to a standard PDF font', () => {
    expect(isStandardFont('Helvetica')).toBe(true)
    expect(getStandardFontName('Helvetica')).toBe('Helvetica')
  })

  it('maps Times New Roman to Times-Roman', () => {
    expect(isStandardFont('Times New Roman')).toBe(true)
    expect(getStandardFontName('Times New Roman')).toBe('Times-Roman')
  })

  it('reports non-standard fonts', () => {
    expect(isStandardFont('Inter')).toBe(false)
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

```bash
npx vitest run tests/export/font-resolver.test.ts
```

Expected: FAIL

- [ ] **Step 3: Implement font-resolver.ts**

```typescript
// src/renderer/export/font-resolver.ts
import { StandardFonts } from 'pdf-lib'

const STANDARD_FONT_MAP: Record<string, string> = {
  'Helvetica': StandardFonts.Helvetica,
  'Arial': StandardFonts.Helvetica,
  'sans-serif': StandardFonts.Helvetica,
  'Times New Roman': StandardFonts.TimesRoman,
  'Times': StandardFonts.TimesRoman,
  'Georgia': StandardFonts.TimesRoman,
  'serif': StandardFonts.TimesRoman,
  'Courier New': StandardFonts.Courier,
  'Courier': StandardFonts.Courier,
  'monospace': StandardFonts.Courier
}

const BOLD_MAP: Record<string, string> = {
  [StandardFonts.Helvetica]: StandardFonts.HelveticaBold,
  [StandardFonts.TimesRoman]: StandardFonts.TimesRomanBold,
  [StandardFonts.Courier]: StandardFonts.CourierBold
}

const ITALIC_MAP: Record<string, string> = {
  [StandardFonts.Helvetica]: StandardFonts.HelveticaOblique,
  [StandardFonts.TimesRoman]: StandardFonts.TimesRomanItalic,
  [StandardFonts.Courier]: StandardFonts.CourierOblique
}

const BOLD_ITALIC_MAP: Record<string, string> = {
  [StandardFonts.Helvetica]: StandardFonts.HelveticaBoldOblique,
  [StandardFonts.TimesRoman]: StandardFonts.TimesRomanBoldItalic,
  [StandardFonts.Courier]: StandardFonts.CourierBoldOblique
}

export function isStandardFont(family: string): boolean {
  return family in STANDARD_FONT_MAP
}

export function getStandardFontName(family: string): string {
  return STANDARD_FONT_MAP[family] ?? StandardFonts.Helvetica
}

export function resolveStandardFont(
  family: string,
  bold: boolean,
  italic: boolean
): string {
  const base = getStandardFontName(family)
  if (bold && italic) return BOLD_ITALIC_MAP[base] ?? base
  if (bold) return BOLD_MAP[base] ?? base
  if (italic) return ITALIC_MAP[base] ?? base
  return base
}

export interface FontResolutionResult {
  fontName: string
  isEmbedded: boolean
  warnings: string[]
}

export function resolveFont(
  family: string,
  bold: boolean,
  italic: boolean
): FontResolutionResult {
  if (isStandardFont(family)) {
    return {
      fontName: resolveStandardFont(family, bold, italic),
      isEmbedded: false,
      warnings: []
    }
  }

  // For v1: fall back to standard fonts with a warning
  // Future: resolve system font files and embed them
  const fallback = resolveStandardFont('Helvetica', bold, italic)
  return {
    fontName: fallback,
    isEmbedded: false,
    warnings: [`Font "${family}" is not available for PDF embedding. Falling back to Helvetica.`]
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
npx vitest run tests/export/font-resolver.test.ts
```

Expected: PASS

- [ ] **Step 5: Write tests for PDF exporter**

```typescript
// tests/export/pdf-exporter.test.ts
import { describe, it, expect } from 'vitest'
import { exportToPdf } from '@export/pdf-exporter'
import { createDocument } from '@model/document'
import { createThread, insertText } from '@model/thread'
import { addTextFrame } from '@model/page'
import { PAGE_SIZES } from '@model/page-sizes'
import type { DocumentLayout } from '@engine/layout-types'

describe('exportToPdf', () => {
  it('produces a valid PDF buffer', async () => {
    let doc = createDocument({
      title: 'PDF Test',
      author: 'Test',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 1
    })

    let thread = createThread({ fontFamily: 'Helvetica', fontSize: 14 })
    thread = insertText(thread, 0, 'Hello PDF world')
    doc.threads[thread.id] = thread

    doc = addTextFrame(doc, doc.pages[0].id, {
      rect: { x: 50, y: 50, width: 200, height: 300 },
      threadId: thread.id,
      threadOrder: 0
    })

    // Minimal mock layout
    const layout: DocumentLayout = {
      threadLayouts: {
        [thread.id]: {
          threadId: thread.id,
          frameLayouts: [{
            frameId: doc.pages[0].frames[0].id,
            pageId: doc.pages[0].id,
            lines: [{
              text: 'Hello PDF world',
              width: 120,
              x: 8,
              y: 8,
              height: 20,
              runStyles: [{
                text: 'Hello PDF world',
                style: { fontFamily: 'Helvetica', fontSize: 14 },
                x: 8,
                width: 120
              }]
            }],
            overflow: false,
            threadCursorEnd: 15
          }],
          totalTextLength: 15,
          overflow: false
        }
      }
    }

    const result = await exportToPdf(doc, layout)
    expect(result.pdfBytes.byteLength).toBeGreaterThan(0)
    expect(result.warnings).toEqual([])

    // Check PDF magic bytes
    const header = new Uint8Array(result.pdfBytes.slice(0, 5))
    const magic = String.fromCharCode(...header)
    expect(magic).toBe('%PDF-')
  })
})
```

- [ ] **Step 6: Run test to verify it fails**

```bash
npx vitest run tests/export/pdf-exporter.test.ts
```

Expected: FAIL

- [ ] **Step 7: Implement pdf-exporter.ts**

```typescript
// src/renderer/export/pdf-exporter.ts
import { PDFDocument, StandardFonts, rgb } from 'pdf-lib'
import type { Document, Page, TextFrame, ImageFrame } from '@model/types'
import type { DocumentLayout, FrameLayout, LayoutLine } from '@engine/layout-types'
import { resolveFont, resolveStandardFont } from './font-resolver'

interface ExportOptions {
  pageRange?: { start: number; end: number }
}

interface ExportResult {
  pdfBytes: ArrayBuffer
  warnings: string[]
}

export async function exportToPdf(
  doc: Document,
  layout: DocumentLayout,
  options: ExportOptions = {}
): Promise<ExportResult> {
  const pdfDoc = await PDFDocument.create()
  const warnings: string[] = []

  // Pre-load standard fonts
  const fontCache = new Map<string, Awaited<ReturnType<typeof pdfDoc.embedFont>>>()

  async function getFont(family: string, bold: boolean, italic: boolean) {
    const resolved = resolveFont(family, bold, italic)
    warnings.push(...resolved.warnings)

    if (!fontCache.has(resolved.fontName)) {
      const font = await pdfDoc.embedFont(resolved.fontName)
      fontCache.set(resolved.fontName, font)
    }
    return fontCache.get(resolved.fontName)!
  }

  // Determine page range
  const startPage = options.pageRange?.start ?? 0
  const endPage = options.pageRange?.end ?? doc.pages.length

  for (let i = startPage; i < endPage; i++) {
    const page = doc.pages[i]
    if (!page) continue

    const pdfPage = pdfDoc.addPage([
      doc.metadata.pageSize.width,
      doc.metadata.pageSize.height
    ])
    const pageHeight = doc.metadata.pageSize.height

    // Background image (if any)
    if (page.backgroundImageAssetId) {
      const asset = doc.assets[page.backgroundImageAssetId]
      if (asset) {
        try {
          let pdfImage
          if (asset.mimeType === 'image/jpeg') {
            pdfImage = await pdfDoc.embedJpg(new Uint8Array(asset.data))
          } else if (asset.mimeType === 'image/png') {
            pdfImage = await pdfDoc.embedPng(new Uint8Array(asset.data))
          }
          if (pdfImage) {
            pdfPage.drawImage(pdfImage, {
              x: 0,
              y: 0,
              width: doc.metadata.pageSize.width,
              height: doc.metadata.pageSize.height
            })
          }
        } catch (err) {
          warnings.push(`Failed to embed background image: ${asset.filename}`)
        }
      }
    }

    // Image frames
    for (const frame of page.frames) {
      if (frame.type !== 'image') continue
      const imgFrame = frame as ImageFrame
      if (!imgFrame.imageAssetId) continue

      const asset = doc.assets[imgFrame.imageAssetId]
      if (!asset) continue

      try {
        let pdfImage
        if (asset.mimeType === 'image/jpeg') {
          pdfImage = await pdfDoc.embedJpg(new Uint8Array(asset.data))
        } else if (asset.mimeType === 'image/png') {
          pdfImage = await pdfDoc.embedPng(new Uint8Array(asset.data))
        }
        if (pdfImage) {
          // PDF y-axis is bottom-up
          pdfPage.drawImage(pdfImage, {
            x: imgFrame.rect.x,
            y: pageHeight - imgFrame.rect.y - imgFrame.rect.height,
            width: imgFrame.rect.width,
            height: imgFrame.rect.height
          })
        }
      } catch (err) {
        warnings.push(`Failed to embed image: ${asset.filename}`)
      }
    }

    // Text frames
    for (const frame of page.frames) {
      if (frame.type !== 'text') continue
      const textFrame = frame as TextFrame

      const threadLayout = layout.threadLayouts[textFrame.threadId]
      const frameLayout = threadLayout?.frameLayouts.find(
        fl => fl.frameId === frame.id
      )
      if (!frameLayout) continue

      for (const line of frameLayout.lines) {
        for (const runStyle of line.runStyles) {
          const font = await getFont(
            runStyle.style.fontFamily ?? 'Helvetica',
            runStyle.style.bold ?? false,
            runStyle.style.italic ?? false
          )

          const fontSize = runStyle.style.fontSize ?? 14

          // Parse color
          let color = rgb(0, 0, 0)
          if (runStyle.style.color) {
            const hex = runStyle.style.color.replace('#', '')
            const r = parseInt(hex.slice(0, 2), 16) / 255
            const g = parseInt(hex.slice(2, 4), 16) / 255
            const b = parseInt(hex.slice(4, 6), 16) / 255
            color = rgb(r, g, b)
          }

          // PDF y-axis is bottom-up
          const pdfY = pageHeight - (textFrame.rect.y + line.y + line.height * 0.8)

          pdfPage.drawText(runStyle.text, {
            x: textFrame.rect.x + runStyle.x,
            y: pdfY,
            size: fontSize,
            font,
            color
          })
        }
      }

      // Continuation markers
      if (frameLayout.continuationTo) {
        const markerFont = await getFont('Helvetica', false, true)
        const markerY = pageHeight - (textFrame.rect.y + textFrame.rect.height - 4)
        pdfPage.drawText(`Cont. pg ${frameLayout.continuationTo.pageNumber}`, {
          x: textFrame.rect.x + 4,
          y: markerY,
          size: 10,
          font: markerFont,
          color: rgb(0.53, 0.53, 0.53)
        })
      }
    }
  }

  // Deduplicate warnings
  const uniqueWarnings = [...new Set(warnings)]

  const pdfBytes = await pdfDoc.save()
  return { pdfBytes: pdfBytes.buffer, warnings: uniqueWarnings }
}
```

- [ ] **Step 8: Run test to verify it passes**

```bash
npx vitest run tests/export/pdf-exporter.test.ts
```

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/renderer/export/ tests/export/
git commit -m "feat: implement PDF exporter with font resolution and image embedding"
```

---

## Task 15: React UI Shell — Layout and Canvas Host

**Files:**
- Create: `src/renderer/ui/components/DocumentCanvas.tsx`
- Create: `src/renderer/ui/components/Toolbar.tsx`
- Create: `src/renderer/ui/components/PageSidebar.tsx`
- Create: `src/renderer/ui/components/PropertiesPanel.tsx`
- Create: `src/renderer/ui/components/StatusBar.tsx`
- Modify: `src/renderer/App.tsx`

- [ ] **Step 1: Implement DocumentCanvas.tsx**

```tsx
// src/renderer/ui/components/DocumentCanvas.tsx
import { useRef, useEffect, useCallback } from 'react'
import { useEditorStore } from '@ui/store/editor-store'
import { render } from '@canvas/canvas-renderer'
import { layoutDocument } from '@engine/layout-engine'
import { screenToDoc, type Camera } from '@canvas/viewport'
import { hitTestFrames, hitTestCharacter } from '@input/hit-test'
import { getSpreadPositions } from '@canvas/viewport'

export default function DocumentCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const {
    document: doc,
    selection,
    setSelection,
    zoom,
    panX,
    panY,
    setPan,
    setZoom,
    activeTool
  } = useEditorStore()

  const camera: Camera = { zoom, panX, panY }

  const renderCanvas = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas || !doc) return

    // Resize canvas to container
    const rect = canvas.getBoundingClientRect()
    const dpr = window.devicePixelRatio || 1
    canvas.width = rect.width * dpr
    canvas.height = rect.height * dpr
    const ctx = canvas.getContext('2d')!
    ctx.scale(dpr, dpr)

    const layout = layoutDocument(doc)

    render({
      doc,
      layout,
      camera,
      canvas,
      selection,
      editMode: true,
      loadedImages: new Map()
    })
  }, [doc, selection, zoom, panX, panY])

  useEffect(() => {
    renderCanvas()
  }, [renderCanvas])

  // Resize observer
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const observer = new ResizeObserver(() => renderCanvas())
    observer.observe(canvas.parentElement!)
    return () => observer.disconnect()
  }, [renderCanvas])

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!doc) return
    const canvas = canvasRef.current!
    const rect = canvas.getBoundingClientRect()
    const screenPoint = {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top
    }
    const docPoint = screenToDoc(screenPoint, camera)
    const spreadPositions = getSpreadPositions(doc.pages.length, doc.metadata.pageSize)

    const hit = hitTestFrames(docPoint, doc.pages, spreadPositions)
    if (hit) {
      setSelection({ type: 'frame', frameId: hit.frame.id, pageId: hit.pageId })
    } else {
      setSelection(null)
    }
  }, [doc, camera, setSelection])

  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (e.ctrlKey || e.metaKey) {
      // Zoom
      e.preventDefault()
      const delta = -e.deltaY * 0.001
      setZoom(zoom + delta)
    } else {
      // Pan
      setPan(panX - e.deltaX, panY - e.deltaY)
    }
  }, [zoom, panX, panY, setZoom, setPan])

  return (
    <canvas
      ref={canvasRef}
      style={{ width: '100%', height: '100%', cursor: activeTool === 'select' ? 'default' : 'crosshair' }}
      onMouseDown={handleMouseDown}
      onWheel={handleWheel}
    />
  )
}
```

- [ ] **Step 2: Implement Toolbar.tsx**

```tsx
// src/renderer/ui/components/Toolbar.tsx
import { useEditorStore } from '@ui/store/editor-store'

export default function Toolbar() {
  const { activeTool, setActiveTool, zoom, setZoom } = useEditorStore()

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      gap: 8,
      padding: '4px 12px',
      borderBottom: '1px solid #ddd',
      background: '#fafafa',
      fontSize: 13
    }}>
      {/* Frame tools */}
      <div style={{ display: 'flex', gap: 2 }}>
        <ToolButton label="Select" active={activeTool === 'select'} onClick={() => setActiveTool('select')} />
        <ToolButton label="Text Frame" active={activeTool === 'draw-text-frame'} onClick={() => setActiveTool('draw-text-frame')} />
        <ToolButton label="Image Frame" active={activeTool === 'draw-image-frame'} onClick={() => setActiveTool('draw-image-frame')} />
      </div>

      <div style={{ width: 1, height: 20, background: '#ddd' }} />

      {/* Text formatting (active when editing text) */}
      <button style={btnStyle}>B</button>
      <button style={btnStyle}>I</button>

      <div style={{ flex: 1 }} />

      {/* Zoom */}
      <span style={{ color: '#666' }}>{Math.round(zoom * 100)}%</span>
      <input
        type="range"
        min={25}
        max={400}
        value={zoom * 100}
        onChange={e => setZoom(parseInt(e.target.value) / 100)}
        style={{ width: 100 }}
      />
    </div>
  )
}

function ToolButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        ...btnStyle,
        background: active ? '#6366f1' : 'transparent',
        color: active ? '#fff' : '#333'
      }}
    >
      {label}
    </button>
  )
}

const btnStyle: React.CSSProperties = {
  padding: '4px 8px',
  border: '1px solid #ddd',
  borderRadius: 4,
  cursor: 'pointer',
  fontSize: 12,
  background: 'transparent'
}
```

- [ ] **Step 3: Implement PageSidebar.tsx**

```tsx
// src/renderer/ui/components/PageSidebar.tsx
import { useEditorStore } from '@ui/store/editor-store'

export default function PageSidebar() {
  const { document: doc, currentPageIndex, setCurrentPageIndex } = useEditorStore()

  if (!doc) return <div style={containerStyle}><span style={{ color: '#999' }}>No document</span></div>

  // Group pages into spreads
  const spreads: Array<{ label: string; pageIndices: number[] }> = []

  if (doc.pages.length > 0) {
    spreads.push({ label: 'Page 1', pageIndices: [0] })
  }

  for (let i = 1; i < doc.pages.length; i += 2) {
    if (i + 1 < doc.pages.length) {
      spreads.push({ label: `Pages ${i + 1}–${i + 2}`, pageIndices: [i, i + 1] })
    } else {
      spreads.push({ label: `Page ${i + 1}`, pageIndices: [i] })
    }
  }

  return (
    <div style={containerStyle}>
      <div style={{ fontSize: 11, fontWeight: 600, color: '#666', padding: '8px 0', textTransform: 'uppercase', letterSpacing: 1 }}>
        Pages
      </div>
      {spreads.map((spread, i) => {
        const isActive = spread.pageIndices.includes(currentPageIndex)
        return (
          <div
            key={i}
            onClick={() => setCurrentPageIndex(spread.pageIndices[0])}
            style={{
              padding: '8px 4px',
              marginBottom: 4,
              borderRadius: 4,
              cursor: 'pointer',
              background: isActive ? '#eef2ff' : 'transparent',
              border: isActive ? '1px solid #c7d2fe' : '1px solid transparent',
              fontSize: 11,
              color: '#333'
            }}
          >
            {/* Thumbnail placeholder */}
            <div style={{
              display: 'flex',
              gap: 4,
              justifyContent: 'center',
              marginBottom: 4
            }}>
              {spread.pageIndices.map(pi => (
                <div
                  key={pi}
                  style={{
                    width: spread.pageIndices.length > 1 ? 40 : 50,
                    height: spread.pageIndices.length > 1 ? 52 : 65,
                    background: '#fff',
                    border: '1px solid #ccc',
                    borderRadius: 2
                  }}
                />
              ))}
            </div>
            <div style={{ textAlign: 'center' }}>{spread.label}</div>
          </div>
        )
      })}
    </div>
  )
}

const containerStyle: React.CSSProperties = {
  width: 120,
  borderRight: '1px solid #ddd',
  padding: '8px',
  overflowY: 'auto',
  background: '#f8f8f8',
  flexShrink: 0
}
```

- [ ] **Step 4: Implement PropertiesPanel.tsx**

```tsx
// src/renderer/ui/components/PropertiesPanel.tsx
import { useEditorStore } from '@ui/store/editor-store'
import type { Frame, TextFrame, ImageFrame } from '@model/types'

export default function PropertiesPanel() {
  const { document: doc, selection } = useEditorStore()

  if (!doc) return <Panel>No document open</Panel>
  if (!selection) return <Panel><DocumentProperties doc={doc} /></Panel>
  if (selection.type === 'frame') {
    const page = doc.pages.find(p => p.id === selection.pageId)
    const frame = page?.frames.find(f => f.id === selection.frameId)
    if (!frame) return <Panel>Frame not found</Panel>

    if (frame.type === 'text') return <Panel><TextFrameProperties frame={frame as TextFrame} /></Panel>
    if (frame.type === 'image') return <Panel><ImageFrameProperties frame={frame as ImageFrame} /></Panel>
  }

  return <Panel>Select an element</Panel>
}

function Panel({ children }: { children: React.ReactNode }) {
  return (
    <div style={{
      width: 220,
      borderLeft: '1px solid #ddd',
      padding: 12,
      overflowY: 'auto',
      background: '#f8f8f8',
      flexShrink: 0,
      fontSize: 12
    }}>
      {children}
    </div>
  )
}

function DocumentProperties({ doc }: { doc: import('@model/types').Document }) {
  return (
    <div>
      <SectionLabel>Document</SectionLabel>
      <PropRow label="Title" value={doc.metadata.title} />
      <PropRow label="Pages" value={String(doc.pages.length)} />
      <PropRow label="Size" value={`${doc.metadata.pageSize.width} × ${doc.metadata.pageSize.height} pt`} />
    </div>
  )
}

function TextFrameProperties({ frame }: { frame: TextFrame }) {
  return (
    <div>
      <SectionLabel>Text Frame</SectionLabel>
      <PropRow label="X" value={String(Math.round(frame.rect.x))} />
      <PropRow label="Y" value={String(Math.round(frame.rect.y))} />
      <PropRow label="Width" value={String(Math.round(frame.rect.width))} />
      <PropRow label="Height" value={String(Math.round(frame.rect.height))} />
      <PropRow label="Thread" value={frame.threadId} />
      <PropRow label="Order" value={String(frame.threadOrder)} />
    </div>
  )
}

function ImageFrameProperties({ frame }: { frame: ImageFrame }) {
  return (
    <div>
      <SectionLabel>Image Frame</SectionLabel>
      <PropRow label="X" value={String(Math.round(frame.rect.x))} />
      <PropRow label="Y" value={String(Math.round(frame.rect.y))} />
      <PropRow label="Width" value={String(Math.round(frame.rect.width))} />
      <PropRow label="Height" value={String(Math.round(frame.rect.height))} />
      <PropRow label="Wrap" value={frame.wrapMode} />
      <PropRow label="Fit" value={frame.imageFit} />
    </div>
  )
}

function SectionLabel({ children }: { children: string }) {
  return <div style={{ fontWeight: 600, color: '#666', marginBottom: 8, textTransform: 'uppercase', letterSpacing: 1, fontSize: 10 }}>{children}</div>
}

function PropRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
      <span style={{ color: '#888' }}>{label}</span>
      <span>{value}</span>
    </div>
  )
}
```

- [ ] **Step 5: Implement StatusBar.tsx**

```tsx
// src/renderer/ui/components/StatusBar.tsx
import { useEditorStore } from '@ui/store/editor-store'

export default function StatusBar() {
  const { document: doc, zoom, currentPageIndex } = useEditorStore()

  const pageCount = doc?.pages.length ?? 0

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '2px 12px',
      borderTop: '1px solid #ddd',
      background: '#fafafa',
      fontSize: 11,
      color: '#666',
      height: 24
    }}>
      <span>
        {pageCount > 0
          ? `Page ${currentPageIndex + 1} of ${pageCount}`
          : 'No document'
        }
      </span>
      <span>{Math.round(zoom * 100)}%</span>
    </div>
  )
}
```

- [ ] **Step 6: Wire up App.tsx**

```tsx
// src/renderer/App.tsx
import { useEffect } from 'react'
import Toolbar from '@ui/components/Toolbar'
import PageSidebar from '@ui/components/PageSidebar'
import DocumentCanvas from '@ui/components/DocumentCanvas'
import PropertiesPanel from '@ui/components/PropertiesPanel'
import StatusBar from '@ui/components/StatusBar'
import { useEditorStore } from '@ui/store/editor-store'
import { presetTemplates } from '@template/preset-templates'

export default function App() {
  const { document: doc, setDocument } = useEditorStore()

  // Load a default template on first launch (temporary — will be replaced by template picker)
  useEffect(() => {
    if (!doc) {
      const template = presetTemplates[0]
      setDocument(template.build())
    }
  }, [doc, setDocument])

  return (
    <div style={{ width: '100vw', height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      <Toolbar />
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <PageSidebar />
        <div style={{ flex: 1, overflow: 'hidden', position: 'relative' }}>
          <DocumentCanvas />
        </div>
        <PropertiesPanel />
      </div>
      <StatusBar />
    </div>
  )
}
```

- [ ] **Step 7: Verify the UI renders**

```bash
npm run dev
```

Expected: Electron window opens showing the full layout — toolbar at top, page sidebar on the left, canvas in the center, properties panel on the right, status bar at the bottom. The canvas should show a basic rendering of the Classic 4-Page template.

- [ ] **Step 8: Commit**

```bash
git add src/renderer/ui/ src/renderer/App.tsx
git commit -m "feat: build React UI shell with toolbar, sidebar, canvas, properties, status bar"
```

---

## Task 16: IPC Handlers and Native Menu

**Files:**
- Create: `src/main/ipc-handlers.ts`
- Modify: `src/main/menu.ts` (create)
- Modify: `src/main/main.ts`

- [ ] **Step 1: Implement ipc-handlers.ts**

```typescript
// src/main/ipc-handlers.ts
import { ipcMain, dialog } from 'electron'
import { readFile, writeFile, rename, unlink } from 'fs/promises'
import { join, homedir } from 'path'
import { existsSync } from 'fs'

export function registerIpcHandlers(): void {
  ipcMain.handle('file:save', async (_event, data: ArrayBuffer, path: string) => {
    // Atomic write: write to .tmp, then rename
    const tmpPath = path + '.tmp'
    await writeFile(tmpPath, Buffer.from(data))
    await rename(tmpPath, path)
    return { success: true }
  })

  ipcMain.handle('file:open', async (_event, path: string) => {
    const buffer = await readFile(path)
    return buffer.buffer
  })

  ipcMain.handle('dialog:save', async (_event, options: object) => {
    const result = await dialog.showSaveDialog({
      filters: [{ name: 'NewsPub Files', extensions: ['newspub'] }],
      ...options
    })
    return result
  })

  ipcMain.handle('dialog:open', async (_event, options: object) => {
    const result = await dialog.showOpenDialog({
      filters: [{ name: 'NewsPub Files', extensions: ['newspub'] }],
      properties: ['openFile'],
      ...options
    })
    return result
  })

  ipcMain.handle('export:pdf', async (_event, data: ArrayBuffer, path: string) => {
    await writeFile(path, Buffer.from(data))
    return { success: true }
  })

  ipcMain.handle('system:fontsDir', async () => {
    const platform = process.platform
    if (platform === 'darwin') {
      return [
        join(homedir(), 'Library/Fonts'),
        '/System/Library/Fonts',
        '/Library/Fonts'
      ]
    } else if (platform === 'win32') {
      return ['C:\\Windows\\Fonts']
    } else {
      return [
        join(homedir(), '.fonts'),
        '/usr/share/fonts',
        '/usr/local/share/fonts'
      ]
    }
  })
}
```

- [ ] **Step 2: Implement menu.ts**

```typescript
// src/main/menu.ts
import { Menu, BrowserWindow } from 'electron'

export function buildMenu(): Menu {
  const template: Electron.MenuItemConstructorOptions[] = [
    {
      label: 'File',
      submenu: [
        { label: 'New', accelerator: 'CmdOrCtrl+N', click: (_, win) => win?.webContents.send('menu:new') },
        { label: 'Open...', accelerator: 'CmdOrCtrl+O', click: (_, win) => win?.webContents.send('menu:open') },
        { type: 'separator' },
        { label: 'Save', accelerator: 'CmdOrCtrl+S', click: (_, win) => win?.webContents.send('menu:save') },
        { label: 'Save As...', accelerator: 'CmdOrCtrl+Shift+S', click: (_, win) => win?.webContents.send('menu:save-as') },
        { label: 'Save As Template...', click: (_, win) => win?.webContents.send('menu:save-template') },
        { type: 'separator' },
        { label: 'Export PDF...', accelerator: 'CmdOrCtrl+E', click: (_, win) => win?.webContents.send('menu:export-pdf') },
        { type: 'separator' },
        { role: 'quit' }
      ]
    },
    {
      label: 'Edit',
      submenu: [
        { label: 'Undo', accelerator: 'CmdOrCtrl+Z', click: (_, win) => win?.webContents.send('menu:undo') },
        { label: 'Redo', accelerator: 'CmdOrCtrl+Shift+Z', click: (_, win) => win?.webContents.send('menu:redo') },
        { type: 'separator' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        { role: 'delete' },
        { type: 'separator' },
        { role: 'selectAll' }
      ]
    },
    {
      label: 'View',
      submenu: [
        { label: 'Zoom In', accelerator: 'CmdOrCtrl+Plus', click: (_, win) => win?.webContents.send('menu:zoom-in') },
        { label: 'Zoom Out', accelerator: 'CmdOrCtrl+-', click: (_, win) => win?.webContents.send('menu:zoom-out') },
        { label: 'Fit to Window', accelerator: 'CmdOrCtrl+0', click: (_, win) => win?.webContents.send('menu:zoom-fit') },
        { type: 'separator' },
        { role: 'toggleDevTools' }
      ]
    },
    {
      label: 'Insert',
      submenu: [
        { label: 'Text Frame', click: (_, win) => win?.webContents.send('menu:insert-text-frame') },
        { label: 'Image Frame', click: (_, win) => win?.webContents.send('menu:insert-image-frame') },
        { type: 'separator' },
        { label: 'Add Spread', click: (_, win) => win?.webContents.send('menu:add-spread') }
      ]
    },
    {
      label: 'Format',
      submenu: [
        { label: 'Bold', accelerator: 'CmdOrCtrl+B', click: (_, win) => win?.webContents.send('menu:bold') },
        { label: 'Italic', accelerator: 'CmdOrCtrl+I', click: (_, win) => win?.webContents.send('menu:italic') }
      ]
    },
    {
      label: 'Help',
      submenu: [
        { label: 'About NewsPub', click: (_, win) => win?.webContents.send('menu:about') }
      ]
    }
  ]

  // macOS app menu
  if (process.platform === 'darwin') {
    template.unshift({
      label: 'NewsPub',
      submenu: [
        { role: 'about' },
        { type: 'separator' },
        { role: 'hide' },
        { role: 'hideOthers' },
        { role: 'unhide' },
        { type: 'separator' },
        { role: 'quit' }
      ]
    })
  }

  return Menu.buildFromTemplate(template)
}
```

- [ ] **Step 3: Update main.ts to use IPC and menu**

Update `src/main/main.ts` to import and register:

```typescript
// Add at top of main.ts:
import { registerIpcHandlers } from './ipc-handlers'
import { buildMenu } from './menu'
import { Menu } from 'electron'

// Add inside app.whenReady().then():
registerIpcHandlers()
Menu.setApplicationMenu(buildMenu())
```

- [ ] **Step 4: Verify the app launches with menus**

```bash
npm run dev
```

Expected: Electron app opens with native menus (File, Edit, View, Insert, Format, Help).

- [ ] **Step 5: Commit**

```bash
git add src/main/
git commit -m "feat: add IPC handlers for file I/O and native application menu"
```

---

## Task 17: Dialog Components

**Files:**
- Create: `src/renderer/ui/components/TemplatePickerDialog.tsx`
- Create: `src/renderer/ui/components/SaveAsTemplateDialog.tsx`
- Create: `src/renderer/ui/components/DocumentSetupDialog.tsx`
- Create: `src/renderer/ui/components/ExportDialog.tsx`

- [ ] **Step 1: Implement TemplatePickerDialog.tsx**

```tsx
// src/renderer/ui/components/TemplatePickerDialog.tsx
import { useState } from 'react'
import { presetTemplates } from '@template/preset-templates'
import type { Document } from '@model/types'

interface Props {
  onSelect: (doc: Document) => void
  onCancel: () => void
}

export default function TemplatePickerDialog({ onSelect, onCancel }: Props) {
  const [selected, setSelected] = useState(0)

  return (
    <div style={overlayStyle}>
      <div style={dialogStyle}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>New Document</h2>
        <p style={{ color: '#666', marginBottom: 16, fontSize: 13 }}>Choose a template to get started</p>

        <div style={{ display: 'flex', gap: 12, marginBottom: 24 }}>
          {presetTemplates.map((tmpl, i) => (
            <div
              key={i}
              onClick={() => setSelected(i)}
              style={{
                padding: 12,
                border: selected === i ? '2px solid #6366f1' : '2px solid #ddd',
                borderRadius: 8,
                cursor: 'pointer',
                width: 160,
                textAlign: 'center'
              }}
            >
              <div style={{
                width: '100%',
                height: 100,
                background: '#f5f5f5',
                borderRadius: 4,
                marginBottom: 8
              }} />
              <div style={{ fontWeight: 600, fontSize: 13 }}>{tmpl.metadata.name}</div>
              <div style={{ color: '#888', fontSize: 11, marginTop: 4 }}>{tmpl.metadata.description}</div>
            </div>
          ))}
        </div>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          <button onClick={onCancel} style={cancelBtnStyle}>Cancel</button>
          <button onClick={() => onSelect(presetTemplates[selected].build())} style={primaryBtnStyle}>
            Create
          </button>
        </div>
      </div>
    </div>
  )
}

const overlayStyle: React.CSSProperties = {
  position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)',
  display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000
}

const dialogStyle: React.CSSProperties = {
  background: '#fff', borderRadius: 12, padding: 24,
  maxWidth: 500, width: '90%', boxShadow: '0 8px 32px rgba(0,0,0,0.2)'
}

const primaryBtnStyle: React.CSSProperties = {
  padding: '8px 16px', background: '#6366f1', color: '#fff',
  border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13
}

const cancelBtnStyle: React.CSSProperties = {
  padding: '8px 16px', background: 'transparent', color: '#666',
  border: '1px solid #ddd', borderRadius: 6, cursor: 'pointer', fontSize: 13
}
```

- [ ] **Step 2: Implement SaveAsTemplateDialog.tsx**

```tsx
// src/renderer/ui/components/SaveAsTemplateDialog.tsx
import { useState } from 'react'
import type { TemplateSaveOptions } from '@model/types'

interface Props {
  defaultTitle: string
  onSave: (name: string, description: string, options: TemplateSaveOptions) => void
  onCancel: () => void
}

export default function SaveAsTemplateDialog({ defaultTitle, onSave, onCancel }: Props) {
  const [name, setName] = useState(defaultTitle)
  const [description, setDescription] = useState('')
  const [keepText, setKeepText] = useState(false)
  const [keepImages, setKeepImages] = useState(false)
  const [keepBackground, setKeepBackground] = useState(true)

  return (
    <div style={overlayStyle}>
      <div style={dialogStyle}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>Save As Template</h2>

        <label style={labelStyle}>Template Name</label>
        <input value={name} onChange={e => setName(e.target.value)} style={inputStyle} />

        <label style={labelStyle}>Description</label>
        <input value={description} onChange={e => setDescription(e.target.value)} style={inputStyle} />

        <div style={{ borderTop: '1px solid #eee', paddingTop: 12, marginTop: 8 }}>
          <label style={labelStyle}>Content to Keep</label>

          <Checkbox checked={keepText} onChange={setKeepText} label="Article text" description="Keep text content in all text frames" />
          <Checkbox checked={keepImages} onChange={setKeepImages} label="Placed images" description="Keep images in image frames" />
          <Checkbox checked={keepBackground} onChange={setKeepBackground} label="Background & decoration images" description="Keep page backgrounds, borders, banners" />
        </div>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 16 }}>
          <button onClick={onCancel} style={cancelBtnStyle}>Cancel</button>
          <button
            onClick={() => onSave(name, description, {
              keepArticleText: keepText,
              keepPlacedImages: keepImages,
              keepBackgroundImages: keepBackground
            })}
            style={primaryBtnStyle}
          >
            Save Template
          </button>
        </div>
      </div>
    </div>
  )
}

function Checkbox({ checked, onChange, label, description }: {
  checked: boolean; onChange: (v: boolean) => void; label: string; description: string
}) {
  return (
    <div
      onClick={() => onChange(!checked)}
      style={{
        display: 'flex', gap: 10, padding: 8, marginBottom: 6,
        borderRadius: 6, cursor: 'pointer',
        background: checked ? 'rgba(99,102,241,0.08)' : '#f8f8f8',
        border: checked ? '1px solid rgba(99,102,241,0.2)' : '1px solid transparent'
      }}
    >
      <div style={{
        width: 18, height: 18, borderRadius: 4, flexShrink: 0,
        background: checked ? '#6366f1' : 'transparent',
        border: checked ? 'none' : '2px solid #ccc',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        color: '#fff', fontSize: 12, fontWeight: 'bold'
      }}>
        {checked && '✓'}
      </div>
      <div>
        <div style={{ fontWeight: 600, fontSize: 13 }}>{label}</div>
        <div style={{ fontSize: 11, color: '#888', marginTop: 2 }}>{description}</div>
      </div>
    </div>
  )
}

const overlayStyle: React.CSSProperties = {
  position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)',
  display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000
}
const dialogStyle: React.CSSProperties = {
  background: '#fff', borderRadius: 12, padding: 24,
  maxWidth: 420, width: '90%', boxShadow: '0 8px 32px rgba(0,0,0,0.2)'
}
const labelStyle: React.CSSProperties = {
  display: 'block', fontSize: 11, color: '#888', marginBottom: 4, marginTop: 12,
  textTransform: 'uppercase', letterSpacing: 1
}
const inputStyle: React.CSSProperties = {
  width: '100%', padding: '8px 10px', border: '1px solid #ddd', borderRadius: 6,
  fontSize: 13, boxSizing: 'border-box'
}
const primaryBtnStyle: React.CSSProperties = {
  padding: '8px 16px', background: '#6366f1', color: '#fff',
  border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13
}
const cancelBtnStyle: React.CSSProperties = {
  padding: '8px 16px', background: 'transparent', color: '#666',
  border: '1px solid #ddd', borderRadius: 6, cursor: 'pointer', fontSize: 13
}
```

- [ ] **Step 3: Implement DocumentSetupDialog.tsx and ExportDialog.tsx**

```tsx
// src/renderer/ui/components/DocumentSetupDialog.tsx
import { useState } from 'react'
import { PAGE_SIZES } from '@model/page-sizes'
import type { Size } from '@model/types'

interface Props {
  currentSize: Size
  onApply: (size: Size, scaleFrames: boolean) => void
  onCancel: () => void
}

export default function DocumentSetupDialog({ currentSize, onApply, onCancel }: Props) {
  const [selectedPreset, setSelectedPreset] = useState<string | 'custom'>(
    Object.entries(PAGE_SIZES).find(
      ([_, s]) => s.width === currentSize.width && s.height === currentSize.height
    )?.[0] ?? 'custom'
  )
  const [customW, setCustomW] = useState(currentSize.width)
  const [customH, setCustomH] = useState(currentSize.height)
  const [scaleFrames, setScaleFrames] = useState(true)

  const resolvedSize = selectedPreset === 'custom'
    ? { width: customW, height: customH }
    : PAGE_SIZES[selectedPreset]

  return (
    <div style={overlayStyle}>
      <div style={dialogStyle}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>Document Setup</h2>

        <label style={labelStyle}>Page Size</label>
        <select
          value={selectedPreset}
          onChange={e => setSelectedPreset(e.target.value)}
          style={{ ...inputStyle, cursor: 'pointer' }}
        >
          {Object.keys(PAGE_SIZES).map(name => (
            <option key={name} value={name}>{name}</option>
          ))}
          <option value="custom">Custom</option>
        </select>

        {selectedPreset === 'custom' && (
          <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
            <div>
              <label style={labelStyle}>Width (pt)</label>
              <input type="number" value={customW} onChange={e => setCustomW(Number(e.target.value))} style={inputStyle} />
            </div>
            <div>
              <label style={labelStyle}>Height (pt)</label>
              <input type="number" value={customH} onChange={e => setCustomH(Number(e.target.value))} style={inputStyle} />
            </div>
          </div>
        )}

        <div style={{ marginTop: 12 }}>
          <label style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13, cursor: 'pointer' }}>
            <input type="checkbox" checked={scaleFrames} onChange={e => setScaleFrames(e.target.checked)} />
            Scale frame positions proportionally
          </label>
        </div>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 16 }}>
          <button onClick={onCancel} style={cancelBtnStyle}>Cancel</button>
          <button onClick={() => onApply(resolvedSize, scaleFrames)} style={primaryBtnStyle}>Apply</button>
        </div>
      </div>
    </div>
  )
}

const overlayStyle: React.CSSProperties = { position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }
const dialogStyle: React.CSSProperties = { background: '#fff', borderRadius: 12, padding: 24, maxWidth: 380, width: '90%', boxShadow: '0 8px 32px rgba(0,0,0,0.2)' }
const labelStyle: React.CSSProperties = { display: 'block', fontSize: 11, color: '#888', marginBottom: 4, marginTop: 8, textTransform: 'uppercase', letterSpacing: 1 }
const inputStyle: React.CSSProperties = { width: '100%', padding: '8px 10px', border: '1px solid #ddd', borderRadius: 6, fontSize: 13, boxSizing: 'border-box' }
const primaryBtnStyle: React.CSSProperties = { padding: '8px 16px', background: '#6366f1', color: '#fff', border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13 }
const cancelBtnStyle: React.CSSProperties = { padding: '8px 16px', background: 'transparent', color: '#666', border: '1px solid #ddd', borderRadius: 6, cursor: 'pointer', fontSize: 13 }
```

```tsx
// src/renderer/ui/components/ExportDialog.tsx
import { useState } from 'react'

interface Props {
  pageCount: number
  onExport: (options: { allPages: boolean; startPage: number; endPage: number }) => void
  onCancel: () => void
}

export default function ExportDialog({ pageCount, onExport, onCancel }: Props) {
  const [allPages, setAllPages] = useState(true)
  const [startPage, setStartPage] = useState(1)
  const [endPage, setEndPage] = useState(pageCount)

  return (
    <div style={overlayStyle}>
      <div style={dialogStyle}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>Export PDF</h2>

        <label style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13, cursor: 'pointer', marginBottom: 8 }}>
          <input type="radio" checked={allPages} onChange={() => setAllPages(true)} />
          All pages ({pageCount})
        </label>
        <label style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13, cursor: 'pointer' }}>
          <input type="radio" checked={!allPages} onChange={() => setAllPages(false)} />
          Pages
          <input type="number" min={1} max={pageCount} value={startPage} onChange={e => setStartPage(Number(e.target.value))} disabled={allPages} style={{ width: 50, padding: '4px 6px', border: '1px solid #ddd', borderRadius: 4 }} />
          to
          <input type="number" min={1} max={pageCount} value={endPage} onChange={e => setEndPage(Number(e.target.value))} disabled={allPages} style={{ width: 50, padding: '4px 6px', border: '1px solid #ddd', borderRadius: 4 }} />
        </label>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 20 }}>
          <button onClick={onCancel} style={cancelBtnStyle}>Cancel</button>
          <button onClick={() => onExport({
            allPages,
            startPage: startPage - 1,
            endPage: allPages ? pageCount : endPage
          })} style={primaryBtnStyle}>Export</button>
        </div>
      </div>
    </div>
  )
}

const overlayStyle: React.CSSProperties = { position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }
const dialogStyle: React.CSSProperties = { background: '#fff', borderRadius: 12, padding: 24, maxWidth: 350, width: '90%', boxShadow: '0 8px 32px rgba(0,0,0,0.2)' }
const primaryBtnStyle: React.CSSProperties = { padding: '8px 16px', background: '#6366f1', color: '#fff', border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13 }
const cancelBtnStyle: React.CSSProperties = { padding: '8px 16px', background: 'transparent', color: '#666', border: '1px solid #ddd', borderRadius: 6, cursor: 'pointer', fontSize: 13 }
```

- [ ] **Step 4: Commit**

```bash
git add src/renderer/ui/components/
git commit -m "feat: add template picker, save-as-template, document setup, and export dialogs"
```

---

## Task 18: Integration — Wire Everything Together

**Files:**
- Modify: `src/renderer/App.tsx`
- Modify: `src/renderer/ui/store/editor-store.ts`
- Modify: `src/preload/preload.ts`

- [ ] **Step 1: Add menu event listeners and dialog state to App.tsx**

```tsx
// Replace src/renderer/App.tsx with:
import { useEffect, useState, useCallback } from 'react'
import Toolbar from '@ui/components/Toolbar'
import PageSidebar from '@ui/components/PageSidebar'
import DocumentCanvas from '@ui/components/DocumentCanvas'
import PropertiesPanel from '@ui/components/PropertiesPanel'
import StatusBar from '@ui/components/StatusBar'
import TemplatePickerDialog from '@ui/components/TemplatePickerDialog'
import SaveAsTemplateDialog from '@ui/components/SaveAsTemplateDialog'
import ExportDialog from '@ui/components/ExportDialog'
import DocumentSetupDialog from '@ui/components/DocumentSetupDialog'
import { useEditorStore } from '@ui/store/editor-store'
import { presetTemplates } from '@template/preset-templates'
import { HistoryManager } from '@history/history-manager'
import { packNewspub, unpackNewspub } from '@file/file-manager'
import { layoutDocument } from '@engine/layout-engine'
import { exportToPdf } from '@export/pdf-exporter'
import { stripToTemplate } from '@template/template-manager'
import type { Document } from '@model/types'

const history = new HistoryManager()

type DialogState =
  | { type: 'none' }
  | { type: 'template-picker' }
  | { type: 'save-as-template' }
  | { type: 'export' }
  | { type: 'document-setup' }

export default function App() {
  const { document: doc, setDocument, filePath, setFilePath, markClean } = useEditorStore()
  const [dialog, setDialog] = useState<DialogState>({ type: 'template-picker' })

  // Listen for menu events from main process
  useEffect(() => {
    const api = (window as any).electronAPI
    if (!api?.onMenuEvent) return

    const handlers: Record<string, () => void> = {
      'menu:new': () => setDialog({ type: 'template-picker' }),
      'menu:save': () => handleSave(),
      'menu:save-as': () => handleSaveAs(),
      'menu:save-template': () => setDialog({ type: 'save-as-template' }),
      'menu:export-pdf': () => setDialog({ type: 'export' }),
      'menu:undo': () => { history.undo(); /* re-read doc from store */ },
      'menu:redo': () => { history.redo(); },
    }

    for (const [channel, handler] of Object.entries(handlers)) {
      api.onMenuEvent(channel, handler)
    }
  }, [])

  const handleSave = useCallback(async () => {
    if (!doc) return
    const api = (window as any).electronAPI
    let path = filePath
    if (!path) {
      const result = await api.showSaveDialog({})
      if (result.canceled || !result.filePath) return
      path = result.filePath
      setFilePath(path)
    }
    const buffer = await packNewspub(doc)
    await api.saveFile(buffer, path)
    markClean()
  }, [doc, filePath, setFilePath, markClean])

  const handleSaveAs = useCallback(async () => {
    if (!doc) return
    const api = (window as any).electronAPI
    const result = await api.showSaveDialog({})
    if (result.canceled || !result.filePath) return
    setFilePath(result.filePath)
    const buffer = await packNewspub(doc)
    await api.saveFile(buffer, result.filePath)
    markClean()
  }, [doc, setFilePath, markClean])

  const handleExport = useCallback(async (options: { allPages: boolean; startPage: number; endPage: number }) => {
    if (!doc) return
    const api = (window as any).electronAPI
    const result = await api.showSaveDialog({ filters: [{ name: 'PDF', extensions: ['pdf'] }] })
    if (result.canceled || !result.filePath) return

    const layout = layoutDocument(doc)
    const pageRange = options.allPages ? undefined : { start: options.startPage, end: options.endPage }
    const { pdfBytes, warnings } = await exportToPdf(doc, layout, { pageRange })

    if (warnings.length > 0) {
      console.warn('PDF export warnings:', warnings)
    }

    await api.exportPDF(pdfBytes, result.filePath)
    setDialog({ type: 'none' })
  }, [doc])

  return (
    <div style={{ width: '100vw', height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      <Toolbar />
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <PageSidebar />
        <div style={{ flex: 1, overflow: 'hidden', position: 'relative' }}>
          <DocumentCanvas />
        </div>
        <PropertiesPanel />
      </div>
      <StatusBar />

      {dialog.type === 'template-picker' && (
        <TemplatePickerDialog
          onSelect={(newDoc) => { setDocument(newDoc); setDialog({ type: 'none' }); history.clear(); }}
          onCancel={() => { if (doc) setDialog({ type: 'none' }) }}
        />
      )}
      {dialog.type === 'save-as-template' && doc && (
        <SaveAsTemplateDialog
          defaultTitle={doc.metadata.title}
          onSave={async (name, desc, options) => {
            const template = stripToTemplate(doc, options)
            template.metadata.title = name
            const buffer = await packNewspub(template)
            const api = (window as any).electronAPI
            const result = await api.showSaveDialog({ filters: [{ name: 'NewsPub Template', extensions: ['newspub'] }] })
            if (!result.canceled && result.filePath) {
              await api.saveFile(buffer, result.filePath)
            }
            setDialog({ type: 'none' })
          }}
          onCancel={() => setDialog({ type: 'none' })}
        />
      )}
      {dialog.type === 'export' && doc && (
        <ExportDialog
          pageCount={doc.pages.length}
          onExport={handleExport}
          onCancel={() => setDialog({ type: 'none' })}
        />
      )}
      {dialog.type === 'document-setup' && doc && (
        <DocumentSetupDialog
          currentSize={doc.metadata.pageSize}
          onApply={(size, scale) => {
            // TODO: scale frames proportionally if scale=true
            setDocument({ ...doc, metadata: { ...doc.metadata, pageSize: size } })
            setDialog({ type: 'none' })
          }}
          onCancel={() => setDialog({ type: 'none' })}
        />
      )}
    </div>
  )
}
```

- [ ] **Step 2: Add onMessage to preload**

Add `onMenuEvent` to the preload bridge:

```typescript
// Add to preload.ts:
onMenuEvent: (channel: string, callback: (...args: any[]) => void) => {
  ipcRenderer.on(channel, (_event, ...args) => callback(...args))
}
```

- [ ] **Step 3: Run all tests**

```bash
npx vitest run
```

Expected: All tests pass.

- [ ] **Step 4: Launch and verify end-to-end**

```bash
npm run dev
```

Expected: Full working app — can view template, select frames, see properties panel update, use menus.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: wire menu events, dialogs, and store for end-to-end workflow"
```

---

## Task 19: Frame Drag, Move, and Resize

**Files:**
- Modify: `src/renderer/ui/components/DocumentCanvas.tsx`

- [ ] **Step 1: Add mouse interaction state and handlers to DocumentCanvas**

Add `handleMouseMove` and `handleMouseUp` handlers that:

1. **Frame move:** When a selected frame is clicked (not on a resize handle), track mouse delta and update `frame.rect.x`/`frame.rect.y` in the document model. Create a history command with a shared `groupId` for the entire drag so it undoes as one step.

2. **Frame resize:** When a resize handle is clicked, track mouse delta and update the appropriate edges of `frame.rect` (e.g., `bottom-right` adjusts `width` and `height`). Same groupId-based undo grouping.

3. **Draw new frame:** When `activeTool` is `draw-text-frame` or `draw-image-frame`, mousedown starts a rubber-band rectangle. On mouseup, create the frame at that rect and switch tool back to `select`.

Key interaction state to track:
```typescript
type DragState =
  | null
  | { type: 'move-frame'; frameId: string; pageId: string; startX: number; startY: number; origRect: Rect; groupId: string }
  | { type: 'resize-frame'; frameId: string; pageId: string; handle: HandlePosition; startX: number; startY: number; origRect: Rect; groupId: string }
  | { type: 'draw-frame'; startX: number; startY: number; currentX: number; currentY: number }
```

- [ ] **Step 2: Test frame move/resize manually**

```bash
npm run dev
```

Expected: Can drag frames to move them, drag handles to resize, draw new text/image frames with the toolbar tools.

- [ ] **Step 3: Commit**

```bash
git add src/renderer/ui/components/DocumentCanvas.tsx
git commit -m "feat: add frame drag/move, resize, and draw-new-frame interaction"
```

---

## Task 20: Keyboard Shortcuts

**Files:**
- Modify: `src/renderer/ui/components/DocumentCanvas.tsx`

- [ ] **Step 1: Add keydown handler to DocumentCanvas**

Add a `useEffect` that listens for `keydown` on the window:

```typescript
useEffect(() => {
  function handleKeyDown(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey

    // Frame nudge (when a frame is selected, not editing text)
    if (selection?.type === 'frame' && !mod) {
      const nudge = e.shiftKey ? 10 : 1
      switch (e.key) {
        case 'ArrowLeft':
          e.preventDefault()
          moveSelectedFrame(-nudge, 0)
          break
        case 'ArrowRight':
          e.preventDefault()
          moveSelectedFrame(nudge, 0)
          break
        case 'ArrowUp':
          e.preventDefault()
          moveSelectedFrame(0, -nudge)
          break
        case 'ArrowDown':
          e.preventDefault()
          moveSelectedFrame(0, nudge)
          break
        case 'Delete':
        case 'Backspace':
          e.preventDefault()
          deleteSelectedFrame()
          break
        case 'Tab':
          e.preventDefault()
          cycleToNextThreadFrame()
          break
      }
    }

    // Formatting shortcuts (when editing text)
    if (selection?.type === 'text' && mod) {
      switch (e.key) {
        case 'b':
          e.preventDefault()
          toggleBold()
          break
        case 'i':
          e.preventDefault()
          toggleItalic()
          break
      }
    }
  }

  window.addEventListener('keydown', handleKeyDown)
  return () => window.removeEventListener('keydown', handleKeyDown)
}, [selection])
```

Implement `moveSelectedFrame`, `deleteSelectedFrame`, `cycleToNextThreadFrame`, `toggleBold`, `toggleItalic` as functions that update the document model through the store and push history commands.

- [ ] **Step 2: Test keyboard shortcuts manually**

```bash
npm run dev
```

Expected: Arrow keys nudge selected frames, Shift+arrow nudges by 10pt, Delete removes frame, Tab cycles thread frames, Cmd+B/I toggles bold/italic on text selection.

- [ ] **Step 3: Commit**

```bash
git add src/renderer/ui/components/DocumentCanvas.tsx
git commit -m "feat: add keyboard shortcuts for frame nudge, delete, tab cycle, and text formatting"
```

---

## Task 21: Final Verification and Build

**Note on v1 PDF font limitation:** The font resolver currently maps non-standard fonts to Helvetica/Times/Courier fallbacks. PDF export will render correctly for these standard fonts. Custom font embedding (reading system `.ttf`/`.otf` files) is a v2 enhancement — the architecture supports it via `font-resolver.ts` but the file-scanning logic is not yet implemented.

- [ ] **Step 1: Run full test suite**

```bash
npx vitest run
```

Expected: All tests pass.

- [ ] **Step 2: Build the Electron app**

```bash
npm run build
```

Expected: Build completes without errors.

- [ ] **Step 3: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final build verification"
```
