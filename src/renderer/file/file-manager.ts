// src/renderer/file/file-manager.ts
import { zipSync, unzipSync, strToU8, strFromU8 } from 'fflate'
import type { Document, Asset } from '@model/types'
import { serializeDocument, deserializeDocument } from './serializer'

export async function packNewspub(doc: Document): Promise<ArrayBuffer> {
  const files: Record<string, Uint8Array> = {}

  // document.json
  const docJson = serializeDocument(doc)
  files['document.json'] = strToU8(docJson)

  // metadata.json
  const metadata = {
    title: doc.metadata.title,
    author: doc.metadata.author,
    createdAt: doc.metadata.createdAt,
    modifiedAt: new Date().toISOString(),
    pageSize: doc.metadata.pageSize,
    version: '1.0.0'
  }
  files['metadata.json'] = strToU8(JSON.stringify(metadata, null, 2))

  // Assets
  for (const [id, asset] of Object.entries(doc.assets)) {
    const path = `assets/${asset.filename}`
    files[path] = new Uint8Array(asset.data)
  }

  // Zip with appropriate compression
  const zipOptions: Record<string, { level: number }> = {}
  for (const key of Object.keys(files)) {
    if (key.endsWith('.json')) {
      zipOptions[key] = { level: 6 } // compress JSON
    } else {
      zipOptions[key] = { level: 0 } // store images as-is (already compressed)
    }
  }

  const zipped = zipSync(files, { level: 0 })
  return zipped.buffer
}

export async function unpackNewspub(buffer: ArrayBuffer): Promise<Document> {
  const data = new Uint8Array(buffer)
  const unzipped = unzipSync(data)

  // Parse document.json
  const docJson = strFromU8(unzipped['document.json'])
  const doc = deserializeDocument(docJson)

  // Load assets
  for (const [path, fileData] of Object.entries(unzipped)) {
    if (!path.startsWith('assets/')) continue
    const filename = path.slice('assets/'.length)

    // Find matching asset in manifest
    const assetEntry = Object.values(doc.assets).find(a => a.filename === filename)

    // Determine mime type from extension
    const ext = filename.split('.').pop()?.toLowerCase()
    const mimeMap: Record<string, Asset['mimeType']> = {
      jpg: 'image/jpeg',
      jpeg: 'image/jpeg',
      png: 'image/png',
      webp: 'image/webp'
    }
    const mimeType = mimeMap[ext ?? ''] ?? 'image/jpeg'

    // Find asset ID from the serialized manifest
    const manifestJson = docJson
    const manifest = JSON.parse(manifestJson)
    const manifestEntry = manifest.assetManifest?.find(
      (a: { filename: string }) => a.filename === filename
    )

    if (manifestEntry) {
      doc.assets[manifestEntry.id] = {
        id: manifestEntry.id,
        filename,
        mimeType,
        data: fileData.buffer
      }
    }
  }

  return doc
}
