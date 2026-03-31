import { describe, it, expect } from 'vitest'
import { PAGE_SIZES } from '@model/page-sizes'

describe('PAGE_SIZES', () => {
  it('contains US Letter at 612x792 points', () => {
    expect(PAGE_SIZES['US Letter']).toEqual({ width: 612, height: 792 })
  })

  it('contains US Legal at 612x1008 points', () => {
    expect(PAGE_SIZES['US Legal']).toEqual({ width: 612, height: 1008 })
  })

  it('contains US Tabloid at 792x1224 points', () => {
    expect(PAGE_SIZES['US Tabloid']).toEqual({ width: 792, height: 1224 })
  })

  it('contains A4 at 595x842 points', () => {
    expect(PAGE_SIZES['A4']).toEqual({ width: 595, height: 842 })
  })

  it('contains A5 at 420x595 points', () => {
    expect(PAGE_SIZES['A5']).toEqual({ width: 420, height: 595 })
  })
})
