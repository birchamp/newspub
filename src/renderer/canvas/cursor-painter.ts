// src/renderer/canvas/cursor-painter.ts

let blinkVisible = true
let blinkInterval: ReturnType<typeof setInterval> | null = null

export function startCursorBlink(): void {
  if (blinkInterval) return
  blinkVisible = true
  blinkInterval = setInterval(() => {
    blinkVisible = !blinkVisible
  }, 530)
}

export function stopCursorBlink(): void {
  if (blinkInterval) {
    clearInterval(blinkInterval)
    blinkInterval = null
  }
  blinkVisible = false
}

export function resetCursorBlink(): void {
  // Called on keypress to keep cursor visible while typing
  blinkVisible = true
  if (blinkInterval) {
    clearInterval(blinkInterval)
    blinkInterval = setInterval(() => {
      blinkVisible = !blinkVisible
    }, 530)
  }
}

export function paintCursor(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  height: number
): void {
  if (!blinkVisible) return
  ctx.save()
  ctx.fillStyle = '#000000'
  ctx.fillRect(x, y, 2, height)
  ctx.restore()
}
