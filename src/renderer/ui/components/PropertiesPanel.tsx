// src/renderer/ui/components/PropertiesPanel.tsx
import { useEditorStore } from '@ui/store/editor-store'
import type { Frame, TextFrame, ImageFrame } from '@model/types'

export default function PropertiesPanel() {
  const { document: doc, selection } = useEditorStore()

  if (!doc) return <Panel>No document open</Panel>
  if (!selection) return <Panel><DocumentProperties doc={doc} /></Panel>
  if (selection.type === 'frame') {
    const page = doc.pages.find(p => p.id === selection.pageId)
    const frame = page?.frames.find(f => f.id === selection.frameId)
    if (!frame) return <Panel>Frame not found</Panel>

    if (frame.type === 'text') return <Panel><TextFrameProperties frame={frame as TextFrame} /></Panel>
    if (frame.type === 'image') return <Panel><ImageFrameProperties frame={frame as ImageFrame} /></Panel>
  }

  return <Panel>Select an element</Panel>
}

function Panel({ children }: { children: React.ReactNode }) {
  return (
    <div style={{
      width: 220,
      borderLeft: '1px solid #ddd',
      padding: 12,
      overflowY: 'auto',
      background: '#f8f8f8',
      flexShrink: 0,
      fontSize: 12
    }}>
      {children}
    </div>
  )
}

function DocumentProperties({ doc }: { doc: import('@model/types').Document }) {
  return (
    <div>
      <SectionLabel>Document</SectionLabel>
      <PropRow label="Title" value={doc.metadata.title} />
      <PropRow label="Pages" value={String(doc.pages.length)} />
      <PropRow label="Size" value={`${doc.metadata.pageSize.width} × ${doc.metadata.pageSize.height} pt`} />
    </div>
  )
}

function TextFrameProperties({ frame }: { frame: TextFrame }) {
  return (
    <div>
      <SectionLabel>Text Frame</SectionLabel>
      <PropRow label="X" value={String(Math.round(frame.rect.x))} />
      <PropRow label="Y" value={String(Math.round(frame.rect.y))} />
      <PropRow label="Width" value={String(Math.round(frame.rect.width))} />
      <PropRow label="Height" value={String(Math.round(frame.rect.height))} />
      <PropRow label="Thread" value={frame.threadId} />
      <PropRow label="Order" value={String(frame.threadOrder)} />
    </div>
  )
}

function ImageFrameProperties({ frame }: { frame: ImageFrame }) {
  return (
    <div>
      <SectionLabel>Image Frame</SectionLabel>
      <PropRow label="X" value={String(Math.round(frame.rect.x))} />
      <PropRow label="Y" value={String(Math.round(frame.rect.y))} />
      <PropRow label="Width" value={String(Math.round(frame.rect.width))} />
      <PropRow label="Height" value={String(Math.round(frame.rect.height))} />
      <PropRow label="Wrap" value={frame.wrapMode} />
      <PropRow label="Fit" value={frame.imageFit} />
    </div>
  )
}

function SectionLabel({ children }: { children: string }) {
  return <div style={{ fontWeight: 600, color: '#666', marginBottom: 8, textTransform: 'uppercase', letterSpacing: 1, fontSize: 10 }}>{children}</div>
}

function PropRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
      <span style={{ color: '#888' }}>{label}</span>
      <span>{value}</span>
    </div>
  )
}
