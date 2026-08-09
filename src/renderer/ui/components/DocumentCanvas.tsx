// src/renderer/ui/components/DocumentCanvas.tsx
import { useRef, useEffect, useCallback, useState, useMemo } from 'react'
import { useEditorStore, type Selection } from '@ui/store/editor-store'
import { render } from '@canvas/canvas-renderer'
import { layoutDocument } from '@engine/layout-engine'
import {
  screenToDoc, applyCamera, resetTransform, zoomAt, clampPan,
  getContentBounds, fitWidthCamera, getSpreadPositions,
  type Camera, type SpreadPosition
} from '@canvas/viewport'
import { hitTestFrames, hitTestResizeHandle, type HandlePosition } from '@input/hit-test'
import { measureLinePrefix, characterIndexAtX } from '@canvas/text-painter'
import { paintCursor } from '@canvas/cursor-painter'
import { insertText, deleteText, applyStyle, getRunAtOffset, getPlainText, createThread, splitRunAtOffset } from '@model/thread'
import { generateId } from '@model/document'
import { expandToWord } from '@input/selection'
import { parseHtmlToRuns, runsToHtml, runsToPlainText } from '@input/clipboard'
import { pushCommand, makeCommand, undo, redo, toggleStyleFlag, zoomStep } from '@ui/actions'
import type { LayoutLine, DocumentLayout } from '@engine/layout-types'
import type { Document, Frame, Page, Point, Rect, Thread, TextFrame } from '@model/types'

const MIN_FRAME_SIZE = 8
const FRAME_PADDING = 8
const DEFAULT_LINE_HEIGHT = 20
const TYPING_COALESCE_MS = 900

// Interaction state tracked during a mouse drag.
type DragState =
  | null
  | { type: 'move-frame'; frameId: string; pageId: string; startX: number; startY: number; origRect: Rect; groupId: string; moved: boolean }
  | { type: 'resize-frame'; frameId: string; pageId: string; handle: HandlePosition; startX: number; startY: number; origRect: Rect; groupId: string }
  | { type: 'draw-frame'; startX: number; startY: number; currentX: number; currentY: number }
  | { type: 'text-select'; threadId: string; frameId: string; pageId: string; anchor: number }
  | { type: 'pan'; startX: number; startY: number; origPanX: number; origPanY: number }

function resizeRect(orig: Rect, handle: HandlePosition, dx: number, dy: number): Rect {
  let { x, y, width, height } = orig
  if (handle === 'left' || handle === 'top-left' || handle === 'bottom-left') {
    x = orig.x + dx
    width = orig.width - dx
  }
  if (handle === 'right' || handle === 'top-right' || handle === 'bottom-right') {
    width = orig.width + dx
  }
  if (handle === 'top' || handle === 'top-left' || handle === 'top-right') {
    y = orig.y + dy
    height = orig.height - dy
  }
  if (handle === 'bottom' || handle === 'bottom-left' || handle === 'bottom-right') {
    height = orig.height + dy
  }
  if (width < MIN_FRAME_SIZE) {
    if (x !== orig.x) x = orig.x + orig.width - MIN_FRAME_SIZE
    width = MIN_FRAME_SIZE
  }
  if (height < MIN_FRAME_SIZE) {
    if (y !== orig.y) y = orig.y + orig.height - MIN_FRAME_SIZE
    height = MIN_FRAME_SIZE
  }
  return { x, y, width, height }
}

const RESIZE_CURSORS: Record<HandlePosition, string> = {
  'top-left': 'nwse-resize', 'bottom-right': 'nwse-resize',
  'top-right': 'nesw-resize', 'bottom-left': 'nesw-resize',
  left: 'ew-resize', right: 'ew-resize',
  top: 'ns-resize', bottom: 'ns-resize'
}

function setFrameRect(doc: Document, pageId: string, frameId: string, rect: Rect): Document {
  return {
    ...doc,
    pages: doc.pages.map(page =>
      page.id !== pageId
        ? page
        : { ...page, frames: page.frames.map(f => (f.id !== frameId ? f : { ...f, rect: { ...rect } })) }
    )
  }
}

function findSpreadForPage(doc: Document, pageId: string, spreadPositions: SpreadPosition[]): SpreadPosition | null {
  const pageIndex = doc.pages.findIndex(p => p.id === pageId)
  if (pageIndex === -1) return null
  return spreadPositions.find(sp => sp.pageIndex === pageIndex) ?? null
}

/** Which page does a document-space point fall on? */
function findPageAtPoint(doc: Document, docPoint: Point, spreadPositions: SpreadPosition[]): { page: Page; spread: SpreadPosition } | null {
  const { width, height } = doc.metadata.pageSize
  for (const sp of spreadPositions) {
    if (
      docPoint.x >= sp.x && docPoint.x <= sp.x + width &&
      docPoint.y >= sp.y && docPoint.y <= sp.y + height
    ) {
      const page = doc.pages[sp.pageIndex]
      if (page) return { page, spread: sp }
    }
  }
  return null
}

// ---- Thread text geometry (caret <-> point mapping) ----

interface ThreadFrameGeo {
  frameId: string
  pageId: string
  spread: SpreadPosition
  rect: Rect
  lines: LayoutLine[]
  startOffset: number
  endOffset: number
}

function getThreadGeometry(
  doc: Document,
  layout: DocumentLayout,
  threadId: string,
  spreadPositions: SpreadPosition[]
): ThreadFrameGeo[] {
  const threadLayout = layout.threadLayouts[threadId]
  const geos: ThreadFrameGeo[] = []
  let offset = 0
  if (threadLayout) {
    for (const fl of threadLayout.frameLayouts) {
      const spread = findSpreadForPage(doc, fl.pageId, spreadPositions)
      const page = doc.pages.find(p => p.id === fl.pageId)
      const frame = page?.frames.find(f => f.id === fl.frameId)
      if (!spread || !frame) continue
      geos.push({
        frameId: fl.frameId,
        pageId: fl.pageId,
        spread,
        rect: frame.rect,
        lines: fl.lines,
        startOffset: offset,
        endOffset: fl.threadCursorEnd
      })
      offset = fl.threadCursorEnd
    }
  }
  if (geos.length === 0) {
    // Empty thread — synthesize geometry from its first frame so the caret
    // still has a home.
    for (const page of doc.pages) {
      for (const f of page.frames) {
        if (f.type === 'text' && f.threadId === threadId) {
          const spread = findSpreadForPage(doc, page.id, spreadPositions)
          if (spread) {
            geos.push({
              frameId: f.id, pageId: page.id, spread, rect: f.rect,
              lines: [], startOffset: 0, endOffset: 0
            })
          }
          return geos
        }
      }
    }
  }
  return geos
}

