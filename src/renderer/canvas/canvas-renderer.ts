// src/renderer/canvas/canvas-renderer.ts
import type { Document, Page, TextFrame, ImageFrame, Frame } from '@model/types'
import type { DocumentLayout, FrameLayout } from '@engine/layout-types'
import type { Camera } from './viewport'
import { applyCamera, resetTransform, getSpreadPositions } from './viewport'
import { paintTextLines, paintContinuationMarker, paintOverflowIndicator } from './text-painter'
import { paintImage, paintBackgroundImage } from './image-painter'
import { paintFrameBorder, paintResizeHandles, paintSelectionHighlight } from './frame-chrome-painter'
import { paintCursor } from './cursor-painter'
import type { Selection } from '@ui/store/editor-store'

interface RenderOptions {
  doc: Document
  layout: DocumentLayout
  camera: Camera
  canvas: HTMLCanvasElement
  selection: Selection
  editMode: boolean
  loadedImages: Map<string, HTMLImageElement>
}

export function render(options: RenderOptions): void {
  const { doc, layout, camera, canvas, selection, editMode, loadedImages } = options
  const ctx = canvas.getContext('2d')
  if (!ctx) return

  // Clear
  resetTransform(ctx)
  ctx.fillStyle = '#e5e5e5' // gray workspace background
  ctx.fillRect(0, 0, canvas.width, canvas.height)

  // Apply camera
  applyCamera(ctx, camera)

  const spreadPositions = getSpreadPositions(
    doc.pages.length,
    doc.metadata.pageSize
  )

  // Render each page
  for (const sp of spreadPositions) {
    const page = doc.pages[sp.pageIndex]
    if (!page) continue

    ctx.save()
    ctx.translate(sp.x, sp.y)

    renderPage(ctx, doc, page, layout, {
      editMode,
      selection,
      loadedImages,
      pageWidth: doc.metadata.pageSize.width,
      pageHeight: doc.metadata.pageSize.height
    })

    ctx.restore()
  }
}

interface PageRenderOptions {
  editMode: boolean
  selection: Selection
  loadedImages: Map<string, HTMLImageElement>
  pageWidth: number
  pageHeight: number
}

function renderPage(
  ctx: CanvasRenderingContext2D,
  doc: Document,
  page: Page,
  layout: DocumentLayout,
  options: PageRenderOptions
): void {
  const { editMode, selection, loadedImages, pageWidth, pageHeight } = options

  // 1. Page background
  ctx.fillStyle = '#ffffff'
  ctx.fillRect(0, 0, pageWidth, pageHeight)

  // 2. Background/decoration image
  if (page.backgroundImageAssetId) {
    const img = loadedImages.get(page.backgroundImageAssetId)
    if (img) paintBackgroundImage(ctx, img, pageWidth, pageHeight)
  }

  // 3. Image frames
  for (const frame of page.frames) {
    if (frame.type !== 'image') continue
    const imgFrame = frame as ImageFrame

    if (imgFrame.imageAssetId) {
      const img = loadedImages.get(imgFrame.imageAssetId)
      if (img) paintImage(ctx, img, imgFrame.rect, imgFrame.imageFit)
    }

    if (editMode) {
      const isSelected = selection?.type === 'frame' && selection.frameId === frame.id
      paintFrameBorder(ctx, frame.rect, 'image', isSelected, !imgFrame.imageAssetId)
      if (isSelected) paintResizeHandles(ctx, frame.rect)
    }
  }

  // 4. Text frames
  for (const frame of page.frames) {
    if (frame.type !== 'text') continue
    const textFrame = frame as TextFrame

    // Find layout for this frame
    const threadLayout = layout.threadLayouts[textFrame.threadId]
    const frameLayout = threadLayout?.frameLayouts.find(fl => fl.frameId === frame.id)

    if (frameLayout) {
      paintTextLines(ctx, frameLayout.lines, textFrame.rect.x, textFrame.rect.y, textFrame.rect.width)

      // Continuation markers
      if (frameLayout.continuationTo) {
        paintContinuationMarker(
          ctx,
          `Cont. pg ${frameLayout.continuationTo.pageNumber}`,
          textFrame.rect.x + 4,
          textFrame.rect.y + textFrame.rect.height - 4
        )
      }
      if (frameLayout.continuationFrom) {
        paintContinuationMarker(
          ctx,
          `Cont. from pg ${frameLayout.continuationFrom.pageNumber}`,
          textFrame.rect.x + 4,
          textFrame.rect.y + 14
        )
      }

      // Overflow indicator
      if (frameLayout.overflow) {
        paintOverflowIndicator(
          ctx,
          textFrame.rect.x + textFrame.rect.width,
          textFrame.rect.y + textFrame.rect.height
        )
      }
    }

    if (editMode) {
      const isSelected = selection?.type === 'frame' && selection.frameId === frame.id
      const isEmpty = !frameLayout || frameLayout.lines.length === 0
      paintFrameBorder(ctx, frame.rect, 'text', isSelected, isEmpty)
      if (isSelected) paintResizeHandles(ctx, frame.rect)
    }
  }
}
