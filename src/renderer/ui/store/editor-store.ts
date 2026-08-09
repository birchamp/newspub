import { create } from 'zustand'
import type { Document, Frame, TextStyle, Rect } from '@model/types'

export type ToolMode = 'select' | 'draw-text-frame' | 'draw-image-frame'

export type Selection = {
  type: 'frame'
  frameId: string
  pageId: string
} | {
  type: 'text'
  threadId: string
  anchor: number
  focus: number
} | null

export interface EditorState {
  // Document
  document: Document | null
  setDocument: (doc: Document) => void
  updateDocument: (updater: (doc: Document) => Document) => void

  // Selection
  selection: Selection
  setSelection: (sel: Selection) => void

  // Tool
  activeTool: ToolMode
  setActiveTool: (tool: ToolMode) => void

  // Viewport
  zoom: number
  setZoom: (zoom: number) => void
  panX: number
  panY: number
  setPan: (x: number, y: number) => void

  // Current page/spread
  currentPageIndex: number
  setCurrentPageIndex: (index: number) => void

  // Dirty tracking
  isDirty: boolean
  markDirty: () => void
  markClean: () => void

  // File path
  filePath: string | null
  setFilePath: (path: string | null) => void
}

export const useEditorStore = create<EditorState>((set) => ({
  document: null,
  setDocument: (doc) => set({ document: doc }),
  updateDocument: (updater) =>
    set((state) => ({
      document: state.document ? updater(state.document) : null,
      isDirty: true
    })),

  selection: null,
  setSelection: (sel) => set({ selection: sel }),

  activeTool: 'select',
  setActiveTool: (tool) => set({ activeTool: tool }),

  zoom: 1,
  setZoom: (zoom) => set({ zoom: Math.max(0.25, Math.min(4, zoom)) }),
  panX: 0,
  panY: 0,
  setPan: (x, y) => set({ panX: x, panY: y }),

  currentPageIndex: 0,
  setCurrentPageIndex: (index) => set({ currentPageIndex: index }),

  isDirty: false,
  markDirty: () => set({ isDirty: true }),
  markClean: () => set({ isDirty: false }),

  filePath: null,
  setFilePath: (path) => set({ filePath: path })
}))
