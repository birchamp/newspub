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
