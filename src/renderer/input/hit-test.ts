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
