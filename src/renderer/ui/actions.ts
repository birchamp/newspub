// src/renderer/ui/actions.ts
// Shared editing actions used by the canvas, toolbar, and menu so every
// entry point goes through the same undo history and store updates.
import { appHistory } from '@history/app-history'
import type { Command } from '@history/commands'
import { useEditorStore } from '@ui/store/editor-store'
import { applyStyle, getRunAtOffset, getPlainText, createThread } from '@model/thread'
import { expandToParagraph } from '@input/selection'
import { generateId } from '@model/document'
import { ZOOM_STOPS } from '@canvas/viewport'
import type { Thread, TextStyle, Frame, Rect, Asset, Document } from '@model/types'

export function makeCommand(type: string, apply: () => void, reverse: () => void, groupId?: string): Command {
  return { type, apply, reverse, groupId, timestamp: Date.now() }
}

export function pushCommand(cmd: Command): void {
  appHistory.push(cmd)
  useEditorStore.getState().bumpHistory()
}

// After undo/redo the selected frame or thread may be gone or shorter —
// drop or clamp the selection so nothing points at stale state.
function reconcileSelection(): void {
  const state = useEditorStore.getState()
  const { document: doc, selection } = state
  if (!doc || !selection) return
  if (selection.type === 'frame') {
    const page = doc.pages.find(p => p.id === selection.pageId)
    if (!page?.frames.some(f => f.id === selection.frameId)) state.setSelection(null)
    return
  }
  const thread = doc.threads[selection.threadId]
  if (!thread) { state.setSelection(null); return }
  const len = thread.runs.reduce((sum, r) => sum + r.text.length, 0)
  if (selection.anchor > len || selection.focus > len) {
    state.setSelection({
      ...selection,
      anchor: Math.min(selection.anchor, len),
      focus: Math.min(selection.focus, len)
    })
  }
}

export function undo(): void {
  appHistory.undo()
  reconcileSelection()
  useEditorStore.getState().bumpHistory()
}

export function redo(): void {
  appHistory.redo()
  reconcileSelection()
  useEditorStore.getState().bumpHistory()
}

export function canUndo(): boolean { return appHistory.canUndo() }
export function canRedo(): boolean { return appHistory.canRedo() }

function setThread(threadId: string, thread: Thread): void {
  useEditorStore.getState().updateDocument(dd => ({
    ...dd,
    threads: { ...dd.threads, [threadId]: thread }
  }))
}

/** Is the given style flag active at the current text selection? */
export function selectionStyleFlag(flag: 'bold' | 'italic'): boolean {
  const { document: doc, selection } = useEditorStore.getState()
  if (!doc || selection?.type !== 'text') return false
  const thread = doc.threads[selection.threadId]
  if (!thread || thread.runs.length === 0) return false
  const start = Math.min(selection.anchor, selection.focus)
  const probe = start === Math.max(selection.anchor, selection.focus) && start > 0 ? start - 1 : start
  const { runIndex } = getRunAtOffset(thread, probe)
  return !!thread.runs[runIndex]?.style[flag]
}

/** Toggle bold/italic over the current text selection (undoable) */
export function toggleStyleFlag(flag: 'bold' | 'italic'): void {
  const { document: doc, selection } = useEditorStore.getState()
  if (!doc || selection?.type !== 'text') return
  const thread = doc.threads[selection.threadId]
  if (!thread) return
  const start = Math.min(selection.anchor, selection.focus)
  const end = Math.max(selection.anchor, selection.focus)
  if (start === end) return

  const current = selectionStyleFlag(flag)
  const before = thread
  const after = applyStyle(thread, start, end, { [flag]: !current })
  const threadId = thread.id

  setThread(threadId, after)
  pushCommand(makeCommand(
    `toggle-${flag}`,
    () => setThread(threadId, after),
    () => setThread(threadId, before)
  ))
}

