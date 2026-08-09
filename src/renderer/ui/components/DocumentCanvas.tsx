// src/renderer/ui/components/DocumentCanvas.tsx
import { useRef, useEffect, useCallback, useState } from 'react'
import { useEditorStore } from '@ui/store/editor-store'
import { render } from '@canvas/canvas-renderer'
import { layoutDocument } from '@engine/layout-engine'
import { screenToDoc, applyCamera, resetTransform, type Camera } from '@canvas/viewport'
import { hitTestFrames, hitTestResizeHandle, type HandlePosition } from '@input/hit-test'
import { getSpreadPositions, type SpreadPosition } from '@canvas/viewport'
import { applyStyle, getRunAtOffset, createThread } from '@model/thread'
import { generateId } from '@model/document'
import { appHistory as canvasHistory } from '@history/app-history'
import type { Command } from '@history/commands'
import type { Document, Frame, Page, Point, Rect } from '@model/types'

const MIN_FRAME_SIZE = 8

// Interaction state tracked during a mouse drag (Task 19).
type DragState =
  | null
  | { type: 'move-frame'; frameId: string; pageId: string; startX: number; startY: number; origRect: Rect; groupId: string }
  | { type: 'resize-frame'; frameId: string; pageId: string; handle: HandlePosition; startX: number; startY: number; origRect: Rect; groupId: string }
  | { type: 'draw-frame'; startX: number; startY: number; currentX: number; currentY: number }

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

