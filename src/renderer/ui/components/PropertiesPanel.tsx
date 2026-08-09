// src/renderer/ui/components/PropertiesPanel.tsx
import { useEditorStore } from '@ui/store/editor-store'
import {
  setFrameGeometry, duplicateFrame, reorderFrame, deleteFrame,
  chooseImageForFrame, setImageFrameOption
} from '@ui/actions'
import type { TextFrame, ImageFrame, Rect } from '@model/types'

export default function PropertiesPanel() {
  const { document: doc, selection } = useEditorStore()

  if (!doc) return <Panel>No document open</Panel>
  if (!selection) return <Panel><DocumentProperties doc={doc} /></Panel>

  const frameSel = selection.type === 'frame'
    ? selection
    : { pageId: selection.pageId, frameId: selection.frameId }
  const page = doc.pages.find(p => p.id === frameSel.pageId)
  const frame = page?.frames.find(f => f.id === frameSel.frameId)
  if (!frame) return <Panel>Frame not found</Panel>

  return (
    <Panel>
      {frame.type === 'text'
        ? <TextFrameProperties frame={frame as TextFrame} pageId={frameSel.pageId} />
        : <ImageFrameProperties frame={frame as ImageFrame} pageId={frameSel.pageId} doc={doc} />}
      <FrameGeometry frame={frame} pageId={frameSel.pageId} />
      <FrameActions pageId={frameSel.pageId} frameId={frame.id} />
    </Panel>
  )
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
      <div style={{ marginTop: 16, color: '#999', lineHeight: 1.5 }}>
        Select a frame to edit its position and content. Double-click a text
        frame to edit text, or an image frame to choose its image. Drag an
        image file onto a page to place it.
      </div>
    </div>
  )
}

function FrameGeometry({ frame, pageId }: { frame: { id: string; rect: Rect }; pageId: string }) {
  const fields: Array<{ label: string; key: keyof Rect }> = [
    { label: 'X', key: 'x' },
    { label: 'Y', key: 'y' },
    { label: 'W', key: 'width' },
    { label: 'H', key: 'height' }
  ]
  return (
    <div style={{ marginTop: 12 }}>
      <SectionLabel>Position &amp; Size</SectionLabel>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 6 }}>
        {fields.map(({ label, key }) => (
          <label key={key} style={{ display: 'flex', alignItems: 'center', gap: 4, color: '#888' }}>
            {label}
            <input
              type="number"
              value={Math.round(frame.rect[key])}
              onChange={e => {
                const v = parseFloat(e.target.value)
                if (!Number.isFinite(v)) return
                setFrameGeometry(pageId, frame.id, { [key]: v }, key)
              }}
              style={numInput}
            />
          </label>
        ))}
      </div>
    </div>
  )
}

function FrameActions({ pageId, frameId }: { pageId: string; frameId: string }) {
  return (
    <div style={{ marginTop: 12 }}>
      <SectionLabel>Arrange</SectionLabel>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 6 }}>
        <button style={smallBtn} onClick={() => reorderFrame(pageId, frameId, 'front')} title="Ctrl+]">Bring Front</button>
        <button style={smallBtn} onClick={() => reorderFrame(pageId, frameId, 'back')} title="Ctrl+[">Send Back</button>
        <button style={smallBtn} onClick={() => duplicateFrame(pageId, frameId)} title="Ctrl+D">Duplicate</button>
        <button style={{ ...smallBtn, color: '#dc2626' }} onClick={() => deleteFrame(pageId, frameId)} title="Delete">Delete</button>
      </div>
    </div>
  )
}

function TextFrameProperties({ frame, pageId }: { frame: TextFrame; pageId: string }) {
  void pageId
  return (
    <div>
      <SectionLabel>Text Frame</SectionLabel>
      <PropRow label="Thread" value={frame.threadId.slice(0, 18)} />
      <PropRow label="Order" value={String(frame.threadOrder)} />
      <div style={{ marginTop: 8, color: '#999', lineHeight: 1.4 }}>
        Double-click (or press Enter) to edit text.
      </div>
    </div>
  )
}

function ImageFrameProperties({ frame, pageId, doc }: { frame: ImageFrame; pageId: string; doc: import('@model/types').Document }) {
  const asset = frame.imageAssetId ? doc.assets[frame.imageAssetId] : null
  return (
    <div>
      <SectionLabel>Image Frame</SectionLabel>
      <PropRow label="Image" value={asset ? asset.filename : '(empty)'} />
      <button
        style={{ ...smallBtn, width: '100%', marginTop: 6 }}
        onClick={() => { void chooseImageForFrame(pageId, frame.id) }}
      >
        {asset ? 'Replace Image…' : 'Choose Image…'}
      </button>
      <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: 6, alignItems: 'center', marginTop: 8 }}>
        <span style={{ color: '#888' }}>Fit</span>
        <select
          value={frame.imageFit}
          onChange={e => setImageFrameOption(pageId, frame.id, { imageFit: e.target.value as ImageFrame['imageFit'] })}
          style={selInput}
        >
          <option value="fit">Fit (contain)</option>
          <option value="fill">Fill (cover)</option>
          <option value="stretch">Stretch</option>
        </select>
        <span style={{ color: '#888' }}>Wrap</span>
        <select
          value={frame.wrapMode}
          onChange={e => setImageFrameOption(pageId, frame.id, { wrapMode: e.target.value as ImageFrame['wrapMode'] })}
          style={selInput}
        >
          <option value="rect">Wrap text around</option>
          <option value="skip">Skip lines</option>
        </select>
      </div>
    </div>
  )
}

function SectionLabel({ children }: { children: React.ReactNode }) {
  return <div style={{ fontWeight: 600, color: '#666', marginBottom: 8, textTransform: 'uppercase', letterSpacing: 1, fontSize: 10 }}>{children}</div>
}

function PropRow({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 4 }}>
      <span style={{ color: '#888' }}>{label}</span>
      <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 120 }}>{value}</span>
    </div>
  )
}

const numInput: React.CSSProperties = {
  width: '100%',
  minWidth: 0,
  padding: '3px 4px',
  border: '1px solid #ddd',
  borderRadius: 4,
  fontSize: 12
}

const selInput: React.CSSProperties = {
  padding: '3px 4px',
  border: '1px solid #ddd',
  borderRadius: 4,
  fontSize: 12,
  background: '#fff'
}

const smallBtn: React.CSSProperties = {
  padding: '4px 6px',
  border: '1px solid #ddd',
  borderRadius: 4,
  cursor: 'pointer',
  fontSize: 11,
  background: '#fff'
}
