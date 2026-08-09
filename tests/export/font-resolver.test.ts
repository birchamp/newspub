// tests/export/font-resolver.test.ts
import { describe, it, expect, vi } from 'vitest'
import { getStandardFontName, isStandardFont } from '@export/font-resolver'

describe('font-resolver', () => {
  it('maps Helvetica to a standard PDF font', () => {
    expect(isStandardFont('Helvetica')).toBe(true)
    expect(getStandardFontName('Helvetica')).toBe('Helvetica')
  })

  it('maps Times New Roman to Times-Roman', () => {
    expect(isStandardFont('Times New Roman')).toBe(true)
    expect(getStandardFontName('Times New Roman')).toBe('Times-Roman')
  })

  it('reports non-standard fonts', () => {
    expect(isStandardFont('Inter')).toBe(false)
  })
})
