// tests/template/template-manager.test.ts
import { describe, it, expect } from 'vitest'
import { stripToTemplate } from '@template/template-manager'
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'
import { addTextFrame, addImageFrame } from '@model/page'
import type { Document, TemplateSaveOptions } from '@model/types'

function makeTestDoc(): Document {
  let doc = createDocument({
    title: 'Test Newsletter',
    author: 'Author',
    pageSize: PAGE_SIZES['US Letter'],
    pageCount: 4
  })

  // Add a thread with content
  doc.threads['t1'] = {
    id: 't1',
    runs: [{ text: 'Article body', style: { fontFamily: 'Inter', fontSize: 14 } }],
    defaultStyle: { fontFamily: 'Inter', fontSize: 14 }
  }

  // Add frames
  doc = addTextFrame(doc, doc.pages[0].id, {
    rect: { x: 50, y: 50, width: 200, height: 300 },
    threadId: 't1',
    threadOrder: 0,
    label: 'Body Text'
  })

  doc = addImageFrame(doc, doc.pages[0].id, {
    rect: { x: 300, y: 50, width: 150, height: 150 },
    wrapMode: 'rect',
    imageFit: 'fit',
    imageAssetId: 'img-1'
  })

  // Add an asset
  doc.assets['img-1'] = {
    id: 'img-1',
    filename: 'photo.jpg',
    mimeType: 'image/jpeg',
    data: new ArrayBuffer(10)
  }

  // Add background image
  doc.pages[0].backgroundImageAssetId = 'bg-1'
  doc.assets['bg-1'] = {
    id: 'bg-1',
    filename: 'border.png',
    mimeType: 'image/png',
    data: new ArrayBuffer(20)
  }

  return doc
}

describe('stripToTemplate', () => {
  it('strips text when keepArticleText is false', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: false,
      keepPlacedImages: true,
      keepBackgroundImages: true
    })
    expect(template.threads['t1'].runs).toEqual([])
  })

  it('keeps text when keepArticleText is true', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: true,
      keepPlacedImages: true,
      keepBackgroundImages: true
    })
    expect(template.threads['t1'].runs[0].text).toBe('Article body')
  })

  it('strips placed images when keepPlacedImages is false', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: false,
      keepPlacedImages: false,
      keepBackgroundImages: true
    })
    const imgFrame = template.pages[0].frames.find(f => f.type === 'image')
    expect(imgFrame).toBeDefined()
    expect((imgFrame as any).imageAssetId).toBeNull()
    expect(template.assets['img-1']).toBeUndefined()
  })

  it('keeps background images when keepBackgroundImages is true', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: false,
      keepPlacedImages: false,
      keepBackgroundImages: true
    })
    expect(template.pages[0].backgroundImageAssetId).toBe('bg-1')
    expect(template.assets['bg-1']).toBeDefined()
  })

  it('strips background images when keepBackgroundImages is false', () => {
    const doc = makeTestDoc()
    const template = stripToTemplate(doc, {
      keepArticleText: false,
      keepPlacedImages: false,
      keepBackgroundImages: false
    })
    expect(template.pages[0].backgroundImageAssetId).toBeNull()
    expect(template.assets['bg-1']).toBeUndefined()
  })
})
