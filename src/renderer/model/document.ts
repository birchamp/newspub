import type { Document, DocumentMetadata, Page, Size } from './types'

let idCounter = 0
export function generateId(prefix: string): string {
  return `${prefix}-${Date.now()}-${++idCounter}`
}

interface CreateDocumentOptions {
  title: string
  author: string
  pageSize: Size
  pageCount: number
}

export function createDocument(options: CreateDocumentOptions): Document {
  const now = new Date().toISOString()
  const pages: Page[] = Array.from({ length: options.pageCount }, () => ({
    id: generateId('page'),
    frames: [],
    backgroundImageAssetId: null
  }))

  return {
    metadata: {
      title: options.title,
      author: options.author,
      createdAt: now,
      modifiedAt: now,
      pageSize: { ...options.pageSize },
      unitPreference: 'inches'
    },
    pages,
    threads: {},
    assets: {}
  }
}