interface CaretRect { x: number; y: number; height: number; geo: ThreadFrameGeo; lineIndex: number }

function caretRectForOffset(ctx: CanvasRenderingContext2D, geos: ThreadFrameGeo[], offset: number): CaretRect | null {
  if (geos.length === 0) return null
  let target = geos.find(g => offset >= g.startOffset && offset <= g.endOffset) ?? geos[geos.length - 1]
  if (target.lines.length === 0) {
    return {
      x: target.spread.x + target.rect.x + FRAME_PADDING,
      y: target.spread.y + target.rect.y + FRAME_PADDING,
      height: DEFAULT_LINE_HEIGHT,
      geo: target,
      lineIndex: 0
    }
  }
  let lineStart = target.startOffset
  for (let i = 0; i < target.lines.length; i++) {
    const line = target.lines[i]
    const len = line.text.length
    const isLast = i === target.lines.length - 1
    if (offset <= lineStart + len - (isLast ? 0 : 1) || (isLast && offset >= lineStart)) {
      const local = Math.max(0, Math.min(offset - lineStart, len))
      const x = measureLinePrefix(ctx, line, local)
      return {
        x: target.spread.x + target.rect.x + line.x + x,
        y: target.spread.y + target.rect.y + line.y,
        height: line.height,
        geo: target,
        lineIndex: i
      }
    }
    lineStart += len
  }
  const last = target.lines[target.lines.length - 1]
  return {
    x: target.spread.x + target.rect.x + last.x + measureLinePrefix(ctx, last, last.text.length),
    y: target.spread.y + target.rect.y + last.y,
    height: last.height,
    geo: target,
    lineIndex: target.lines.length - 1
  }
}

function caretOffsetForPoint(ctx: CanvasRenderingContext2D, geos: ThreadFrameGeo[], docPoint: Point): number {
  if (geos.length === 0) return 0
  // Prefer the frame containing the point; otherwise the nearest frame.
  let target = geos.find(g =>
    docPoint.x >= g.spread.x + g.rect.x && docPoint.x <= g.spread.x + g.rect.x + g.rect.width &&
    docPoint.y >= g.spread.y + g.rect.y && docPoint.y <= g.spread.y + g.rect.y + g.rect.height
  )
  if (!target) {
    let best = Infinity
    for (const g of geos) {
      const cx = g.spread.x + g.rect.x + g.rect.width / 2
      const cy = g.spread.y + g.rect.y + g.rect.height / 2
      const d = (docPoint.x - cx) ** 2 + (docPoint.y - cy) ** 2
      if (d < best) { best = d; target = g }
    }
  }
  if (!target || target.lines.length === 0) return target?.startOffset ?? 0

  const relY = docPoint.y - target.spread.y - target.rect.y
  let lineStart = target.startOffset
  for (let i = 0; i < target.lines.length; i++) {
    const line = target.lines[i]
    const isLast = i === target.lines.length - 1
    if (relY < line.y + line.height || isLast) {
      if (relY < line.y && i === 0) return lineStart
      const relX = docPoint.x - target.spread.x - target.rect.x - line.x
      return lineStart + characterIndexAtX(ctx, line, relX)
    }
    lineStart += line.text.length
  }
  return target.endOffset
}

