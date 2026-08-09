import { HistoryManager } from './history-manager'

// Single shared undo/redo stack for the whole app: canvas interactions,
// keyboard edits, and menu Undo/Redo all operate on the same history.
export const appHistory = new HistoryManager()