function makeCommand(type: string, apply: () => void, reverse: () => void, groupId?: string): Command {
  return { type, apply, reverse, groupId, timestamp: Date.now() }
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

export default function DocumentCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const {
    document: doc,
    updateDocument,
    selection,
    setSelection,
    zoom,
    panX,
    panY,
    setPan,
    setZoom,
    activeTool,
    setActiveTool
  } = useEditorStore()

  const camera: Camera = { zoom, panX, panY }
  const [drag, setDrag] = useState<DragState>(null)
  const dragRef = useRef<DragState>(null)
  dragRef.current = drag

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

    // Rubber-band preview while drawing a new frame (Task 19)
    if (drag?.type === 'draw-frame') {
      applyCamera(ctx, camera)
      const x = Math.min(drag.startX, drag.currentX)
      const y = Math.min(drag.startY, drag.currentY)
      const w = Math.abs(drag.currentX - drag.startX)
      const h = Math.abs(drag.currentY - drag.startY)
      ctx.save()
      ctx.strokeStyle = '#2680eb'
      ctx.setLineDash([4, 3])
      ctx.lineWidth = 1 / camera.zoom
      ctx.strokeRect(x, y, w, h)
      ctx.restore()
      resetTransform(ctx)
    }
  }, [doc, selection, zoom, panX, panY, drag])

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

  const toDocPoint = useCallback((e: { clientX: number; clientY: number }): Point => {
    const canvas = canvasRef.current!
    const rect = canvas.getBoundingClientRect()
    return screenToDoc({ x: e.clientX - rect.left, y: e.clientY - rect.top }, camera)
  }, [camera])

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!doc) return
    const docPoint = toDocPoint(e)
    const spreadPositions = getSpreadPositions(doc.pages.length, doc.metadata.pageSize)

    // Draw new frame: rubber-band start (Task 19)
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

    // Resize handle hit-test on the currently selected frame (Task 19)
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
      // Start a move drag on the hit frame (Task 19)
      setDrag({
        type: 'move-frame',
        frameId: hit.frame.id,
        pageId: hit.pageId,
        startX: docPoint.x,
        startY: docPoint.y,
        origRect: { ...hit.frame.rect },
        groupId: generateId('drag')
      })
    } else {
      setSelection(null)
    }
  }, [doc, camera, activeTool, selection, setSelection, toDocPoint])

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const d = dragRef.current
    if (!d || !doc) return
    const docPoint = toDocPoint(e)

    if (d.type === 'move-frame') {
      const rect: Rect = {
        ...d.origRect,
        x: d.origRect.x + (docPoint.x - d.startX),
        y: d.origRect.y + (docPoint.y - d.startY)
      }
      updateDocument(dd => setFrameRect(dd, d.pageId, d.frameId, rect))
    } else if (d.type === 'resize-frame') {
      const rect = resizeRect(d.origRect, d.handle, docPoint.x - d.startX, docPoint.y - d.startY)
      updateDocument(dd => setFrameRect(dd, d.pageId, d.frameId, rect))
    } else if (d.type === 'draw-frame') {
      setDrag({ ...d, currentX: docPoint.x, currentY: docPoint.y })
    }
  }, [doc, updateDocument, toDocPoint])

  const handleMouseUp = useCallback((e: React.MouseEvent) => {
    const d = dragRef.current
    setDrag(null)
    if (!d || !doc) return
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
        canvasHistory.push(makeCommand(
          d.type,
          () => useEditorStore.getState().updateDocument(dd => setFrameRect(dd, d.pageId, d.frameId, after)),
          () => useEditorStore.getState().updateDocument(dd => setFrameRect(dd, d.pageId, d.frameId, before)),
          d.groupId
        ))
      }
      return
    }

    // draw-frame: create the frame at the rubber-band rect (Task 19)
    if (d.type === 'draw-frame') {
      const spreadPositions = getSpreadPositions(doc.pages.length, doc.metadata.pageSize)
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
        let newThreadId: string | null = null

        if (activeTool === 'draw-text-frame') {
          const thread = createThread({})
          newThreadId = thread.id
          frame = {
            type: 'text',
            id: generateId('frame'),
            rect: localRect,
            threadId: thread.id,
            threadOrder: 0
          }
          updateDocument(dd => ({
            ...dd,
            threads: { ...dd.threads, [thread.id]: thread },
            pages: dd.pages.map(p => (p.id !== pageId ? p : { ...p, frames: [...p.frames, frame] }))
          }))
        } else {
          frame = {
            type: 'image',
            id: generateId('frame'),
            rect: localRect,
            imageAssetId: null,
            wrapMode: 'rect',
            imageFit: 'fit'
          }
          updateDocument(dd => ({
            ...dd,
            pages: dd.pages.map(p => (p.id !== pageId ? p : { ...p, frames: [...p.frames, frame] }))
          }))
        }

        setSelection({ type: 'frame', frameId: frame.id, pageId })

        const threadId = newThreadId
        canvasHistory.push(makeCommand(
          'draw-frame',
          () => useEditorStore.getState().updateDocument(dd => ({
            ...dd,
            threads: threadId && frame.type === 'text'
              ? { ...dd.threads, [threadId]: dd.threads[threadId] ?? createThread({}) }
              : dd.threads,
            pages: dd.pages.map(p => (p.id !== pageId ? p : { ...p, frames: [...p.frames, frame] }))
          })),
          () => useEditorStore.getState().updateDocument(dd => ({
            ...dd,
            pages: dd.pages.map(p =>
              p.id !== pageId ? p : { ...p, frames: p.frames.filter(f => f.id !== frame.id) }
            )
          }))
        ))
      }

      // Switch tool back to select after drawing
      setActiveTool('select')
    }
  }, [doc, activeTool, updateDocument, setSelection, setActiveTool, toDocPoint])

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

  // ---- Keyboard shortcut helpers (Task 20) ----

  const moveSelectedFrame = useCallback((dx: number, dy: number) => {
    if (!doc || selection?.type !== 'frame') return
    const page = doc.pages.find(p => p.id === selection.pageId)
    const frame = page?.frames.find(f => f.id === selection.frameId)
    if (!frame) return
    const before = { ...frame.rect }
    const after = { ...frame.rect, x: frame.rect.x + dx, y: frame.rect.y + dy }
    const { pageId, frameId } = selection
    updateDocument(dd => setFrameRect(dd, pageId, frameId, after))
    canvasHistory.push(makeCommand(
      'nudge-frame',
      () => useEditorStore.getState().updateDocument(dd => setFrameRect(dd, pageId, frameId, after)),
      () => useEditorStore.getState().updateDocument(dd => setFrameRect(dd, pageId, frameId, before))
    ))
  }, [doc, selection, updateDocument])

  const deleteSelectedFrame = useCallback(() => {
    if (!doc || selection?.type !== 'frame') return
    const page = doc.pages.find(p => p.id === selection.pageId)
    const frameIndex = page?.frames.findIndex(f => f.id === selection.frameId) ?? -1
    if (!page || frameIndex === -1) return
    const frame = page.frames[frameIndex]
    const pageId = page.id

    const removeFrame = () =>
      useEditorStore.getState().updateDocument(dd => ({
        ...dd,
        pages: dd.pages.map(p =>
          p.id !== pageId ? p : { ...p, frames: p.frames.filter(f => f.id !== frame.id) }
        )
      }))
    const restoreFrame = () =>
      useEditorStore.getState().updateDocument(dd => ({
        ...dd,
        pages: dd.pages.map(p => {
          if (p.id !== pageId) return p
          const frames = [...p.frames]
          frames.splice(Math.min(frameIndex, frames.length), 0, frame)
          return { ...p, frames }
        })
      }))

    removeFrame()
    setSelection(null)
    canvasHistory.push(makeCommand('delete-frame', removeFrame, restoreFrame))
  }, [doc, selection, setSelection])

  const cycleToNextThreadFrame = useCallback(() => {
    if (!doc || selection?.type !== 'frame') return
    const page = doc.pages.find(p => p.id === selection.pageId)
    const frame = page?.frames.find(f => f.id === selection.frameId)
    if (!frame || frame.type !== 'text') return

    // Collect all frames of this thread across all pages, sorted by threadOrder
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

  const toggleStyleFlag = useCallback((flag: 'bold' | 'italic') => {
    if (!doc || selection?.type !== 'text') return
    const thread = doc.threads[selection.threadId]
    if (!thread) return
    const start = Math.min(selection.anchor, selection.focus)
    const end = Math.max(selection.anchor, selection.focus)
    if (start === end) return

    const { runIndex } = getRunAtOffset(thread, start)
    const currentValue = !!thread.runs[runIndex]?.style[flag]
    const threadId = thread.id
    const beforeThread = thread
    const afterThread = applyStyle(thread, start, end, { [flag]: !currentValue })

    const setThread = (t: typeof thread) =>
      useEditorStore.getState().updateDocument(dd => ({
        ...dd,
        threads: { ...dd.threads, [threadId]: t }
      }))

    setThread(afterThread)
    canvasHistory.push(makeCommand(
      `toggle-${flag}`,
      () => setThread(afterThread),
      () => setThread(beforeThread)
    ))
  }, [doc, selection])

  const toggleBold = useCallback(() => toggleStyleFlag('bold'), [toggleStyleFlag])
  const toggleItalic = useCallback(() => toggleStyleFlag('italic'), [toggleStyleFlag])

  // ---- Keyboard shortcuts (Task 20) ----
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
  }, [selection, moveSelectedFrame, deleteSelectedFrame, cycleToNextThreadFrame, toggleBold, toggleItalic])

  return (
    <canvas
      ref={canvasRef}
      style={{ width: '100%', height: '100%', cursor: activeTool === 'select' ? 'default' : 'crosshair' }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onWheel={handleWheel}
    />
  )
}
