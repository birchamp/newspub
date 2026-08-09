// src/renderer/export/font-resolver.ts
import { StandardFonts } from 'pdf-lib'

const STANDARD_FONT_MAP: Record<string, string> = {
  'Helvetica': StandardFonts.Helvetica,
  'Arial': StandardFonts.Helvetica,
  'sans-serif': StandardFonts.Helvetica,
  'Times New Roman': StandardFonts.TimesRoman,
  'Times': StandardFonts.TimesRoman,
  'Georgia': StandardFonts.TimesRoman,
  'serif': StandardFonts.TimesRoman,
  'Courier New': StandardFonts.Courier,
  'Courier': StandardFonts.Courier,
  'monospace': StandardFonts.Courier
}

const BOLD_MAP: Record<string, string> = {
  [StandardFonts.Helvetica]: StandardFonts.HelveticaBold,
  [StandardFonts.TimesRoman]: StandardFonts.TimesRomanBold,
  [StandardFonts.Courier]: StandardFonts.CourierBold
}

const ITALIC_MAP: Record<string, string> = {
  [StandardFonts.Helvetica]: StandardFonts.HelveticaOblique,
  [StandardFonts.TimesRoman]: StandardFonts.TimesRomanItalic,
  [StandardFonts.Courier]: StandardFonts.CourierOblique
}

const BOLD_ITALIC_MAP: Record<string, string> = {
  [StandardFonts.Helvetica]: StandardFonts.HelveticaBoldOblique,
  [StandardFonts.TimesRoman]: StandardFonts.TimesRomanBoldItalic,
  [StandardFonts.Courier]: StandardFonts.CourierBoldOblique
}

export function isStandardFont(family: string): boolean {
  return family in STANDARD_FONT_MAP
}

export function getStandardFontName(family: string): string {
  return STANDARD_FONT_MAP[family] ?? StandardFonts.Helvetica
}

export function resolveStandardFont(
  family: string,
  bold: boolean,
  italic: boolean
): string {
  const base = getStandardFontName(family)
  if (bold && italic) return BOLD_ITALIC_MAP[base] ?? base
  if (bold) return BOLD_MAP[base] ?? base
  if (italic) return ITALIC_MAP[base] ?? base
  return base
}

export interface FontResolutionResult {
  fontName: string
  isEmbedded: boolean
  warnings: string[]
}

export function resolveFont(
  family: string,
  bold: boolean,
  italic: boolean
): FontResolutionResult {
  if (isStandardFont(family)) {
    return {
      fontName: resolveStandardFont(family, bold, italic),
      isEmbedded: false,
      warnings: []
    }
  }

  // For v1: fall back to standard fonts with a warning
  // Future: resolve system font files and embed them
  const fallback = resolveStandardFont('Helvetica', bold, italic)
  return {
    fontName: fallback,
    isEmbedded: false,
    warnings: [`Font "${family}" is not available for PDF embedding. Falling back to Helvetica.`]
  }
}
