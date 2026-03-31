export interface PageDimensions {
  width: number   // points (1/72 inch)
  height: number  // points
}

export const PAGE_SIZES: Record<string, PageDimensions> = {
  'US Letter': { width: 612, height: 792 },
  'US Legal': { width: 612, height: 1008 },
  'US Tabloid': { width: 792, height: 1224 },
  'A4': { width: 595, height: 842 },
  'A5': { width: 420, height: 595 }
}
