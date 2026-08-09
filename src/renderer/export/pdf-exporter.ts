// src/renderer/export/pdf-exporter.ts
import { PDFDocument, StandardFonts, rgb } from 'pdf-lib'
import type { Document, Page, TextFrame, ImageFrame } from '@model/types'
import type { DocumentLayout, FrameLayout, LayoutLine } from '@engine/layout-types'
import { resolveFont, resolveStandardFont } from './font-resolver'
import { toArrayBuffer } from '@file/file-manager'
import { lineAlignOffset } from '@engine/alignment'

interface ExportOptions {
  pageRange?: { start: number; end: number }
}

interface ExportResult {
  pdfBytes: ArrayBuffer
  warnings: string[]
}

export async function exportToPdf(
  doc: Document,
  layout: DocumentLayout,
  options: ExportOptions = {}
): Promise<ExportResult> {
  const pdfDoc = await PDFDocument.create()
  const warnings: string[] = []

  // Pre-load standard fonts
  const fontCache = new Map<string, Awaited<ReturnType<typeof pdfDoc.embedFont>>>()

  async function getFont(family: string, bold: boolean, italic: boolean) {
    const resolved = resolveFont(family, bold, italic)
    warnings.push(...resolved.warnings)

    if (!fontCache.has(resolved.fontName)) {
      const font = await pdfDoc.embedFont(resolved.fontName)
      fontCache.set(resolved.fontName, font)
    }
    return fontCache.get(resolved.fontName)!
  }

  // Determine page range
  const startPage = options.pageRange?.start ?? 0
  const endPage = options.pageRange?.end ?? doc.pages.length

  for (let i = startPage; i < endPage; i++) {
    const page = doc.pages[i]
    if (!page) continue

    const pdfPage = pdfDoc.addPage([
      doc.metadata.pageSize.width,
      doc.metadata.pageSize.height
    ])
    const pageHeight = doc.metadata.pageSize.height

    // Background image (if any)
    if (page.backgroundImageAssetId) {
      const asset = doc.assets[page.backgroundImageAssetId]
      if (asset) {
        try {
          let pdfImage
          if (asset.mimeType === 'image/jpeg') {
            pdfImage = await pdfDoc.embedJpg(new Uint8Array(asset.data))
          } else if (asset.mimeType === 'image/png') {
            pdfImage = await pdfDoc.embedPng(new Uint8Array(asset.data))
          }
          if (pdfImage) {
            pdfPage.drawImage(pdfImage, {
              x: 0,
              y: 0,
              width: doc.metadata.pageSize.width,
              height: doc.metadata.pageSize.height
            })
          }
        } catch (err) {
          warnings.push(`Failed to embed background image: ${asset.filename}`)
        }
      }
    }

    // Image frames
    for (const frame of page.frames) {
      if (frame.type !== 'image') continue
      const imgFrame = frame as ImageFrame
      if (!imgFrame.imageAssetId) continue

      const asset = doc.assets[imgFrame.imageAssetId]
      if (!asset) continue

      try {
        let pdfImage
        if (asset.mimeType === 'image/jpeg') {
          pdfImage = await pdfDoc.embedJpg(new Uint8Array(asset.data))
        } else if (asset.mimeType === 'image/png') {
          pdfImage = await pdfDoc.embedPng(new Uint8Array(asset.data))
        }
        if (pdfImage) {
          // PDF y-axis is bottom-up
          pdfPage.drawImage(pdfImage, {
            x: imgFrame.rect.x,
            y: pageHeight - imgFrame.rect.y - imgFrame.rect.height,
            width: imgFrame.rect.width,
            height: imgFrame.rect.height
          })
        }
      } catch (err) {
        warnings.push(`Failed to embed image: ${asset.filename}`)
      }
    }

    // Text frames
    for (const frame of page.frames) {
      if (frame.type !== 'text') continue
      const textFrame = frame as TextFrame

      const threadLayout = layout.threadLayouts[textFrame.threadId]
      const frameLayout = threadLayout?.frameLayouts.find(
        fl => fl.frameId === frame.id
      )
      if (!frameLayout) continue

      for (const line of frameLayout.lines) {
        const align = lineAlignOffset(line, textFrame.rect.width)
        for (const runStyle of line.runStyles) {
          const font = await getFont(
            runStyle.style.fontFamily ?? 'Helvetica',
            runStyle.style.bold ?? false,
            runStyle.style.italic ?? false
          )

          const fontSize = runStyle.style.fontSize ?? 14

          // Parse color
          let color = rgb(0, 0, 0)
          if (runStyle.style.color) {
            const hex = runStyle.style.color.replace('#', '')
            const r = parseInt(hex.slice(0, 2), 16) / 255
            const g = parseInt(hex.slice(2, 4), 16) / 255
            const b = parseInt(hex.slice(4, 6), 16) / 255
            color = rgb(r, g, b)
          }

          // PDF y-axis is bottom-up
          const pdfY = pageHeight - (textFrame.rect.y + line.y + line.height * 0.8)

          pdfPage.drawText(runStyle.text, {
            x: textFrame.rect.x + runStyle.x + align,
            y: pdfY,
            size: fontSize,
            font,
            color
          })
        }
      }

      // Continuation markers
      if (frameLayout.continuationTo) {
        const markerFont = await getFont('Helvetica', false, true)
        const markerY = pageHeight - (textFrame.rect.y + textFrame.rect.height - 4)
        pdfPage.drawText(`Cont. pg ${frameLayout.continuationTo.pageNumber}`, {
          x: textFrame.rect.x + 4,
          y: markerY,
          size: 10,
          font: markerFont,
          color: rgb(0.53, 0.53, 0.53)
        })
      }
    }
  }

  // Deduplicate warnings
  const uniqueWarnings = [...new Set(warnings)]

  const pdfBytes = await pdfDoc.save()
  return { pdfBytes: toArrayBuffer(pdfBytes), warnings: uniqueWarnings }
}
