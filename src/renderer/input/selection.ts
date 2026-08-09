// src/renderer/input/selection.ts

export interface TextSelection {
  threadId: string
  anchor: number
  focus: number
}

export function createTextSelection(
  threadId: string,
  anchor: number,
  focus: number
): TextSelection {
  return { threadId, anchor, focus }
}

export function isCollapsed(sel: TextSelection): boolean {
  return sel.anchor === sel.focus
}

export function getSelectionRange(sel: TextSelection): { start: number; end: number } {
  return {
    start: Math.min(sel.anchor, sel.focus),
    end: Math.max(sel.anchor, sel.focus)
  }
}

export function expandToWord(
  text: string,
  offset: number
): { start: number; end: number } {
  const wordBreak = /[\s.,!?;:'"()\[\]{}<>\/\\]/

  let start = offset
  while (start > 0 && !wordBreak.test(text[start - 1])) {
    start--
  }

  let end = offset
  while (end < text.length && !wordBreak.test(text[end])) {
    end++
  }

  return { start, end }
}

export function expandToParagraph(
  text: string,
  offset: number
): { start: number; end: number } {
  let start = offset
  while (start > 0 && text[start - 1] !== '\n') {
    start--
  }

  let end = offset
  while (end < text.length && text[end] !== '\n') {
    end++
  }

  return { start, end }
}
