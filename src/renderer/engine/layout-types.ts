// src/renderer/engine/layout-types.ts
import type { Rect, TextStyle } from '@model/types'

export interface LayoutLine {
  text: string
  width: number
  x: number       // relative to frame
  y: number       // relative to frame
  height: number  // line height
  runStyles: Array<{
    text: string
    style: TextStyle
    x: number     // x offset within line
    width: number
  }>
}

export interface FrameLayout {
  frameId: string
  pageId: string
  lines: LayoutLine[]
  overflow: boolean
  threadCursorEnd: number  // offset in thread where this frame's text ends
  continuationTo?: { pageNumber: number }
  continuationFrom?: { pageNumber: number }
}

export interface ThreadLayout {
  threadId: string
  frameLayouts: FrameLayout[]
  totalTextLength: number
  overflow: boolean
}

export interface DocumentLayout {
  threadLayouts: Record<string, ThreadLayout>
}

export interface ExclusionZone {
  rect: Rect
  mode: 'skip' | 'rect'
}
