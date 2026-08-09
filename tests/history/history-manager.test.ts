// tests/history/history-manager.test.ts
import { describe, it, expect, beforeEach } from 'vitest'
import { HistoryManager } from '@history/history-manager'
import type { Command } from '@history/commands'

describe('HistoryManager', () => {
  let history: HistoryManager
  let value: number

  beforeEach(() => {
    history = new HistoryManager()
    value = 0
  })

  function makeCommand(delta: number, groupId?: string): Command {
    return {
      type: 'test',
      apply: () => { value += delta },
      reverse: () => { value -= delta },
      groupId,
      timestamp: Date.now()
    }
  }

  it('undoes a single command', () => {
    const cmd = makeCommand(10)
    cmd.apply()
    history.push(cmd)
    expect(value).toBe(10)

    history.undo()
    expect(value).toBe(0)
  })

  it('redoes after undo', () => {
    const cmd = makeCommand(10)
    cmd.apply()
    history.push(cmd)
    history.undo()
    history.redo()
    expect(value).toBe(10)
  })

  it('clears redo stack on new command', () => {
    const cmd1 = makeCommand(10)
    cmd1.apply()
    history.push(cmd1)

    history.undo()
    expect(value).toBe(0)

    const cmd2 = makeCommand(5)
    cmd2.apply()
    history.push(cmd2)

    history.redo() // should do nothing
    expect(value).toBe(5)
  })

  it('undoes grouped commands as one step', () => {
    const group = 'drag-1'
    for (let i = 0; i < 5; i++) {
      const cmd = makeCommand(1, group)
      cmd.apply()
      history.push(cmd)
    }
    expect(value).toBe(5)

    history.undo() // should undo all 5
    expect(value).toBe(0)
  })

  it('treats different groups as separate undo steps', () => {
    const cmd1 = makeCommand(10, 'group-a')
    cmd1.apply()
    history.push(cmd1)

    const cmd2 = makeCommand(20, 'group-b')
    cmd2.apply()
    history.push(cmd2)

    expect(value).toBe(30)
    history.undo() // undo group-b
    expect(value).toBe(10)
    history.undo() // undo group-a
    expect(value).toBe(0)
  })

  it('reports canUndo and canRedo', () => {
    expect(history.canUndo()).toBe(false)
    expect(history.canRedo()).toBe(false)

    const cmd = makeCommand(10)
    cmd.apply()
    history.push(cmd)

    expect(history.canUndo()).toBe(true)
    expect(history.canRedo()).toBe(false)

    history.undo()
    expect(history.canUndo()).toBe(false)
    expect(history.canRedo()).toBe(true)
  })
})
