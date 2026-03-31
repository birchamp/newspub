// ---- Geometry ----
export interface Point {
  x: number  // points
  y: number  // points
}

export interface Size {
  width: number   // points
  height: number  // points
}

export interface Rect {
  x: number
  y: number
  width: number
  height: number
}

// ---- Styled Text ----
export interface TextStyle {
  fontFamily?: string
  fontSize?: number      // points
  bold?: boolean
  italic?: boolean
  color?: string         // hex
  listType?: 'bullet' | 'numbered' | null
  indent?: number        // points
  alignment?: 'left' | 'center' | 'right' | 'justify'
  dropCap?: boolean
  pullQuote?: boolean
}

export interface StyledRun {
  text: string
  style: TextStyle
}

// ---- Frames ----
export type WrapMode = 'skip' | 'rect'
export type ImageFit = 'fill' | 'fit' | 'stretch'

export interface TextFrame {
  type: 'text'
  id: string
  rect: Rect
  threadId: string
  threadOrder: number        // position within the thread's frame list
  styleOverrides?: TextStyle // default style for this frame
  label?: string             // template hint: "Headline", "Body Text"
}

export interface ImageFrame {
  type: 'image'
  id: string
  rect: Rect
  imageAssetId: string | null  // null = empty frame
  wrapMode: WrapMode
  imageFit: ImageFit
  label?: string               // template hint: "Image"
}

export type Frame = TextFrame | ImageFrame

// ---- Pages ----
export interface Page {
  id: string
  frames: Frame[]
  backgroundImageAssetId: string | null
}

// ---- Threads ----
export interface Thread {
  id: string
  runs: StyledRun[]
  defaultStyle: TextStyle
}

// ---- Assets ----
export interface Asset {
  id: string
  filename: string
  mimeType: 'image/jpeg' | 'image/png' | 'image/webp'
  data: ArrayBuffer     // in-memory; lazy-loaded from zip
}

// ---- Document Metadata ----
export interface DocumentMetadata {
  title: string
  author: string
  createdAt: string       // ISO 8601
  modifiedAt: string      // ISO 8601
  pageSize: Size
  unitPreference: 'inches' | 'millimeters'
}

// ---- Document (top-level) ----
export interface Document {
  metadata: DocumentMetadata
  pages: Page[]
  threads: Record<string, Thread>
  assets: Record<string, Asset>
}

// ---- Template Metadata ----
export interface TemplateMetadata {
  name: string
  description: string
  pageSize: Size
  pageCount: number
  thumbnail?: string     // base64 data URI
}

export interface TemplateSaveOptions {
  keepArticleText: boolean
  keepPlacedImages: boolean
  keepBackgroundImages: boolean  // default true
}
