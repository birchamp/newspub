// src/renderer/input/input-manager.ts
import type { Point } from '@model/types'
import { parseHtmlToRuns, runsToHtml, runsToPlainText } from './clipboard'
import type { TextSelection } from './selection'
import { createTextSelection, getSelectionRange, isCollapsed } from './selection'

export interface InputManagerCallbacks {
  onTextInsert: (text: string, offset: number) => void
  onTextDelete: (from: number, to: number) => void
  onPaste: (runs: ReturnType<typeof parseHtmlToRuns>, offset: number) => void
  onSelectionChange: (selection: TextSelection | null) => void
  requestRender: () => void
}

export class InputManager {
  private textarea: HTMLTextAreaElement
  private callbacks: InputManagerCallbacks
  private currentSelection: TextSelection | null = null

  constructor(
    container: HTMLElement,
    callbacks: InputManagerCallbacks
  ) {
    this.callbacks = callbacks

    // Create hidden textarea
    this.textarea = document.createElement('textarea')
    this.textarea.style.cssText = `
      position: fixed;
      top: -9999px;
      left: -9999px;
      width: 1px;
      height: 1px;
      opacity: 0;
      pointer-events: none;
    `
    container.appendChild(this.textarea)

    this.textarea.addEventListener('input', this.handleInput)
    this.textarea.addEventListener('compositionstart', this.handleCompositionStart)
    this.textarea.addEventListener('compositionend', this.handleCompositionEnd)
    this.textarea.addEventListener('paste', this.handlePaste)
    this.textarea.addEventListener('copy', this.handleCopy)
    this.textarea.addEventListener('cut', this.handleCut)
  }

  focus(): void {
    this.textarea.focus()
  }

  setSelection(sel: TextSelection | null): void {
    this.currentSelection = sel
  }

  /** Position textarea near the cursor for IME popup placement */
  positionNearCursor(screenX: number, screenY: number): void {
    this.textarea.style.top = `${screenY}px`
    this.textarea.style.left = `${screenX}px`
  }

  destroy(): void {
    this.textarea.removeEventListener('input', this.handleInput)
    this.textarea.removeEventListener('paste', this.handlePaste)
    this.textarea.removeEventListener('copy', this.handleCopy)
    this.textarea.removeEventListener('cut', this.handleCut)
    this.textarea.remove()
  }

  private handleInput = (e: Event): void => {
    const inputEvent = e as InputEvent
    if (!this.currentSelection) return

    const text = inputEvent.data ?? ''
    if (text.length === 0) return

    const sel = this.currentSelection
    if (!isCollapsed(sel)) {
      const range = getSelectionRange(sel)
      this.callbacks.onTextDelete(range.start, range.end)
      this.callbacks.onTextInsert(text, range.start)
    } else {
      this.callbacks.onTextInsert(text, sel.anchor)
    }

    // Clear textarea for next input
    this.textarea.value = ''
    this.callbacks.requestRender()
  }

  private handleCompositionStart = (): void => {
    // IME composition started — don't process input events until end
  }

  private handleCompositionEnd = (e: CompositionEvent): void => {
    // Final composed text
    if (!this.currentSelection) return
    const text = e.data ?? ''
    if (text.length > 0) {
      this.callbacks.onTextInsert(text, this.currentSelection.anchor)
      this.textarea.value = ''
      this.callbacks.requestRender()
    }
  }

  private handlePaste = (e: ClipboardEvent): void => {
    e.preventDefault()
    if (!this.currentSelection) return

    const html = e.clipboardData?.getData('text/html')
    const plain = e.clipboardData?.getData('text/plain')

    if (html) {
      const runs = parseHtmlToRuns(html)
      this.callbacks.onPaste(runs, this.currentSelection.anchor)
    } else if (plain) {
      this.callbacks.onTextInsert(plain, this.currentSelection.anchor)
    }

    this.callbacks.requestRender()
  }

  private handleCopy = (e: ClipboardEvent): void => {
    // Copy is handled by the store — this is a hook point
    e.preventDefault()
  }

  private handleCut = (e: ClipboardEvent): void => {
    e.preventDefault()
    if (!this.currentSelection || isCollapsed(this.currentSelection)) return
    const range = getSelectionRange(this.currentSelection)
    this.callbacks.onTextDelete(range.start, range.end)
    this.callbacks.requestRender()
  }
}