/** Current value of a text style property at the selection start (for toolbar display) */
export function selectionStyleValue<K extends keyof TextStyle>(key: K): TextStyle[K] | undefined {
  const { document: doc, selection } = useEditorStore.getState()
  if (!doc || selection?.type !== 'text') return undefined
  const thread = doc.threads[selection.threadId]
  if (!thread || thread.runs.length === 0) return thread?.defaultStyle[key]
  const start = Math.min(selection.anchor, selection.focus)
  const probe = start === Math.max(selection.anchor, selection.focus) && start > 0 ? start - 1 : start
  const { runIndex } = getRunAtOffset(thread, probe)
  return thread.runs[runIndex]?.style[key] ?? thread.defaultStyle[key]
}

/**
 * Apply style properties (fontSize, fontFamily, color…) over the current
 * text selection. With a collapsed caret, applies to the whole thread — the
 * common case when styling a headline frame before/after typing.
 */
export function applyTextStyle(style: Partial<TextStyle>): void {
  const { document: doc, selection } = useEditorStore.getState()
  if (!doc || selection?.type !== 'text') return
  const thread = doc.threads[selection.threadId]
  if (!thread) return
  let start = Math.min(selection.anchor, selection.focus)
  let end = Math.max(selection.anchor, selection.focus)
  if (start === end) {
    start = 0
    end = getPlainText(thread).length
  }
  const before = thread
  const after: Thread = {
    ...(end > start ? applyStyle(thread, start, end, style) : thread),
    defaultStyle: { ...thread.defaultStyle, ...style }
  }
  const threadId = thread.id
  setThread(threadId, after)
  pushCommand(makeCommand(
    'apply-text-style',
    () => setThread(threadId, after),
    () => setThread(threadId, before)
  ))
}

/** Set paragraph alignment for the paragraph(s) covered by the selection */
export function setAlignment(alignment: 'left' | 'center' | 'right'): void {
  const { document: doc, selection } = useEditorStore.getState()
  if (!doc || selection?.type !== 'text') return
  const thread = doc.threads[selection.threadId]
  if (!thread) return
  const text = getPlainText(thread)
  const selStart = Math.min(selection.anchor, selection.focus)
  const selEnd = Math.max(selection.anchor, selection.focus)
  const start = expandToParagraph(text, Math.min(selStart, text.length)).start
  const end = expandToParagraph(text, Math.min(selEnd, text.length)).end

  const before = thread
  const after: Thread = {
    ...(end > start ? applyStyle(thread, start, end, { alignment }) : thread),
    defaultStyle: { ...thread.defaultStyle, alignment }
  }
  const threadId = thread.id
  setThread(threadId, after)
  pushCommand(makeCommand(
    'set-alignment',
    () => setThread(threadId, after),
    () => setThread(threadId, before)
  ))
}

// ---- Frame operations ----

function findFrame(doc: Document, pageId: string, frameId: string): Frame | null {
  return doc.pages.find(p => p.id === pageId)?.frames.find(f => f.id === frameId) ?? null
}

/** Set a frame's rect from the properties panel (coalesced per field burst) */
const geometryBurst: { key: string; groupId: string; last: number } = { key: '', groupId: '', last: 0 }
export function setFrameGeometry(pageId: string, frameId: string, patch: Partial<Rect>, burstKey: string): void {
  const state = useEditorStore.getState()
  if (!state.document) return
  const frame = findFrame(state.document, pageId, frameId)
  if (!frame) return
  const before = { ...frame.rect }
  const after = { ...frame.rect, ...patch }
  if (after.width < 8) after.width = 8
  if (after.height < 8) after.height = 8

  const setRect = (rect: Rect) => useEditorStore.getState().updateDocument(dd => ({
    ...dd,
    pages: dd.pages.map(p =>
      p.id !== pageId ? p : { ...p, frames: p.frames.map(f => f.id !== frameId ? f : { ...f, rect: { ...rect } }) }
    )
  }))
  setRect(after)

  const now = Date.now()
  const key = `${frameId}:${burstKey}`
  if (geometryBurst.key !== key || now - geometryBurst.last > 1500) {
    geometryBurst.key = key
    geometryBurst.groupId = generateId('geom')
  }
  geometryBurst.last = now
  pushCommand(makeCommand('set-geometry', () => setRect(after), () => setRect(before), geometryBurst.groupId))
}

