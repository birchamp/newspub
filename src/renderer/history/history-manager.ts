// src/renderer/history/history-manager.ts
import type { Command } from './commands'

export class HistoryManager {
  private undoStack: Command[] = []
  private redoStack: Command[] = []

  push(command: Command): void {
    this.undoStack.push(command)
    this.redoStack = [] // new action clears redo
  }

  undo(): void {
    if (!this.canUndo()) return

    const last = this.undoStack[this.undoStack.length - 1]
    const groupId = last.groupId

    if (groupId) {
      // Pop all commands in this group
      const group: Command[] = []
      while (
        this.undoStack.length > 0 &&
        this.undoStack[this.undoStack.length - 1].groupId === groupId
      ) {
        group.push(this.undoStack.pop()!)
      }
      // Reverse in order (newest first)
      for (const cmd of group) {
        cmd.reverse()
      }
      this.redoStack.push(...group.reverse())
    } else {
      const cmd = this.undoStack.pop()!
      cmd.reverse()
      this.redoStack.push(cmd)
    }
  }

  redo(): void {
    if (!this.canRedo()) return

    const next = this.redoStack[this.redoStack.length - 1]
    const groupId = next.groupId

    if (groupId) {
      const group: Command[] = []
      while (
        this.redoStack.length > 0 &&
        this.redoStack[this.redoStack.length - 1].groupId === groupId
      ) {
        group.push(this.redoStack.pop()!)
      }
      for (const cmd of group) {
        cmd.apply()
      }
      this.undoStack.push(...group.reverse())
    } else {
      const cmd = this.redoStack.pop()!
      cmd.apply()
      this.undoStack.push(cmd)
    }
  }

  canUndo(): boolean {
    return this.undoStack.length > 0
  }

  canRedo(): boolean {
    return this.redoStack.length > 0
  }

  clear(): void {
    this.undoStack = []
    this.redoStack = []
  }
}
