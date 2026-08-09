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
