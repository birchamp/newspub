// src/renderer/template/preset-templates.ts
import type { Document, TextFrame, Page } from '@model/types'
import { createDocument, generateId } from '@model/document'
import { createThread } from '@model/thread'
import { PAGE_SIZES } from '@model/page-sizes'
import type { TemplateMetadata } from '@model/types'

export interface PresetTemplate {
  metadata: TemplateMetadata
  build: () => Document
}

export const presetTemplates: PresetTemplate[] = [
  {
    metadata: {
      name: 'Classic 4-Page Newsletter',
      description: 'Front page hero, 2-3 spread with 3 columns, back page contacts',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 4
    },
    build: () => buildClassic4Page()
  },
  {
    metadata: {
      name: 'Simple 2-Page',
      description: 'Single front and back with 2 columns',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 2
    },
    build: () => buildSimple2Page()
  }
]

function buildClassic4Page(): Document {
  const doc = createDocument({
    title: 'Untitled Newsletter',
    author: '',
    pageSize: PAGE_SIZES['US Letter'],
    pageCount: 4
  })

  const margin = 36 // 0.5 inch
  const pw = 612
  const ph = 792
  const contentW = pw - margin * 2
  const colW = (contentW - 12) / 2 // 2 columns with 12pt gutter

  // Page 1: headline + 2-column intro
  const headlineThread = createThread({ fontFamily: 'Georgia', fontSize: 28, bold: true })
  doc.threads[headlineThread.id] = headlineThread

  const bodyThread = createThread({ fontFamily: 'Inter', fontSize: 11 })
  doc.threads[bodyThread.id] = bodyThread

  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: contentW, height: 60 },
    threadId: headlineThread.id, threadOrder: 0, label: 'Headline'
  })
  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin + 72, width: colW, height: ph - margin * 2 - 72 },
    threadId: bodyThread.id, threadOrder: 0, label: 'Body Text'
  })
  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin + colW + 12, y: margin + 72, width: colW, height: ph - margin * 2 - 72 },
    threadId: bodyThread.id, threadOrder: 1, label: 'Body Text'
  })

  // Pages 2-3: continued articles
  doc.pages[1].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: colW, height: ph - margin * 2 },
    threadId: bodyThread.id, threadOrder: 2, label: 'Body Text'
  })
  doc.pages[1].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin + colW + 12, y: margin, width: colW, height: ph - margin * 2 },
    threadId: bodyThread.id, threadOrder: 3, label: 'Body Text'
  })

  // Page 2 right side
  doc.pages[2].frames.push({
    type: 'image', id: generateId('frame'),
    rect: { x: margin, y: margin, width: contentW, height: 200 },
    imageAssetId: null, wrapMode: 'skip', imageFit: 'fill', label: 'Image'
  })
  doc.pages[2].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin + 212, width: contentW, height: ph - margin * 2 - 212 },
    threadId: bodyThread.id, threadOrder: 4, label: 'Body Text'
  })

  // Page 4: back page
  const contactThread = createThread({ fontFamily: 'Inter', fontSize: 10 })
  doc.threads[contactThread.id] = contactThread

  doc.pages[3].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: contentW, height: ph - margin * 2 },
    threadId: contactThread.id, threadOrder: 0, label: 'Contact Info'
  })

  return doc
}

function buildSimple2Page(): Document {
  const doc = createDocument({
    title: 'Untitled Newsletter',
    author: '',
    pageSize: PAGE_SIZES['US Letter'],
    pageCount: 2
  })

  const margin = 36
  const contentW = 612 - margin * 2
  const colW = (contentW - 12) / 2

  const thread = createThread({ fontFamily: 'Inter', fontSize: 11 })
  doc.threads[thread.id] = thread

  // Page 1
  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: colW, height: 720 },
    threadId: thread.id, threadOrder: 0, label: 'Body Text'
  })
  doc.pages[0].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin + colW + 12, y: margin, width: colW, height: 720 },
    threadId: thread.id, threadOrder: 1, label: 'Body Text'
  })

  // Page 2
  doc.pages[1].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin, y: margin, width: colW, height: 720 },
    threadId: thread.id, threadOrder: 2, label: 'Body Text'
  })
  doc.pages[1].frames.push({
    type: 'text', id: generateId('frame'),
    rect: { x: margin + colW + 12, y: margin, width: colW, height: 720 },
    threadId: thread.id, threadOrder: 3, label: 'Body Text'
  })

  return doc
}
