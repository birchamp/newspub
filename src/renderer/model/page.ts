import type {
  Document, Page, TextFrame, ImageFrame, Rect, WrapMode, ImageFit
} from './types'
import { generateId } from './document'

interface AddTextFrameOptions {
  rect: Rect
  threadId: string
  threadOrder: number
  label?: string
}

export function addTextFrame(
  doc: Document,
  pageId: string,
  options: AddTextFrameOptions
): Document {
  const frame: TextFrame = {
    type: 'text',
    id: generateId('frame'),
    rect: { ...options.rect },
    threadId: options.threadId,
    threadOrder: options.threadOrder,
    label: options.label
  }

  return updatePage(doc, pageId, page => ({
    ...page,
    frames: [...page.frames, frame]
  }))
}

interface AddImageFrameOptions {
  rect: Rect
  wrapMode: WrapMode
  imageFit: ImageFit
  imageAssetId?: string | null
  label?: string
}

export function addImageFrame(
  doc: Document,
  pageId: string,
  options: AddImageFrameOptions
): Document {
  const frame: ImageFrame = {
    type: 'image',
    id: generateId('frame'),
    rect: { ...options.rect },
    imageAssetId: options.imageAssetId ?? null,
    wrapMode: options.wrapMode,
    imageFit: options.imageFit,
    label: options.label
  }

  return updatePage(doc, pageId, page => ({
    ...page,
    frames: [...page.frames, frame]
  }))
}

export function removeFrame(
  doc: Document,
  pageId: string,
  frameId: string
): Document {
  return updatePage(doc, pageId, page => ({
    ...page,
    frames: page.frames.filter(f => f.id !== frameId)
  }))
}

export function addSpread(doc: Document, afterIndex: number): Document {
  const newPages: Page[] = [
    { id: generateId('page'), frames: [], backgroundImageAssetId: null },
    { id: generateId('page'), frames: [], backgroundImageAssetId: null }
  ]
  const pages = [...doc.pages]
  pages.splice(afterIndex, 0, ...newPages)
  return { ...doc, pages }
}

export function removePage(doc: Document, pageId: string): Document {
  return { ...doc, pages: doc.pages.filter(p => p.id !== pageId) }
}

function updatePage(
  doc: Document,
  pageId: string,
  updater: (page: Page) => Page
): Document {
  return {
    ...doc,
    pages: doc.pages.map(p => p.id === pageId ? updater(p) : p)
  }
}
