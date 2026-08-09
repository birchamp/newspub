// src/renderer/ui/actions.ts
// Shared editing actions used by the canvas, toolbar, and menu so every
// entry point goes through the same undo history and store updates.
import { appHistory } from '@history/app-history'
import type { Command } from '@history/commands'
import { useEditorStore } from '@ui/store/editor-store'
import { applyStyle, getRunAtOffset } from '@model/thread'
import { ZOOM_STOPS } from '@canvas/viewport'
import type { Thread } from '@model/types'

export function makeCommand(type: string, apply: () => void, reverse: () => void, groupId?: string): Command {
  return { type, apply, reverse, groupId, timestamp: Date.now() }
}

export function pushCommand(cmd: Command): void {
  appHistory.push(cmd)
  useEditorStore.getState().bumpHistory()
}

// After undo/redo the selected frame or thread may be gone or shorter —
// drop or clamp the selection so nothing points at stale state.
function reconcileSelection(): void {
  const state = useEditorStore.getState()
  const { document: doc, selection } = state
  if (!doc || !selection) return
  if (selection.type === 'frame') {
    const page = doc.pages.find(p => p.id === selection.pageId)
    if (!page?.frames.some(f => f.id === selection.frameId)) state.setSelection(null)
    return
  }
  const thread = doc.threads[selection.threadId]
  if (!thread) { state.setSelection(null); return }
  const len = thread.runs.reduce((sum, r) => sum + r.text.length, 0)
  if (selection.anchor > len || selection.focus > len) {
    state.setSelection({
      ...selection,
      anchor: Math.min(selection.anchor, len),
      focus: Math.min(selection.focus, len)
    })
  }
}

export function undo(): void {
  appHistory.undo()
  reconcileSelection()
  useEditorStore.getState().bumpHistory()
}

export function redo(): void {
  appHistory.redo()
  reconcileSelection()
  useEditorStore.getState().bumpHistory()
}

export function canUndo(): boolean { return appHistory.canUndo() }
export function canRedo(): boolean { return appHistory.canRedo() }

function setThread(threadId: string, thread: Thread): void {
  useEditorStore.getState().updateDocument(dd => ({
    ...dd,
    threads: { ...dd.threads, [threadId]: thread }
  }))
}

/** Is the given style flag active at the current text selection? */
export function selectionStyleFlag(flag: 'bold' | 'italic'): boolean {
  const { document: doc, selection } = useEditorStore.getState()
  if (!doc || selection?.type !== 'text') return false
  const thread = doc.threads[selection.threadId]
  if (!thread || thread.runs.length === 0) return false
  const start = Math.min(selection.anchor, selection.focus)
  const probe = start === Math.max(selection.anchor, selection.focus) && start > 0 ? start - 1 : start
  const { runIndex } = getRunAtOffset(thread, probe)
  return !!thread.runs[runIndex]?.style[flag]
}

/** Toggle bold/italic over the current text selection (undoable) */
export function toggleStyleFlag(flag: 'bold' | 'italic'): void {
  const { document: doc, selection } = useEditorStore.getState()
  if (!doc || selection?.type !== 'text') return
  const thread = doc.threads[selection.threadId]
  if (!thread) return
  const start = Math.min(selection.anchor, selection.focus)
  const end = Math.max(selection.anchor, selection.focus)
  if (start === end) return

  const current = selectionStyleFlag(flag)
  const before = thread
  const after = applyStyle(thread, start, end, { [flag]: !current })
  const threadId = thread.id

  setThread(threadId, after)
  pushCommand(makeCommand(
    `toggle-${flag}`,
    () => setThread(threadId, after),
    () => setThread(threadId, before)
  ))
}

/** Step zoom to the next/previous preset stop, anchored at viewport center */
export function zoomStep(direction: 1 | -1): void {
  const { zoom, zoomAtCenter } = useEditorStore.getState()
  const stops = ZOOM_STOPS
  let target: number | undefined
  if (direction === 1) {
    target = stops.find(s => s > zoom + 0.001)
  } else {
    target = [...stops].reverse().find(s => s < zoom - 0.001)
  }
  if (target !== undefined) zoomAtCenter(target)
}