/** Duplicate a frame slightly offset; text frames get their own copied thread */
export function duplicateFrame(pageId: string, frameId: string): void {
  const state = useEditorStore.getState()
  if (!state.document) return
  const frame = findFrame(state.document, pageId, frameId)
  if (!frame) return

  const rect = { ...frame.rect, x: frame.rect.x + 12, y: frame.rect.y + 12 }
  let newFrame: Frame
  let newThread: Thread | null = null
  if (frame.type === 'text') {
    const src = state.document.threads[frame.threadId]
    newThread = {
      ...(src ?? createThread({})),
      id: generateId('thread'),
      runs: src ? src.runs.map(r => ({ text: r.text, style: { ...r.style } })) : []
    }
    newFrame = { ...frame, id: generateId('frame'), rect, threadId: newThread.id, threadOrder: 0 }
  } else {
    newFrame = { ...frame, id: generateId('frame'), rect }
  }

  const thread = newThread
  const add = () => useEditorStore.getState().updateDocument(dd => ({
    ...dd,
    threads: thread ? { ...dd.threads, [thread.id]: thread } : dd.threads,
    pages: dd.pages.map(p => p.id !== pageId ? p : { ...p, frames: [...p.frames, newFrame] })
  }))
  const remove = () => useEditorStore.getState().updateDocument(dd => {
    const threads = { ...dd.threads }
    if (thread) delete threads[thread.id]
    return {
      ...dd,
      threads,
      pages: dd.pages.map(p => p.id !== pageId ? p : { ...p, frames: p.frames.filter(f => f.id !== newFrame.id) })
    }
  })
  add()
  useEditorStore.getState().setSelection({ type: 'frame', frameId: newFrame.id, pageId })
  pushCommand(makeCommand('duplicate-frame', add, remove))
}

/** Move a frame to the front or back of its page's stacking order */
export function reorderFrame(pageId: string, frameId: string, where: 'front' | 'back'): void {
  const state = useEditorStore.getState()
  if (!state.document) return
  const page = state.document.pages.find(p => p.id === pageId)
  const index = page?.frames.findIndex(f => f.id === frameId) ?? -1
  if (!page || index === -1) return
  if ((where === 'front' && index === page.frames.length - 1) || (where === 'back' && index === 0)) return

  const move = (toWhere: 'front' | 'back' | number) => useEditorStore.getState().updateDocument(dd => ({
    ...dd,
    pages: dd.pages.map(p => {
      if (p.id !== pageId) return p
      const frames = [...p.frames]
      const i = frames.findIndex(f => f.id === frameId)
      if (i === -1) return p
      const [f] = frames.splice(i, 1)
      if (toWhere === 'front') frames.push(f)
      else if (toWhere === 'back') frames.unshift(f)
      else frames.splice(toWhere, 0, f)
      return { ...p, frames }
    })
  }))
  move(where)
  pushCommand(makeCommand('reorder-frame', () => move(where), () => move(index)))
}

/** Delete a frame (removing its thread if no other frame uses it) */
export function deleteFrame(pageId: string, frameId: string): void {
  const state = useEditorStore.getState()
  const doc = state.document
  if (!doc) return
  const page = doc.pages.find(p => p.id === pageId)
  const frameIndex = page?.frames.findIndex(f => f.id === frameId) ?? -1
  if (!page || frameIndex === -1) return
  const frame = page.frames[frameIndex]

  let orphanThread: Thread | null = null
  if (frame.type === 'text') {
    const others = doc.pages.some(p =>
      p.frames.some(f => f.type === 'text' && f.threadId === frame.threadId && f.id !== frame.id)
    )
    if (!others) orphanThread = doc.threads[frame.threadId] ?? null
  }

  const remove = () => useEditorStore.getState().updateDocument(dd => {
    const threads = { ...dd.threads }
    if (orphanThread) delete threads[orphanThread.id]
    return {
      ...dd,
      threads,
      pages: dd.pages.map(p =>
        p.id !== pageId ? p : { ...p, frames: p.frames.filter(f => f.id !== frame.id) }
      )
    }
  })
  const restore = () => useEditorStore.getState().updateDocument(dd => ({
    ...dd,
    threads: orphanThread ? { ...dd.threads, [orphanThread.id]: orphanThread } : dd.threads,
    pages: dd.pages.map(p => {
      if (p.id !== pageId) return p
      const frames = [...p.frames]
      frames.splice(Math.min(frameIndex, frames.length), 0, frame)
      return { ...p, frames }
    })
  }))

  remove()
  state.setSelection(null)
  pushCommand(makeCommand('delete-frame', remove, restore))
}

