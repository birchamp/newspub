// src/renderer/ui/components/PageSidebar.tsx
import { useEffect, useRef } from 'react'
import { useEditorStore } from '@ui/store/editor-store'
import { layoutDocument } from '@engine/layout-engine'
import type { Document } from '@model/types'
import type { DocumentLayout } from '@engine/layout-types'

export default function PageSidebar() {
  const { document: doc, currentPageIndex, scrollToPage } = useEditorStore()

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

  const layout = layoutDocument(doc)

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
            onClick={() => scrollToPage(spread.pageIndices[0])}
            className="page-thumb-item"
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
            <div style={{ display: 'flex', gap: 4, justifyContent: 'center', marginBottom: 4 }}>
              {spread.pageIndices.map(pi => (
                <PageThumb
                  key={doc.pages[pi]?.id ?? pi}
                  doc={doc}
                  layout={layout}
                  pageIndex={pi}
                  width={spread.pageIndices.length > 1 ? 40 : 50}
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

/** Miniature live preview of a page: frame outlines and text-line hints */
function PageThumb({ doc, layout, pageIndex, width }: {
  doc: Document
  layout: DocumentLayout
  pageIndex: number
  width: number
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const page = doc.pages[pageIndex]
  const { width: pw, height: ph } = doc.metadata.pageSize
  const height = Math.round(width * ph / pw)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !page) return
    const dpr = window.devicePixelRatio || 1
    canvas.width = width * dpr
    canvas.height = height * dpr
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    const scale = (width * dpr) / pw
    ctx.setTransform(scale, 0, 0, scale, 0, 0)

    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, pw, ph)

    for (const frame of page.frames) {
      const r = frame.rect
      if (frame.type === 'image') {
        ctx.fillStyle = 'rgba(249, 115, 22, 0.12)'
        ctx.fillRect(r.x, r.y, r.width, r.height)
        ctx.strokeStyle = 'rgba(249, 115, 22, 0.5)'
        ctx.lineWidth = 1 / scale
        ctx.strokeRect(r.x, r.y, r.width, r.height)
      } else {
        ctx.strokeStyle = 'rgba(99, 102, 241, 0.35)'
        ctx.lineWidth = 1 / scale
        ctx.strokeRect(r.x, r.y, r.width, r.height)
        // Text-line hints
        const fl = layout.threadLayouts[frame.threadId]?.frameLayouts.find(f => f.frameId === frame.id)
        if (fl) {
          ctx.fillStyle = 'rgba(60, 60, 60, 0.55)'
          for (const line of fl.lines) {
            const w = Math.min(line.width, r.width - line.x * 2)
            if (w > 0) ctx.fillRect(r.x + line.x, r.y + line.y + line.height * 0.25, w, line.height * 0.45)
          }
        }
      }
    }
  }, [page, layout, width, height, pw, ph])

  return (
    <canvas
      ref={canvasRef}
      style={{
        width,
        height,
        background: '#fff',
        border: '1px solid #ccc',
        borderRadius: 2
      }}
    />
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
