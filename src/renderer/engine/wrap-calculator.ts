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
