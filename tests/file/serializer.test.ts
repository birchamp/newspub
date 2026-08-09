// tests/file/serializer.test.ts
import { describe, it, expect } from 'vitest'
import { serializeDocument, deserializeDocument } from '@file/serializer'
import { createDocument } from '@model/document'
import { PAGE_SIZES } from '@model/page-sizes'

describe('serializer', () => {
  it('round-trips a document through JSON', () => {
    const doc = createDocument({
      title: 'Test Newsletter',
      author: 'Author',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 4
    })

    const json = serializeDocument(doc)
    const parsed = deserializeDocument(json)

    expect(parsed.metadata.title).toBe('Test Newsletter')
    expect(parsed.pages).toHaveLength(4)
    expect(parsed.metadata.pageSize).toEqual({ width: 612, height: 792 })
  })

  it('preserves thread data', () => {
    const doc = createDocument({
      title: 'Test',
      author: '',
      pageSize: PAGE_SIZES['US Letter'],
      pageCount: 2
    })
    doc.threads['thread-1'] = {
      id: 'thread-1',
      runs: [{ text: 'Hello world', style: { bold: true, fontFamily: 'Inter', fontSize: 14 } }],
      defaultStyle: { fontFamily: 'Inter', fontSize: 14 }
    }

    const json = serializeDocument(doc)
    const parsed = deserializeDocument(json)

    expect(parsed.threads['thread-1'].runs[0].text).toBe('Hello world')
    expect(parsed.threads['thread-1'].runs[0].style.bold).toBe(true)
  })
})
