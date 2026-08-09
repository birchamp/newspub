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
