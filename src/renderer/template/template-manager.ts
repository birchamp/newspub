// src/renderer/template/template-manager.ts
import type { Document, TemplateSaveOptions, ImageFrame, TemplateMetadata } from '@model/types'

export function stripToTemplate(
  doc: Document,
  options: TemplateSaveOptions
): Document {
  const template = structuredClone(doc)
  const usedAssetIds = new Set<string>()

  // Strip or keep thread content
  for (const thread of Object.values(template.threads)) {
    if (!options.keepArticleText) {
      thread.runs = []
    }
  }

  // Strip or keep placed images
  for (const page of template.pages) {
    for (let i = 0; i < page.frames.length; i++) {
      const frame = page.frames[i]
      if (frame.type === 'image') {
        const imgFrame = frame as ImageFrame
        if (options.keepPlacedImages && imgFrame.imageAssetId) {
          usedAssetIds.add(imgFrame.imageAssetId)
        } else {
          imgFrame.imageAssetId = null
        }
      }
    }

    // Background images
    if (options.keepBackgroundImages && page.backgroundImageAssetId) {
      usedAssetIds.add(page.backgroundImageAssetId)
    } else {
      page.backgroundImageAssetId = null
    }
  }

  // Remove unused assets
  const newAssets: typeof template.assets = {}
  for (const [id, asset] of Object.entries(template.assets)) {
    if (usedAssetIds.has(id)) {
      newAssets[id] = asset
    }
  }
  template.assets = newAssets

  return template
}

export function getTemplateMetadata(doc: Document, name: string, description: string): TemplateMetadata {
  return {
    name,
    description,
    pageSize: { ...doc.metadata.pageSize },
    pageCount: doc.pages.length
  }
}

export function applyTemplate(template: Document): Document {
  // Create a fresh document from the template
  const doc = structuredClone(template)
  const now = new Date().toISOString()
  doc.metadata.createdAt = now
  doc.metadata.modifiedAt = now
  return doc
}
