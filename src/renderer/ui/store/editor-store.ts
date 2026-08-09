import { create } from 'zustand'
import type { Document } from '@model/types'
import { clampZoom, zoomAt, clampPan, getContentBounds, type Camera } from '@canvas/viewport'

export type ToolMode = 'select' | 'draw-text-frame' | 'draw-image-frame'

export type Selection = {
  type: 'frame'
  frameId: string
  pageId: string
} | {
  type: 'text'
  threadId: string
  frameId: string
  pageId: string
  anchor: number
  focus: number
} | null

export interface EditorState {
  // Document
  document: Document | null
  /** Bumped each time a *different* document is loaded (not on edits) */
  docEpoch: number
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
  panX: number
  panY: number
  viewportW: number
  viewportH: number
  setViewportSize: (w: number, h: number) => void
  setZoom: (zoom: number) => void
  setPan: (x: number, y: number) => void
  setCamera: (camera: Camera) => void
  /** Zoom keeping the viewport center anchored, with pan clamping */
  zoomAtCenter: (zoom: number) => void
  /** Ask the canvas to re-fit the document into view */
  fitRequest: number
  requestFit: () => void
  /** Ask the canvas to scroll a page into view */
  scrollTarget: { pageIndex: number; nonce: number } | null
  scrollToPage: (pageIndex: number) => void

  // Current page/spread
  currentPageIndex: number
  setCurrentPageIndex: (index: number) => void

  // Undo/redo UI state — bumped by ui/actions so buttons re-render
  historyVersion: number
  bumpHistory: () => void

  // Dirty tracking
  isDirty: boolean
  markDirty: () => void
  markClean: () => void

  // File path
  filePath: string | null
  setFilePath: (path: string | null) => void
}

function clampedCamera(state: EditorState, camera: Camera): Camera {
  if (!state.document) return camera
  const bounds = getContentBounds(state.document.pages.length, state.document.metadata.pageSize)
  return clampPan(camera, state.viewportW || 1, state.viewportH || 1, bounds)
}

export const useEditorStore = create<EditorState>((set) => ({
  document: null,
  docEpoch: 0,
  setDocument: (doc) => set((state) => ({
    document: doc,
    docEpoch: state.docEpoch + 1,
    selection: null,
    isDirty: false,
    currentPageIndex: 0
  })),
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
  panX: 0,
  panY: 0,
  viewportW: 0,
  viewportH: 0,
  setViewportSize: (w, h) => set({ viewportW: w, viewportH: h }),
  setZoom: (zoom) => set({ zoom: clampZoom(zoom) }),
  setPan: (x, y) => set((state) => {
    const cam = clampedCamera(state, { zoom: state.zoom, panX: x, panY: y })
    return { panX: cam.panX, panY: cam.panY }
  }),
  setCamera: (camera) => set((state) => {
    const cam = clampedCamera(state, { ...camera, zoom: clampZoom(camera.zoom) })
    return { zoom: cam.zoom, panX: cam.panX, panY: cam.panY }
  }),
  zoomAtCenter: (zoom) => set((state) => {
    const pivot = { x: state.viewportW / 2, y: state.viewportH / 2 }
    const next = zoomAt({ zoom: state.zoom, panX: state.panX, panY: state.panY }, zoom, pivot)
    const cam = clampedCamera(state, next)
    return { zoom: cam.zoom, panX: cam.panX, panY: cam.panY }
  }),

  fitRequest: 0,
  requestFit: () => set((state) => ({ fitRequest: state.fitRequest + 1 })),

  scrollTarget: null,
  scrollToPage: (pageIndex) => set((state) => ({
    scrollTarget: { pageIndex, nonce: (state.scrollTarget?.nonce ?? 0) + 1 },
    currentPageIndex: pageIndex
  })),

  currentPageIndex: 0,
  setCurrentPageIndex: (index) => set({ currentPageIndex: index }),

  historyVersion: 0,
  bumpHistory: () => set((state) => ({ historyVersion: state.historyVersion + 1 })),

  isDirty: false,
  markDirty: () => set({ isDirty: true }),
  markClean: () => set({ isDirty: false }),

  filePath: null,
  setFilePath: (path) => set({ filePath: path })
}))