export default function DocumentCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const {
    document: doc,
    docEpoch,
    updateDocument,
    selection,
    setSelection,
    zoom,
    panX,
    panY,
    setPan,
    setCamera,
    setViewportSize,
    activeTool,
    setActiveTool,
    fitRequest,
    scrollTarget,
    setCurrentPageIndex
  } = useEditorStore()

  const camera: Camera = { zoom, panX, panY }
  const [drag, setDrag] = useState<DragState>(null)
  const [blinkOn, setBlinkOn] = useState(true)
  const [repaintTick, setRepaintTick] = useState(0)
  const dragRef = useRef<DragState>(null)
  dragRef.current = drag
  const typingRef = useRef<{ groupId: string; last: number; kind: string } | null>(null)
  const nudgeRef = useRef<{ groupId: string; last: number } | null>(null)
  const imagesRef = useRef<Map<string, HTMLImageElement>>(new Map())
  const animRef = useRef<number | null>(null)

  // Layout is pure in the document — compute once per document change.
  const layout = useMemo(() => (doc ? layoutDocument(doc) : null), [doc])
  const spreadPositions = useMemo(
    () => (doc ? getSpreadPositions(doc.pages.length, doc.metadata.pageSize) : []),
    [doc]
  )

  const textSel = selection?.type === 'text' ? selection : null

  const measureCtx = useCallback((): CanvasRenderingContext2D | null => {
    return canvasRef.current?.getContext('2d') ?? null
  }, [])

  const threadGeos = useMemo(() => {
    if (!doc || !layout || !textSel) return []
    return getThreadGeometry(doc, layout, textSel.threadId, spreadPositions)
  }, [doc, layout, textSel, spreadPositions])

  // ---- Image asset cache ----
  useEffect(() => {
    if (!doc) return
    const cache = imagesRef.current
    for (const [id, asset] of Object.entries(doc.assets)) {
      if (cache.has(id)) continue
      const blob = new Blob([asset.data], { type: asset.mimeType })
      const url = URL.createObjectURL(blob)
      const img = new Image()
      img.onload = () => setRepaintTick(t => t + 1)
      img.src = url
      cache.set(id, img)
    }
  }, [doc])

  // ---- Rendering ----
  const renderCanvas = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const parent = canvas.parentElement!
    const width = parent.clientWidth
    const height = parent.clientHeight
    const dpr = window.devicePixelRatio || 1
    if (canvas.width !== Math.round(width * dpr) || canvas.height !== Math.round(height * dpr)) {
      canvas.width = Math.round(width * dpr)
      canvas.height = Math.round(height * dpr)
      canvas.style.width = `${width}px`
      canvas.style.height = `${height}px`
    }
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    if (!doc || !layout) {
      resetTransform(ctx)
      ctx.fillStyle = '#e5e5e5'
      ctx.fillRect(0, 0, canvas.width, canvas.height)
      return
    }

    // Pre-multiply DPR into the camera so everything renders crisply.
    const dprCamera: Camera = { zoom: zoom * dpr, panX: panX * dpr, panY: panY * dpr }

    render({
      doc,
      layout,
      camera: dprCamera,
      canvas,
      selection,
      editMode: true,
      loadedImages: imagesRef.current
    })

    // Text selection highlight + caret
    if (textSel && threadGeos.length > 0) {
      applyCamera(ctx, dprCamera)
      const start = Math.min(textSel.anchor, textSel.focus)
      const end = Math.max(textSel.anchor, textSel.focus)

      if (end > start) {
        ctx.save()
        ctx.fillStyle = 'rgba(99, 102, 241, 0.28)'
        for (const geo of threadGeos) {
          let lineStart = geo.startOffset
          for (const line of geo.lines) {
            const len = line.text.length
            const selStart = Math.max(start, lineStart)
            const selEnd = Math.min(end, lineStart + len)
            if (selEnd > selStart) {
              const x0 = measureLinePrefix(ctx, line, selStart - lineStart)
              const x1 = measureLinePrefix(ctx, line, selEnd - lineStart)
              ctx.fillRect(
                geo.spread.x + geo.rect.x + line.x + x0,
                geo.spread.y + geo.rect.y + line.y,
                x1 - x0,
                line.height
              )
            }
            lineStart += len
          }
        }
        ctx.restore()
      } else if (blinkOn) {
        const caret = caretRectForOffset(ctx, threadGeos, textSel.focus)
        if (caret) paintCursor(ctx, caret.x, caret.y + 2, caret.height - 4)
      }
      resetTransform(ctx)
    }

    // Rubber-band preview while drawing a new frame
    const d = dragRef.current
    if (d?.type === 'draw-frame') {
      applyCamera(ctx, dprCamera)
      const x = Math.min(d.startX, d.currentX)
      const y = Math.min(d.startY, d.currentY)
      const w = Math.abs(d.currentX - d.startX)
      const h = Math.abs(d.currentY - d.startY)
      ctx.save()
      ctx.strokeStyle = '#2680eb'
      ctx.setLineDash([4, 3])
      ctx.lineWidth = 1 / zoom
      ctx.strokeRect(x, y, w, h)
      ctx.restore()
      resetTransform(ctx)
    }
  }, [doc, layout, selection, textSel, threadGeos, zoom, panX, panY, drag, blinkOn, repaintTick])

  useEffect(() => { renderCanvas() }, [renderCanvas])

  // ---- Viewport sizing ----
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const parent = canvas.parentElement!
    const update = () => {
      setViewportSize(parent.clientWidth, parent.clientHeight)
      renderCanvas()
    }
    update()
    const observer = new ResizeObserver(update)
    observer.observe(parent)
    return () => observer.disconnect()
  }, [renderCanvas, setViewportSize])

  // ---- Fit document on load / fit request ----
  useEffect(() => {
    if (!doc) return
    const canvas = canvasRef.current
    if (!canvas) return
    const parent = canvas.parentElement!
    const bounds = getContentBounds(doc.pages.length, doc.metadata.pageSize)
    setCamera(fitWidthCamera(parent.clientWidth, parent.clientHeight, bounds))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [docEpoch, fitRequest])

  // ---- Scroll to page (sidebar click) with a short eased animation ----
  useEffect(() => {
    if (!doc || !scrollTarget) return
    const canvas = canvasRef.current
    if (!canvas) return
    const parent = canvas.parentElement!
    const sp = spreadPositions.find(s => s.pageIndex === scrollTarget.pageIndex)
    if (!sp) return
    const state = useEditorStore.getState()
    const z = state.zoom
    // Center the spread pair horizontally; put its top just below the top edge.
    const partner = spreadPositions.find(s => s.y === sp.y && s.pageIndex !== sp.pageIndex)
    const left = partner ? Math.min(sp.x, partner.x) : sp.x
    const right = partner ? Math.max(sp.x, partner.x) + doc.metadata.pageSize.width : sp.x + doc.metadata.pageSize.width
    const targetPanX = (parent.clientWidth - (right - left) * z) / 2 - left * z
    const targetPanY = 40 - sp.y * z

    const from = { x: state.panX, y: state.panY }
    const startTime = performance.now()
    const DURATION = 200
    if (animRef.current) cancelAnimationFrame(animRef.current)
    const stepAnim = (now: number) => {
      const t = Math.min(1, (now - startTime) / DURATION)
      const ease = 1 - (1 - t) * (1 - t)
      useEditorStore.getState().setPan(
        from.x + (targetPanX - from.x) * ease,
        from.y + (targetPanY - from.y) * ease
      )
      if (t < 1) animRef.current = requestAnimationFrame(stepAnim)
    }
    animRef.current = requestAnimationFrame(stepAnim)
    return () => { if (animRef.current) cancelAnimationFrame(animRef.current) }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scrollTarget?.nonce])

  // ---- Track current page from scroll position ----
  useEffect(() => {
    if (!doc || spreadPositions.length === 0) return
    const canvas = canvasRef.current
    if (!canvas) return
    const parent = canvas.parentElement!
    const centerDocY = (parent.clientHeight / 2 - panY) / zoom
    let best = 0
    let bestDist = Infinity
    for (const sp of spreadPositions) {
      const mid = sp.y + doc.metadata.pageSize.height / 2
      const dist = Math.abs(mid - centerDocY)
      if (dist < bestDist) { bestDist = dist; best = sp.pageIndex }
    }
    if (useEditorStore.getState().currentPageIndex !== best) setCurrentPageIndex(best)
  }, [doc, spreadPositions, panY, zoom, setCurrentPageIndex])

  // ---- Caret blink ----
  useEffect(() => {
    if (!textSel) return
    setBlinkOn(true)
    const interval = setInterval(() => setBlinkOn(b => !b), 530)
    return () => clearInterval(interval)
  }, [textSel?.threadId, textSel?.anchor, textSel?.focus])

  // ---- Hidden textarea focus management ----
  useEffect(() => {
    if (textSel) {
      textareaRef.current?.focus({ preventScroll: true })
      // Keep the textarea near the caret so IME popups appear in place.
      const ctx = measureCtx()
      if (ctx && threadGeos.length > 0) {
        const caret = caretRectForOffset(ctx, threadGeos, textSel.focus)
        const canvas = canvasRef.current
        if (caret && canvas && textareaRef.current) {
          const rect = canvas.getBoundingClientRect()
          const sx = caret.x * zoom + panX + rect.left
          const sy = caret.y * zoom + panY + rect.top
          textareaRef.current.style.left = `${Math.max(0, Math.min(window.innerWidth - 2, sx))}px`
          textareaRef.current.style.top = `${Math.max(0, Math.min(window.innerHeight - 2, sy))}px`
        }
      }
    }
  }, [textSel, threadGeos, zoom, panX, panY, measureCtx])

  const toDocPoint = useCallback((e: { clientX: number; clientY: number }): Point => {
    const canvas = canvasRef.current!
    const rect = canvas.getBoundingClientRect()
    return screenToDoc({ x: e.clientX - rect.left, y: e.clientY - rect.top }, camera)
  }, [camera])

  // ---- Text editing operations ----

  const setThreadWithHistory = useCallback((
    threadId: string,
    before: Thread,
    after: Thread,
    caret: { anchor: number; focus: number },
    coalesceKind: string | null
  ) => {
    if (!textSel && selection?.type !== 'text') { /* caller ensures editing */ }
    const apply = () => useEditorStore.getState().updateDocument(dd => ({
      ...dd, threads: { ...dd.threads, [threadId]: after }
    }))
    const reverse = () => useEditorStore.getState().updateDocument(dd => ({
      ...dd, threads: { ...dd.threads, [threadId]: before }
    }))
    apply()
    const sel = useEditorStore.getState().selection
    if (sel?.type === 'text') {
      setSelection({ ...sel, anchor: caret.anchor, focus: caret.focus })
    }

    let groupId: string | undefined
    const now = Date.now()
    if (coalesceKind) {
      const t = typingRef.current
      if (t && t.kind === coalesceKind && now - t.last < TYPING_COALESCE_MS) {
        groupId = t.groupId
        t.last = now
      } else {
        groupId = generateId('typing')
        typingRef.current = { groupId, last: now, kind: coalesceKind }
      }
    } else {
      typingRef.current = null
    }
    pushCommand(makeCommand('edit-text', apply, reverse, groupId))
  }, [setSelection, selection, textSel])

  const replaceSelectedText = useCallback((text: string, coalesceKind: string | null = 'type') => {
    const state = useEditorStore.getState()
    const sel = state.selection
    if (!state.document || sel?.type !== 'text') return
    const thread = state.document.threads[sel.threadId]
    if (!thread) return
    const start = Math.min(sel.anchor, sel.focus)
    const end = Math.max(sel.anchor, sel.focus)
    let after = thread
    if (end > start) after = deleteText(after, start, end)
    if (text.length > 0) {
      // Continue the style at the insertion point
      let style
      if (after.runs.length > 0) {
        const { runIndex } = getRunAtOffset(after, start)
        style = after.runs[runIndex]?.style
      }
      after = insertText(after, start, text, style)
    }
    const caret = start + text.length
    setThreadWithHistory(sel.threadId, thread, after, { anchor: caret, focus: caret }, coalesceKind)
  }, [setThreadWithHistory])

  const deleteDirection = useCallback((dir: -1 | 1) => {
    const state = useEditorStore.getState()
    const sel = state.selection
    if (!state.document || sel?.type !== 'text') return
    const thread = state.document.threads[sel.threadId]
    if (!thread) return
    const len = getPlainText(thread).length
    let start = Math.min(sel.anchor, sel.focus)
    let end = Math.max(sel.anchor, sel.focus)
    if (start === end) {
      if (dir === -1) { if (start === 0) return; start -= 1 }
      else { if (end >= len) return; end += 1 }
    }
    const after = deleteText(thread, start, end)
    setThreadWithHistory(sel.threadId, thread, after, { anchor: start, focus: start }, 'delete')
  }, [setThreadWithHistory])

  const pasteRuns = useCallback((runs: ReturnType<typeof parseHtmlToRuns>) => {
    const state = useEditorStore.getState()
    const sel = state.selection
    if (!state.document || sel?.type !== 'text') return
    const thread = state.document.threads[sel.threadId]
    if (!thread) return
    const start = Math.min(sel.anchor, sel.focus)
    const end = Math.max(sel.anchor, sel.focus)
    let after = thread
    if (end > start) after = deleteText(after, start, end)
    let pos = start
    for (const run of runs) {
      after = insertText(after, pos, run.text, run.style)
      pos += run.text.length
    }
    setThreadWithHistory(sel.threadId, thread, after, { anchor: pos, focus: pos }, null)
  }, [setThreadWithHistory])

  const moveCaret = useCallback((
    move: (offset: number, thread: Thread) => number,
    extend: boolean
  ) => {
    const state = useEditorStore.getState()
    const sel = state.selection
    if (!state.document || sel?.type !== 'text') return
    const thread = state.document.threads[sel.threadId]
    if (!thread) return
    const len = getPlainText(thread).length
    const next = Math.max(0, Math.min(len, move(sel.focus, thread)))
    if (extend) {
      setSelection({ ...sel, focus: next })
    } else {
      setSelection({ ...sel, anchor: next, focus: next })
    }
  }, [setSelection])

  const verticalCaretMove = useCallback((dir: -1 | 1, extend: boolean) => {
    const ctx = measureCtx()
    if (!ctx || threadGeos.length === 0) return
    moveCaret((offset) => {
      const caret = caretRectForOffset(ctx, threadGeos, offset)
      if (!caret) return offset
      const targetY = caret.y + (dir === 1 ? caret.height * 1.5 : -caret.height * 0.5)
      return caretOffsetForPoint(ctx, threadGeos, { x: caret.x, y: targetY })
    }, extend)
  }, [measureCtx, threadGeos, moveCaret])

  const exitTextEditing = useCallback(() => {
    const sel = useEditorStore.getState().selection
    if (sel?.type === 'text') {
      setSelection({ type: 'frame', frameId: sel.frameId, pageId: sel.pageId })
      typingRef.current = null
      textareaRef.current?.blur()
    }
  }, [setSelection])

  const enterTextEditing = useCallback((frame: TextFrame, pageId: string, docPoint: Point | null, selectWord: boolean) => {
    if (!doc || !layout) return
    const ctx = measureCtx()
    const geos = getThreadGeometry(doc, layout, frame.threadId, spreadPositions)
    let offset = 0
    if (ctx && docPoint) offset = caretOffsetForPoint(ctx, geos, docPoint)
    let anchor = offset
    let focus = offset
    if (selectWord) {
      const thread = doc.threads[frame.threadId]
      if (thread) {
        const { start, end } = expandToWord(getPlainText(thread), offset)
        anchor = start
        focus = end
      }
    }
    setSelection({ type: 'text', threadId: frame.threadId, frameId: frame.id, pageId, anchor, focus })
  }, [doc, layout, spreadPositions, measureCtx, setSelection])

  // ---- Textarea events (typing, clipboard, IME) ----

  const handleTextareaInput = useCallback((e: React.FormEvent<HTMLTextAreaElement>) => {
    const native = e.nativeEvent as InputEvent
    if (native.isComposing) return
    const text = textareaRef.current?.value ?? ''
    if (text.length === 0) return
    replaceSelectedText(text)
    if (textareaRef.current) textareaRef.current.value = ''
  }, [replaceSelectedText])

  const handleCompositionEnd = useCallback((e: React.CompositionEvent) => {
    if (e.data) replaceSelectedText(e.data)
    if (textareaRef.current) textareaRef.current.value = ''
  }, [replaceSelectedText])

  const selectedRunsSlice = useCallback(() => {
    const state = useEditorStore.getState()
    const sel = state.selection
    if (!state.document || sel?.type !== 'text') return null
    const thread = state.document.threads[sel.threadId]
    if (!thread) return null
    const start = Math.min(sel.anchor, sel.focus)
    const end = Math.max(sel.anchor, sel.focus)
    if (start === end) return null
    const { after: tail } = splitRunAtOffset(thread.runs, start)
    const { before: middle } = splitRunAtOffset(tail, end - start)
    return middle
  }, [])

  const handleCopy = useCallback((e: React.ClipboardEvent) => {
    const runs = selectedRunsSlice()
    if (!runs) return
    e.preventDefault()
    e.clipboardData.setData('text/plain', runsToPlainText(runs))
    e.clipboardData.setData('text/html', runsToHtml(runs))
  }, [selectedRunsSlice])

  const handleCut = useCallback((e: React.ClipboardEvent) => {
    const runs = selectedRunsSlice()
    if (!runs) return
    e.preventDefault()
    e.clipboardData.setData('text/plain', runsToPlainText(runs))
    e.clipboardData.setData('text/html', runsToHtml(runs))
    replaceSelectedText('', null)
  }, [selectedRunsSlice, replaceSelectedText])

  const handlePaste = useCallback((e: React.ClipboardEvent) => {
    e.preventDefault()
    const html = e.clipboardData.getData('text/html')
    const plain = e.clipboardData.getData('text/plain')
    if (html) {
      pasteRuns(parseHtmlToRuns(html))
    } else if (plain) {
      replaceSelectedText(plain, null)
    }
  }, [pasteRuns, replaceSelectedText])

  const handleTextareaKeyDown = useCallback((e: React.KeyboardEvent) => {
    const mod = e.metaKey || e.ctrlKey
    switch (e.key) {
      case 'Backspace':
        e.preventDefault()
        deleteDirection(-1)
        return
      case 'Delete':
        e.preventDefault()
        deleteDirection(1)
        return
      case 'Enter':
        e.preventDefault()
        replaceSelectedText('\n', null)
        return
      case 'Escape':
        e.preventDefault()
        exitTextEditing()
        return
      case 'ArrowLeft':
        e.preventDefault()
        moveCaret(o => {
          const sel = useEditorStore.getState().selection
          if (!e.shiftKey && sel?.type === 'text' && sel.anchor !== sel.focus) {
            return Math.min(sel.anchor, sel.focus)
          }
          return o - 1
        }, e.shiftKey)
        return
      case 'ArrowRight':
        e.preventDefault()
        moveCaret(o => {
          const sel = useEditorStore.getState().selection
          if (!e.shiftKey && sel?.type === 'text' && sel.anchor !== sel.focus) {
            return Math.max(sel.anchor, sel.focus)
          }
          return o + 1
        }, e.shiftKey)
        return
      case 'ArrowUp':
        e.preventDefault()
        verticalCaretMove(-1, e.shiftKey)
        return
      case 'ArrowDown':
        e.preventDefault()
        verticalCaretMove(1, e.shiftKey)
        return
      case 'Home':
        e.preventDefault()
        moveCaret((o) => {
          const ctx = measureCtx()
          if (!ctx) return 0
          const caret = caretRectForOffset(ctx, threadGeos, o)
          if (!caret) return 0
          let lineStart = caret.geo.startOffset
          for (let i = 0; i < caret.lineIndex; i++) lineStart += caret.geo.lines[i].text.length
          return lineStart
        }, e.shiftKey)
        return
      case 'End':
        e.preventDefault()
        moveCaret((o, thread) => {
          const ctx = measureCtx()
          if (!ctx) return getPlainText(thread).length
          const caret = caretRectForOffset(ctx, threadGeos, o)
          if (!caret) return getPlainText(thread).length
          let lineEnd = caret.geo.startOffset
          for (let i = 0; i <= caret.lineIndex; i++) lineEnd += caret.geo.lines[i]?.text.length ?? 0
          return lineEnd
        }, e.shiftKey)
        return
      case 'a':
        if (mod) {
          e.preventDefault()
          moveCaret((_, thread) => getPlainText(thread).length, false)
          const sel = useEditorStore.getState().selection
          if (sel?.type === 'text') setSelection({ ...sel, anchor: 0 })
        }
        return
      case 'Tab':
        e.preventDefault()
        return
    }
  }, [deleteDirection, replaceSelectedText, exitTextEditing, moveCaret, verticalCaretMove, measureCtx, threadGeos, setSelection])

  // ---- Mouse interaction ----

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!doc) return

    // Middle-button drag pans the canvas
    if (e.button === 1) {
      e.preventDefault()
      setDrag({ type: 'pan', startX: e.clientX, startY: e.clientY, origPanX: panX, origPanY: panY })
      return
    }
    if (e.button !== 0) return

    const docPoint = toDocPoint(e)

    // Draw new frame: rubber-band start
    if (activeTool === 'draw-text-frame' || activeTool === 'draw-image-frame') {
      setDrag({
        type: 'draw-frame',
        startX: docPoint.x,
        startY: docPoint.y,
        currentX: docPoint.x,
        currentY: docPoint.y
      })
      return
    }

    // Editing text: click inside the thread's frames moves the caret
    if (textSel) {
      const inThread = threadGeos.some(g =>
        docPoint.x >= g.spread.x + g.rect.x && docPoint.x <= g.spread.x + g.rect.x + g.rect.width &&
        docPoint.y >= g.spread.y + g.rect.y && docPoint.y <= g.spread.y + g.rect.y + g.rect.height
      )
      if (inThread) {
        const ctx = measureCtx()
        if (ctx) {
          const offset = caretOffsetForPoint(ctx, threadGeos, docPoint)
          if (e.shiftKey) {
            setSelection({ ...textSel, focus: offset })
          } else {
            setSelection({ ...textSel, anchor: offset, focus: offset })
            setDrag({ type: 'text-select', threadId: textSel.threadId, frameId: textSel.frameId, pageId: textSel.pageId, anchor: offset })
          }
        }
        return
      }
      // Clicked outside the edited thread — leave editing and fall through.
      exitTextEditing()
    }

    // Resize handle hit-test on the currently selected frame
    if (selection?.type === 'frame') {
      const spread = findSpreadForPage(doc, selection.pageId, spreadPositions)
      const page = doc.pages.find(p => p.id === selection.pageId)
      const frame = page?.frames.find(f => f.id === selection.frameId)
      if (spread && frame) {
        const localPoint: Point = { x: docPoint.x - spread.x, y: docPoint.y - spread.y }
        const handle = hitTestResizeHandle(localPoint, frame.rect, 6 / camera.zoom)
        if (handle) {
          setDrag({
            type: 'resize-frame',
            frameId: frame.id,
            pageId: selection.pageId,
            handle,
            startX: docPoint.x,
            startY: docPoint.y,
            origRect: { ...frame.rect },
            groupId: generateId('drag')
          })
          return
        }
      }
    }

    const hit = hitTestFrames(docPoint, doc.pages, spreadPositions)
    if (hit) {
      setSelection({ type: 'frame', frameId: hit.frame.id, pageId: hit.pageId })
      setDrag({
        type: 'move-frame',
        frameId: hit.frame.id,
        pageId: hit.pageId,
        startX: docPoint.x,
        startY: docPoint.y,
        origRect: { ...hit.frame.rect },
        groupId: generateId('drag'),
        moved: false
      })
    } else {
      setSelection(null)
    }
  }, [doc, camera, panX, panY, activeTool, selection, textSel, threadGeos, spreadPositions, setSelection, toDocPoint, measureCtx, exitTextEditing])

  const updateHoverCursor = useCallback((e: React.MouseEvent) => {
    const canvas = canvasRef.current
    if (!canvas || !doc) return
    if (activeTool !== 'select') {
      canvas.style.cursor = 'crosshair'
      return
    }
    const docPoint = toDocPoint(e)

    if (textSel) {
      const inThread = threadGeos.some(g =>
        docPoint.x >= g.spread.x + g.rect.x && docPoint.x <= g.spread.x + g.rect.x + g.rect.width &&
        docPoint.y >= g.spread.y + g.rect.y && docPoint.y <= g.spread.y + g.rect.y + g.rect.height
      )
      if (inThread) { canvas.style.cursor = 'text'; return }
    }

    if (selection?.type === 'frame') {
      const spread = findSpreadForPage(doc, selection.pageId, spreadPositions)
      const page = doc.pages.find(p => p.id === selection.pageId)
      const frame = page?.frames.find(f => f.id === selection.frameId)
      if (spread && frame) {
        const localPoint: Point = { x: docPoint.x - spread.x, y: docPoint.y - spread.y }
        const handle = hitTestResizeHandle(localPoint, frame.rect, 6 / camera.zoom)
        if (handle) { canvas.style.cursor = RESIZE_CURSORS[handle]; return }
        if (
          localPoint.x >= frame.rect.x && localPoint.x <= frame.rect.x + frame.rect.width &&
          localPoint.y >= frame.rect.y && localPoint.y <= frame.rect.y + frame.rect.height
        ) { canvas.style.cursor = 'move'; return }
      }
    }
    canvas.style.cursor = 'default'
  }, [doc, activeTool, selection, textSel, threadGeos, spreadPositions, camera, toDocPoint])

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const d = dragRef.current
    if (!d) {
      updateHoverCursor(e)
      return
    }
    if (!doc) return

    if (d.type === 'pan') {
      setPan(d.origPanX + (e.clientX - d.startX), d.origPanY + (e.clientY - d.startY))
      return
    }

    const docPoint = toDocPoint(e)

    if (d.type === 'move-frame') {
      const dx = docPoint.x - d.startX
      const dy = docPoint.y - d.startY
      if (!d.moved && Math.abs(dx) * camera.zoom < 3 && Math.abs(dy) * camera.zoom < 3) return
      if (!d.moved) setDrag({ ...d, moved: true })
      const rect: Rect = { ...d.origRect, x: d.origRect.x + dx, y: d.origRect.y + dy }
      updateDocument(dd => setFrameRect(dd, d.pageId, d.frameId, rect))
    } else if (d.type === 'resize-frame') {
      const rect = resizeRect(d.origRect, d.handle, docPoint.x - d.startX, docPoint.y - d.startY)
      updateDocument(dd => setFrameRect(dd, d.pageId, d.frameId, rect))
    } else if (d.type === 'draw-frame') {
      setDrag({ ...d, currentX: docPoint.x, currentY: docPoint.y })
    } else if (d.type === 'text-select') {
      const ctx = measureCtx()
      if (ctx && threadGeos.length > 0) {
        const offset = caretOffsetForPoint(ctx, threadGeos, docPoint)
        const sel = useEditorStore.getState().selection
        if (sel?.type === 'text' && sel.focus !== offset) {
          setSelection({ ...sel, focus: offset })
        }
      }
    }
  }, [doc, camera, updateDocument, toDocPoint, updateHoverCursor, measureCtx, threadGeos, setSelection])

  const handleMouseUp = useCallback((e: React.MouseEvent) => {
    const d = dragRef.current
    setDrag(null)
    if (!d || !doc) return
    if (d.type === 'pan' || d.type === 'text-select') return
    const docPoint = toDocPoint(e)

    if (d.type === 'move-frame' || d.type === 'resize-frame') {
      const page = doc.pages.find(p => p.id === d.pageId)
      const frame = page?.frames.find(f => f.id === d.frameId)
      if (!frame) return
      const before = { ...d.origRect }
      const after = { ...frame.rect }
      const changed =
        before.x !== after.x || before.y !== after.y ||
        before.width !== after.width || before.height !== after.height
      if (changed) {
        // One grouped command per drag gesture so it undoes as a single step
        pushCommand(makeCommand(
          d.type,
          () => useEditorStore.getState().updateDocument(dd => setFrameRect(dd, d.pageId, d.frameId, after)),
          () => useEditorStore.getState().updateDocument(dd => setFrameRect(dd, d.pageId, d.frameId, before)),
          d.groupId
        ))
      }
      return
    }

    // draw-frame: create the frame at the rubber-band rect
    if (d.type === 'draw-frame') {
      const docRect: Rect = {
        x: Math.min(d.startX, docPoint.x),
        y: Math.min(d.startY, docPoint.y),
        width: Math.abs(docPoint.x - d.startX),
        height: Math.abs(docPoint.y - d.startY)
      }
      const target =
        findPageAtPoint(doc, { x: docRect.x, y: docRect.y }, spreadPositions) ??
        findPageAtPoint(doc, { x: d.startX, y: d.startY }, spreadPositions)

      if (target && docRect.width >= MIN_FRAME_SIZE && docRect.height >= MIN_FRAME_SIZE) {
        const localRect: Rect = {
          x: docRect.x - target.spread.x,
          y: docRect.y - target.spread.y,
          width: docRect.width,
          height: docRect.height
        }
        const pageId = target.page.id
        let frame: Frame
        let newThread: Thread | null = null

        if (activeTool === 'draw-text-frame') {
          newThread = createThread({})
          frame = {
            type: 'text',
            id: generateId('frame'),
            rect: localRect,
            threadId: newThread.id,
            threadOrder: 0
          }
        } else {
          frame = {
            type: 'image',
            id: generateId('frame'),
            rect: localRect,
            imageAssetId: null,
            wrapMode: 'rect',
            imageFit: 'fit'
          }
        }

        const thread = newThread
        const addFrame = () => useEditorStore.getState().updateDocument(dd => ({
          ...dd,
          threads: thread ? { ...dd.threads, [thread.id]: dd.threads[thread.id] ?? thread } : dd.threads,
          pages: dd.pages.map(p => (p.id !== pageId ? p : { ...p, frames: [...p.frames, frame] }))
        }))
        const removeFrame = () => useEditorStore.getState().updateDocument(dd => {
          const threads = { ...dd.threads }
          if (thread) delete threads[thread.id]
          return {
            ...dd,
            threads,
            pages: dd.pages.map(p =>
              p.id !== pageId ? p : { ...p, frames: p.frames.filter(f => f.id !== frame.id) }
            )
          }
        })

        addFrame()
        pushCommand(makeCommand('draw-frame', addFrame, removeFrame))

        if (frame.type === 'text') {
          // Drop straight into editing — that's what you want a fresh text frame for.
          setSelection({ type: 'text', threadId: frame.threadId, frameId: frame.id, pageId, anchor: 0, focus: 0 })
        } else {
          setSelection({ type: 'frame', frameId: frame.id, pageId })
        }
      }

      // Switch tool back to select after drawing
      setActiveTool('select')
    }
  }, [doc, activeTool, spreadPositions, setSelection, setActiveTool, toDocPoint])

  const handleDoubleClick = useCallback((e: React.MouseEvent) => {
    if (!doc || activeTool !== 'select') return
    const docPoint = toDocPoint(e)

    if (textSel) {
      // Double-click while editing selects the word
      const ctx = measureCtx()
      const thread = doc.threads[textSel.threadId]
      if (ctx && thread) {
        const offset = caretOffsetForPoint(ctx, threadGeos, docPoint)
        const { start, end } = expandToWord(getPlainText(thread), offset)
        setSelection({ ...textSel, anchor: start, focus: end })
      }
      return
    }

    const hit = hitTestFrames(docPoint, doc.pages, spreadPositions)
    if (hit && hit.frame.type === 'text') {
      enterTextEditing(hit.frame, hit.pageId, docPoint, false)
    }
  }, [doc, activeTool, textSel, threadGeos, spreadPositions, toDocPoint, measureCtx, setSelection, enterTextEditing])

  // ---- Wheel: pan / ctrl+wheel zoom-at-cursor (native, non-passive) ----
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const onWheel = (e: WheelEvent) => {
      e.preventDefault()
      const state = useEditorStore.getState()
      if (!state.document) return
      const rect = canvas.getBoundingClientRect()
      if (e.ctrlKey || e.metaKey) {
        const pivot = { x: e.clientX - rect.left, y: e.clientY - rect.top }
        const factor = Math.exp(-e.deltaY * 0.0015)
        state.setCamera(zoomAt(
          { zoom: state.zoom, panX: state.panX, panY: state.panY },
          state.zoom * factor,
          pivot
        ))
      } else {
        state.setPan(state.panX - e.deltaX, state.panY - e.deltaY)
      }
    }
    canvas.addEventListener('wheel', onWheel, { passive: false })
    return () => canvas.removeEventListener('wheel', onWheel)
  }, [])

  // ---- Frame-level keyboard shortcuts ----

  const moveSelectedFrame = useCallback((dx: number, dy: number) => {
    if (!doc || selection?.type !== 'frame') return
    const page = doc.pages.find(p => p.id === selection.pageId)
    const frame = page?.frames.find(f => f.id === selection.frameId)
    if (!frame) return
    const before = { ...frame.rect }
    const after = { ...frame.rect, x: frame.rect.x + dx, y: frame.rect.y + dy }
    const { pageId, frameId } = selection
    updateDocument(dd => setFrameRect(dd, pageId, frameId, after))

    // Coalesce a held-arrow burst into one undo step
    const now = Date.now()
    let groupId: string
    if (nudgeRef.current && now - nudgeRef.current.last < 600) {
      groupId = nudgeRef.current.groupId
      nudgeRef.current.last = now
    } else {
      groupId = generateId('nudge')
      nudgeRef.current = { groupId, last: now }
    }
    pushCommand(makeCommand(
      'nudge-frame',
      () => useEditorStore.getState().updateDocument(dd => setFrameRect(dd, pageId, frameId, after)),
      () => useEditorStore.getState().updateDocument(dd => setFrameRect(dd, pageId, frameId, before)),
      groupId
    ))
  }, [doc, selection, updateDocument])

  const deleteSelectedFrame = useCallback(() => {
    if (!doc || selection?.type !== 'frame') return
    const page = doc.pages.find(p => p.id === selection.pageId)
    const frameIndex = page?.frames.findIndex(f => f.id === selection.frameId) ?? -1
    if (!page || frameIndex === -1) return
    const frame = page.frames[frameIndex]
    const pageId = page.id

    // If this is the only frame of its thread, remove the thread too
    let orphanThread: Thread | null = null
    if (frame.type === 'text') {
      const others = doc.pages.some(p =>
        p.frames.some(f => f.type === 'text' && f.threadId === frame.threadId && f.id !== frame.id)
      )
      if (!others) orphanThread = doc.threads[frame.threadId] ?? null
    }

    const removeFrame = () =>
      useEditorStore.getState().updateDocument(dd => {
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
    const restoreFrame = () =>
      useEditorStore.getState().updateDocument(dd => ({
        ...dd,
        threads: orphanThread ? { ...dd.threads, [orphanThread.id]: orphanThread } : dd.threads,
        pages: dd.pages.map(p => {
          if (p.id !== pageId) return p
          const frames = [...p.frames]
          frames.splice(Math.min(frameIndex, frames.length), 0, frame)
          return { ...p, frames }
        })
      }))

    removeFrame()
    setSelection(null)
    pushCommand(makeCommand('delete-frame', removeFrame, restoreFrame))
  }, [doc, selection, setSelection])

  const cycleToNextThreadFrame = useCallback(() => {
    if (!doc || selection?.type !== 'frame') return
    const page = doc.pages.find(p => p.id === selection.pageId)
    const frame = page?.frames.find(f => f.id === selection.frameId)
    if (!frame || frame.type !== 'text') return

    const threadFrames: Array<{ frameId: string; pageId: string; order: number }> = []
    for (const p of doc.pages) {
      for (const f of p.frames) {
        if (f.type === 'text' && f.threadId === frame.threadId) {
          threadFrames.push({ frameId: f.id, pageId: p.id, order: f.threadOrder })
        }
      }
    }
    threadFrames.sort((a, b) => a.order - b.order)
    if (threadFrames.length < 2) return
    const idx = threadFrames.findIndex(tf => tf.frameId === frame.id)
    const next = threadFrames[(idx + 1) % threadFrames.length]
    setSelection({ type: 'frame', frameId: next.frameId, pageId: next.pageId })
  }, [doc, selection, setSelection])

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Already handled (e.g. by the hidden textarea's handler earlier in
      // this same dispatch — React can re-register this listener mid-event).
      if (e.defaultPrevented) return

      const mod = e.metaKey || e.ctrlKey

      // Ignore keystrokes aimed at real form fields (dialog inputs) — but not
      // our own hidden textarea, whose events bubble here too.
      const target = e.target as HTMLElement | null
      const inFormField =
        target && target !== textareaRef.current &&
        (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)
      if (inFormField) return

      // App-level shortcuts. The native menu handles these too when its
      // accelerators fire; when it consumes the key this handler never sees
      // it, so there is no double-execution.
      if (mod) {
        const key = e.key.toLowerCase()
        if (key === 'z' && !e.shiftKey) { e.preventDefault(); typingRef.current = null; undo(); return }
        if ((key === 'z' && e.shiftKey) || key === 'y') { e.preventDefault(); typingRef.current = null; redo(); return }
        if (key === 'b' && selection?.type === 'text') { e.preventDefault(); toggleStyleFlag('bold'); return }
        if (key === 'i' && selection?.type === 'text') { e.preventDefault(); toggleStyleFlag('italic'); return }
        if (key === '=' || key === '+') { e.preventDefault(); zoomStep(1); return }
        if (key === '-') { e.preventDefault(); zoomStep(-1); return }
        if (key === '0') { e.preventDefault(); useEditorStore.getState().requestFit(); return }
      }

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
          case 'Escape':
            e.preventDefault()
            setSelection(null)
            break
          case 'Enter': {
            // Enter starts editing a selected text frame
            e.preventDefault()
            const page = doc?.pages.find(p => p.id === selection.pageId)
            const frame = page?.frames.find(f => f.id === selection.frameId)
            if (frame?.type === 'text') enterTextEditing(frame, selection.pageId, null, false)
            break
          }
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [doc, selection, moveSelectedFrame, deleteSelectedFrame, cycleToNextThreadFrame, setSelection, enterTextEditing])

  return (
    <>
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onDoubleClick={handleDoubleClick}
      />
      <textarea
        ref={textareaRef}
        aria-hidden
        tabIndex={-1}
        onInput={handleTextareaInput}
        onCompositionEnd={handleCompositionEnd}
        onKeyDown={handleTextareaKeyDown}
        onCopy={handleCopy}
        onCut={handleCut}
        onPaste={handlePaste}
        style={{
          position: 'fixed',
          top: 0,
          left: 0,
          width: 1,
          height: 1,
          padding: 0,
          border: 'none',
          outline: 'none',
          opacity: 0,
          pointerEvents: 'none',
          resize: 'none',
          overflow: 'hidden'
        }}
      />
    </>
  )
}
