// src/renderer/file/serializer.ts
import type { Document } from '@model/types'

interface SerializedDocument {
  metadata: Document['metadata']
  pages: Array<{
    id: string
    frames: Document['pages'][number]['frames']
    backgroundImageAssetId: string | null
  }>
  threads: Record<string, {
    id: string
    runs: Array<{ text: string; style: Record<string, unknown> }>
    defaultStyle: Record<string, unknown>
  }>
  // Assets are stored separately in the zip, not in JSON
  assetManifest: Array<{
    id: string
    filename: string
    mimeType: string
  }>
}

export function serializeDocument(doc: Document): string {
  const serialized: SerializedDocument = {
    metadata: doc.metadata,
    pages: doc.pages.map(p => ({
      id: p.id,
      frames: p.frames,
      backgroundImageAssetId: p.backgroundImageAssetId
    })),
    threads: Object.fromEntries(
      Object.entries(doc.threads).map(([id, thread]) => [
        id,
        {
          id: thread.id,
          runs: thread.runs.map(r => ({
            text: r.text,
            style: r.style as Record<string, unknown>
          })),
          defaultStyle: thread.defaultStyle as Record<string, unknown>
        }
      ])
    ),
    assetManifest: Object.values(doc.assets).map(a => ({
      id: a.id,
      filename: a.filename,
      mimeType: a.mimeType
    }))
  }

  return JSON.stringify(serialized, null, 2)
}

export function deserializeDocument(json: string): Document {
  const data: SerializedDocument = JSON.parse(json)

  return {
    metadata: data.metadata,
    pages: data.pages.map(p => ({
      id: p.id,
      frames: p.frames,
      backgroundImageAssetId: p.backgroundImageAssetId
    })),
    threads: Object.fromEntries(
      Object.entries(data.threads).map(([id, thread]) => [
        id,
        {
          id: thread.id,
          runs: thread.runs.map(r => ({
            text: r.text,
            style: r.style
          })),
          defaultStyle: thread.defaultStyle
        }
      ])
    ) as Document['threads'],
    assets: {} // Assets loaded separately from zip
  }
}
