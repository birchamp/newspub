// tests/export/pdf-exporter.test.ts
import { describe, it, expect } from 'vitest'
import { exportToPdf } from '@export/pdf-exporter'
import { createDocument } from '@model/document'
import { createThread, insertText } from '@model/thread'
import { addTextFrame } from '@model/page'
import { PAGE_SIZES } from '@model/page-sizes'
import type { DocumentLayout } from '@engine/layout-types'

describe('exportToPdf', () => {
  it('produces a valid PDF buffer', async () => {
    let doc = createDocument({
      title: 'PDF Test',
      author: 'Test',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 1
    })

    let thread = createThread({ fontFamily: 'Helvetica', fontSize: 14 })
    thread = insertText(thread, 0, 'Hello PDF world')
    doc.threads[thread.id] = thread

    doc = addTextFrame(doc, doc.pages[0].id, {
      rect: { x: 50, y: 50, width: 200, height: 300 },
      threadId: thread.id,
      threadOrder: 0
    })

    // Minimal mock layout
    const layout: DocumentLayout = {
      threadLayouts: {
        [thread.id]: {
          threadId: thread.id,
          frameLayouts: [{
            frameId: doc.pages[0].frames[0].id,
            pageId: doc.pages[0].id,
            lines: [{
              text: 'Hello PDF world',
              width: 120,
              x: 8,
              y: 8,
              height: 20,
              runStyles: [{
                text: 'Hello PDF world',
                style: { fontFamily: 'Helvetica', fontSize: 14 },
                x: 8,
                width: 120
              }]
            }],
            overflow: false,
            threadCursorEnd: 15
          }],
          totalTextLength: 15,
          overflow: false
        }
      }
    }

    const result = await exportToPdf(doc, layout)
    expect(result.pdfBytes.byteLength).toBeGreaterThan(0)
    expect(result.warnings).toEqual([])

    // Check PDF magic bytes
    const header = new Uint8Array(result.pdfBytes.slice(0, 5))
    const magic = String.fromCharCode(...header)
    expect(magic).toBe('%PDF-')
  })
})
