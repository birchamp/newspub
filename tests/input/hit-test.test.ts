// tests/input/hit-test.test.ts
import { describe, it, expect } from 'vitest'
import { hitTestFrame, hitTestResizeHandle } from '@input/hit-test'
import type { Rect } from '@model/types'

describe('hitTestFrame', () => {
  it('returns true when point is inside frame', () => {
    const rect: Rect = { x: 50, y: 50, width: 200, height: 300 }
    expect(hitTestFrame({ x: 100, y: 100 }, rect)).toBe(true)
  })

  it('returns false when point is outside frame', () => {
    const rect: Rect = { x: 50, y: 50, width: 200, height: 300 }
    expect(hitTestFrame({ x: 10, y: 10 }, rect)).toBe(false)
  })
})

describe('hitTestResizeHandle', () => {
  it('identifies corner handles', () => {
    const rect: Rect = { x: 50, y: 50, width: 200, height: 300 }
    const result = hitTestResizeHandle({ x: 50, y: 50 }, rect)
    expect(result).toBe('top-left')
  })

  it('returns null when not near a handle', () => {
    const rect: Rect = { x: 50, y: 50, width: 200, height: 300 }
    const result = hitTestResizeHandle({ x: 150, y: 200 }, rect)
    expect(result).toBeNull()
  })
})
