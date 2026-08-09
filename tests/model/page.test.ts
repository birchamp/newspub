import { describe, it, expect } from 'vitest'
import { addTextFrame, addImageFrame, removeFrame, addSpread } from '@model/page'
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'
import type { Document } from '@model/types'

describe('addTextFrame', () => {
  it('adds a text frame to a page', () => {
    const doc = createDocument({ title: 'T', author: '', pageSize: PAGE_SIZES['US Letter'], pageCount: 2 })
    const updated = addTextFrame(doc, doc.pages[0].id, {
      rect: { x: 50, y: 50, width: 200, height: 300 },
      threadId: 'thread-1',
      threadOrder: 0
    })
    expect(updated.pages[0].frames).toHaveLength(1)
    expect(updated.pages[0].frames[0].type).toBe('text')
  })
})

describe('addImageFrame', () => {
  it('adds an image frame with wrap mode', () => {
    const doc = createDocument({ title: 'T', author: '', pageSize: PAGE_SIZES['US Letter'], pageCount: 2 })
    const updated = addImageFrame(doc, doc.pages[0].id, {
      rect: { x: 100, y: 100, width: 150, height: 150 },
      wrapMode: 'rect',
      imageFit: 'fit'
    })
    expect(updated.pages[0].frames).toHaveLength(1)
    expect(updated.pages[0].frames[0].type).toBe('image')
  })
})

describe('removeFrame', () => {
  it('removes a frame by id', () => {
    let doc = createDocument({ title: 'T', author: '', pageSize: PAGE_SIZES['US Letter'], pageCount: 2 })
    doc = addTextFrame(doc, doc.pages[0].id, {
      rect: { x: 50, y: 50, width: 200, height: 300 },
      threadId: 'thread-1',
      threadOrder: 0
    })
    const frameId = doc.pages[0].frames[0].id
    const updated = removeFrame(doc, doc.pages[0].id, frameId)
    expect(updated.pages[0].frames).toHaveLength(0)
  })
})

describe('addSpread', () => {
  it('adds two pages to the document', () => {
    const doc = createDocument({ title: 'T', author: '', pageSize: PAGE_SIZES['US Letter'], pageCount: 4 })
    const updated = addSpread(doc, 4) // insert after page index 3
    expect(updated.pages).toHaveLength(6)
  })
})
