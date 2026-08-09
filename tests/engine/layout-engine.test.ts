// tests/engine/layout-engine.test.ts
import { describe, it, expect, vi } from 'vitest'
import { layoutThread } from '@engine/layout-engine'
import type { Document, Thread, TextFrame, Page } from '@model/types'

// Mock frame-layout module
vi.mock('@engine/frame-layout', () => ({
  layoutFrameText: vi.fn()
    .mockReturnValueOnce({
      lines: [{ text: 'First frame text', width: 100, x: 8, y: 8, height: 20, runStyles: [] }],
      endOffset: 16,
      overflow: true
    })
    .mockReturnValueOnce({
      lines: [{ text: 'Second frame text', width: 100, x: 8, y: 8, height: 20, runStyles: [] }],
      endOffset: 34,
      overflow: false
    })
}))

describe('layoutThread', () => {
  it('flows text across multiple frames', () => {
    const thread: Thread = {
      id: 'thread-1',
      runs: [{ text: 'First frame text Second frame text', style: { fontFamily: 'Inter', fontSize: 14 } }],
      defaultStyle: { fontFamily: 'Inter', fontSize: 14 }
    }

    const frames: Array<{ frame: TextFrame; pageId: string; pageIndex: number }> = [
      {
        frame: { type: 'text', id: 'f1', rect: { x: 0, y: 0, width: 200, height: 100 }, threadId: 'thread-1', threadOrder: 0 },
        pageId: 'p1',
        pageIndex: 0
      },
      {
        frame: { type: 'text', id: 'f2', rect: { x: 0, y: 0, width: 200, height: 100 }, threadId: 'thread-1', threadOrder: 1 },
        pageId: 'p2',
        pageIndex: 1
      }
    ]

    const result = layoutThread(thread, frames, [], 20)

    expect(result.frameLayouts).toHaveLength(2)
    expect(result.frameLayouts[0].lines[0].text).toBe('First frame text')
    expect(result.frameLayouts[0].continuationTo?.pageNumber).toBe(2)
    expect(result.overflow).toBe(false)
  })
})