// ---- Image placement ----

const IMAGE_MIME: Record<string, Asset['mimeType']> = {
  jpg: 'image/jpeg', jpeg: 'image/jpeg', png: 'image/png', webp: 'image/webp'
}

export function assetFromFile(filename: string, data: ArrayBuffer): Asset | null {
  const ext = filename.split('.').pop()?.toLowerCase() ?? ''
  const mimeType = IMAGE_MIME[ext]
  if (!mimeType) return null
  return { id: generateId('asset'), filename, mimeType, data }
}

/** Put an image asset into an existing image frame (undoable) */
export function placeImageInFrame(pageId: string, frameId: string, asset: Asset): void {
  const state = useEditorStore.getState()
  if (!state.document) return
  const frame = findFrame(state.document, pageId, frameId)
  if (!frame || frame.type !== 'image') return
  const prevAssetId = frame.imageAssetId

  const setAsset = (assetId: string | null, addAsset: Asset | null) =>
    useEditorStore.getState().updateDocument(dd => ({
      ...dd,
      assets: addAsset ? { ...dd.assets, [addAsset.id]: addAsset } : dd.assets,
      pages: dd.pages.map(p =>
        p.id !== pageId ? p : {
          ...p,
          frames: p.frames.map(f => f.id !== frameId || f.type !== 'image' ? f : { ...f, imageAssetId: assetId })
        }
      )
    }))

  setAsset(asset.id, asset)
  pushCommand(makeCommand(
    'place-image',
    () => setAsset(asset.id, asset),
    () => setAsset(prevAssetId, null)
  ))
}

/** Open a file picker and place the chosen image into the frame */
export async function chooseImageForFrame(pageId: string, frameId: string): Promise<void> {
  const api = (window as any).electronAPI
  if (!api?.showOpenDialog) return
  const result = await api.showOpenDialog({
    filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp'] }],
    properties: ['openFile']
  })
  if (result.canceled || !result.filePaths?.length) return
  const path = result.filePaths[0]
  const data: ArrayBuffer = await api.openFile(path)
  const filename = path.split(/[\\/]/).pop() ?? 'image'
  const asset = assetFromFile(filename, data)
  if (asset) placeImageInFrame(pageId, frameId, asset)
}

/** Set an image frame's fit or wrap mode (undoable) */
export function setImageFrameOption(
  pageId: string,
  frameId: string,
  patch: Partial<{ imageFit: 'fill' | 'fit' | 'stretch'; wrapMode: 'skip' | 'rect' }>
): void {
  const state = useEditorStore.getState()
  if (!state.document) return
  const frame = findFrame(state.document, pageId, frameId)
  if (!frame || frame.type !== 'image') return
  const before = { imageFit: frame.imageFit, wrapMode: frame.wrapMode }
  const apply = (values: typeof before) => useEditorStore.getState().updateDocument(dd => ({
    ...dd,
    pages: dd.pages.map(p =>
      p.id !== pageId ? p : {
        ...p,
        frames: p.frames.map(f => f.id !== frameId || f.type !== 'image' ? f : { ...f, ...values })
      }
    )
  }))
  const after = { ...before, ...patch }
  apply(after)
  pushCommand(makeCommand('image-frame-option', () => apply(after), () => apply(before)))
}

/** Step zoom to the next/previous preset stop, anchored at viewport center */
export function zoomStep(direction: 1 | -1): void {
  const { zoom, zoomAtCenter } = useEditorStore.getState()
  const stops = ZOOM_STOPS
  let target: number | undefined
  if (direction === 1) {
    target = stops.find(s => s > zoom + 0.001)
  } else {
    target = [...stops].reverse().find(s => s < zoom - 0.001)
  }
  if (target !== undefined) zoomAtCenter(target)
}
