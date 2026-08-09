// src/renderer/file/autosave.ts
import type { Document } from '@model/types'
import { packNewspub } from './file-manager'

const AUTOSAVE_INTERVAL_MS = 60_000 // 60 seconds

export class AutosaveManager {
  private timer: ReturnType<typeof setInterval> | null = null
  private isDirty = false
  private getDocument: () => Document | null
  private saveFn: (data: ArrayBuffer, path: string) => Promise<void>
  private getFilePath: () => string | null

  constructor(
    getDocument: () => Document | null,
    getFilePath: () => string | null,
    saveFn: (data: ArrayBuffer, path: string) => Promise<void>
  ) {
    this.getDocument = getDocument
    this.getFilePath = getFilePath
    this.saveFn = saveFn
  }

  start(): void {
    this.stop()
    this.timer = setInterval(() => this.tick(), AUTOSAVE_INTERVAL_MS)
  }

  stop(): void {
    if (this.timer) {
      clearInterval(this.timer)
      this.timer = null
    }
  }

  markDirty(): void {
    this.isDirty = true
  }

  markClean(): void {
    this.isDirty = false
  }

  private async tick(): Promise<void> {
    if (!this.isDirty) return
    const doc = this.getDocument()
    const filePath = this.getFilePath()
    if (!doc || !filePath) return

    try {
      const buffer = await packNewspub(doc)
      await this.saveFn(buffer, filePath + '.autosave')
      this.isDirty = false
    } catch (err) {
      console.error('Autosave failed:', err)
    }
  }
}
