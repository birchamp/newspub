// src/renderer/history/commands.ts

export interface Command {
  type: string
  apply: () => void
  reverse: () => void
  groupId?: string
  timestamp: number
}
