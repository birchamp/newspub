// src/renderer/canvas/image-painter.ts
import type { Rect, ImageFit } from '@model/types'

// Cache loaded images by asset ID
const imageCache = new Map<string, HTMLImageElement>()

export function loadImage(assetId: string, data: ArrayBuffer, mimeType: string): Promise<HTMLImageElement> {
  if (imageCache.has(assetId)) return Promise.resolve(imageCache.get(assetId)!)

  return new Promise((resolve, reject) => {
    const blob = new Blob([data], { type: mimeType })
    const url = URL.createObjectURL(blob)
    const img = new Image()
    img.onload = () => {
      imageCache.set(assetId, img)
      resolve(img)
    }
    img.onerror = reject
    img.src = url
  })
}

export function paintImage(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement,
  rect: Rect,
  fit: ImageFit
): void {
  ctx.save()

  let sx = 0, sy = 0, sw = img.width, sh = img.height
  let dx = rect.x, dy = rect.y, dw = rect.width, dh = rect.height

  if (fit === 'fit') {
    const scale = Math.min(rect.width / img.width, rect.height / img.height)
    dw = img.width * scale
    dh = img.height * scale
    dx = rect.x + (rect.width - dw) / 2
    dy = rect.y + (rect.height - dh) / 2
  } else if (fit === 'fill') {
    const scale = Math.max(rect.width / img.width, rect.height / img.height)
    sw = rect.width / scale
    sh = rect.height / scale
    sx = (img.width - sw) / 2
    sy = (img.height - sh) / 2
  }
  // 'stretch' uses rect directly

  ctx.drawImage(img, sx, sy, sw, sh, dx, dy, dw, dh)
  ctx.restore()
}

export function paintBackgroundImage(
  ctx: CanvasRenderingContext2D,
  img: HTMLImageElement,
  pageWidth: number,
  pageHeight: number
): void {
  ctx.drawImage(img, 0, 0, pageWidth, pageHeight)
}

export function clearImageCache(): void {
  imageCache.clear()
}
