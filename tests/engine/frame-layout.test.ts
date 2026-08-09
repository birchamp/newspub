// tests/engine/frame-layout.test.ts
import { describe, it, expect, vi } from 'vitest'
import { layoutFrameText } from '@engine/frame-layout'
import type { StyledRun, Rect } from '@model/types'
import type { ExclusionZone } from '@engine/layout-types'

// Mock pretext since it requires a real Canvas context
vi.mock('@chenglou/pretext', () => ({
  prepareWithSegments: vi.fn(() => ({
    segments: [{ text: 'Hello world this is a test of text layout in a frame' }]
  })),
  layoutNextLine: vi.fn()
    .mockReturnValueOnce({ text: 'Hello world this is', width: 180, start: { segmentIndex: 0, graphemeIndex: 0 }, end: { segmentIndex: 0, graphemeIndex: 19 } })
    .mockReturnValueOnce({ text: 'a test of text layout', width: 190, start: { segmentIndex: 0, graphemeIndex: 19 }, end: { segmentIndex: 0, graphemeIndex: 40 } })
    .mockReturnValueOnce({ text: 'in a frame', width: 90, start: { segmentIndex: 0, graphemeIndex: 40 }, end: { segmentIndex: 0, graphemeIndex: 50 } })
    .mockReturnValueOnce(null) // signals end of text
}))

describe('layoutFrameText', () => {
  it('produces lines that fit within the frame', () => {
    const runs: StyledRun[] = [
      { text: 'Hello world this is a test of text layout in a frame', style: { fontFamily: 'Inter', fontSize: 14 } }
    ]
    const frameRect: Rect = { x: 50, y: 50, width: 200, height: 100 }
    const lineHeight = 20

    const result = layoutFrameText({
      runs,
      frameRect,
      lineHeight,
      exclusions: [],
      startOffset: 0
    })

    expect(result.lines.length).toBeGreaterThan(0)
    expect(result.lines[0].text).toBe('Hello world this is')
  })
})
