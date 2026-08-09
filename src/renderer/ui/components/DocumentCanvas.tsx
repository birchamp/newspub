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
