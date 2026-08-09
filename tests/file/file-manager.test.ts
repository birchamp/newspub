// tests/file/file-manager.test.ts
import { describe, it, expect } from 'vitest'
import { packNewspub, unpackNewspub } from '@file/file-manager'
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'

describe('file-manager', () => {
  it('round-trips a document through .newspub zip format', async () => {
    const doc = createDocument({
      title: 'Zip Test',
      author: 'Test',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 4
    })
    doc.threads['t1'] = {
      id: 't1',
      runs: [{ text: 'Article text', style: { fontFamily: 'Inter', fontSize: 14 } }],
      defaultStyle: { fontFamily: 'Inter', fontSize: 14 }
    }

    const zipBuffer = await packNewspub(doc)
    expect(zipBuffer.byteLength).toBeGreaterThan(0)

    const restored = await unpackNewspub(zipBuffer)
    expect(restored.metadata.title).toBe('Zip Test')
    expect(restored.pages).toHaveLength(4)
    expect(restored.threads['t1'].runs[0].text).toBe('Article text')
  })

  it('includes assets in the zip', async () => {
    const doc = createDocument({
      title: 'Asset Test',
      author: '',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 1
    })
    // Add a fake image asset
    const fakeImage = new Uint8Array([0xFF, 0xD8, 0xFF, 0xE0]).buffer
    doc.assets['img-1'] = {
      id: 'img-1',
      filename: 'photo.jpg',
      mimeType: 'image/jpeg',
      data: fakeImage
    }

    const zipBuffer = await packNewspub(doc)
    const restored = await unpackNewspub(zipBuffer)

    expect(restored.assets['img-1']).toBeDefined()
    expect(restored.assets['img-1'].filename).toBe('photo.jpg')
    expect(restored.assets['img-1'].data.byteLength).toBe(4)
  })
})
