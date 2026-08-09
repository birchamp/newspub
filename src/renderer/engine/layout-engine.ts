// src/renderer/engine/layout-engine.ts
import type { Document, Thread, TextFrame, ImageFrame, Page } from '@model/types'
import type {
  DocumentLayout, ThreadLayout, FrameLayout, ExclusionZone
} from './layout-types'
import { layoutFrameText } from './frame-layout'
import { computeExclusionZones } from './wrap-calculator'

export function layoutThread(
  thread: Thread,
  frames: Array<{ frame: TextFrame; pageId: string; pageIndex: number }>,
  imageFrames: Array<{ rect: import('@model/types').Rect; wrapMode: 'skip' | 'rect' }>,
  lineHeight?: number  // omit to derive per-frame from the text's font size
): ThreadLayout {
  const frameLayouts: FrameLayout[] = []
  let currentOffset = 0
  const totalTextLength = thread.runs.reduce((sum, r) => sum + r.text.length, 0)

  for (let i = 0; i < frames.length; i++) {
    const { frame, pageId, pageIndex } = frames[i]

    // Compute exclusion zones from overlapping images
    const exclusions = computeExclusionZones(frame.rect, imageFrames)

    const result = layoutFrameText({
      runs: thread.runs,
      frameRect: frame.rect,
      lineHeight,
      exclusions,
      startOffset: currentOffset
    })

    const frameLayout: FrameLayout = {
      frameId: frame.id,
      pageId,
      lines: result.lines,
      overflow: result.overflow && i === frames.length - 1,
      threadCursorEnd: result.endOffset
    }

    // Continuation markers
    if (result.overflow && i < frames.length - 1) {
      const nextPageIndex = frames[i + 1].pageIndex
      if (nextPageIndex !== pageIndex) {
        frameLayout.continuationTo = { pageNumber: nextPageIndex + 1 }
      }
    }
    if (i > 0) {
      const prevPageIndex = frames[i - 1].pageIndex
      if (prevPageIndex !== pageIndex) {
        frameLayout.continuationFrom = { pageNumber: prevPageIndex + 1 }
      }
    }

    frameLayouts.push(frameLayout)
    currentOffset = result.endOffset

    // If all text is consumed, stop
    if (!result.overflow) break
  }

  return {
    threadId: thread.id,
    frameLayouts,
    totalTextLength,
    overflow: currentOffset < totalTextLength
  }
}

export function layoutDocument(doc: Document): DocumentLayout {
  const threadLayouts: Record<string, ThreadLayout> = {}

  // Collect all image frames across pages for exclusion calculation
  const allImageFrames = doc.pages.flatMap(page =>
    page.frames
      .filter((f): f is ImageFrame => f.type === 'image')
      .map(f => ({ rect: f.rect, wrapMode: f.wrapMode }))
  )

  for (const [threadId, thread] of Object.entries(doc.threads)) {
    // Collect all text frames for this thread, ordered by threadOrder
    const frames: Array<{ frame: TextFrame; pageId: string; pageIndex: number }> = []

    doc.pages.forEach((page, pageIndex) => {
      page.frames
        .filter((f): f is TextFrame => f.type === 'text' && f.threadId === threadId)
        .sort((a, b) => a.threadOrder - b.threadOrder)
        .forEach(frame => {
          frames.push({ frame, pageId: page.id, pageIndex })
        })
    })

    // Sort all frames by threadOrder globally
    frames.sort((a, b) => a.frame.threadOrder - b.frame.threadOrder)

    if (frames.length > 0) {
      threadLayouts[threadId] = layoutThread(thread, frames, allImageFrames)
    }
  }

  return { threadLayouts }
}
