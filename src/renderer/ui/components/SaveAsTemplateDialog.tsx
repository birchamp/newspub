// src/renderer/ui/components/SaveAsTemplateDialog.tsx
import { useState } from 'react'
import type { TemplateSaveOptions } from '@model/types'

interface Props {
  defaultTitle: string
  onSave: (name: string, description: string, options: TemplateSaveOptions) => void
  onCancel: () => void
}

export default function SaveAsTemplateDialog({ defaultTitle, onSave, onCancel }: Props) {
  const [name, setName] = useState(defaultTitle)
  const [description, setDescription] = useState('')
  const [keepText, setKeepText] = useState(false)
  const [keepImages, setKeepImages] = useState(false)
  const [keepBackground, setKeepBackground] = useState(true)

  return (
    <div style={overlayStyle}>
      <div style={dialogStyle}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>Save As Template</h2>

        <label style={labelStyle}>Template Name</label>
        <input value={name} onChange={e => setName(e.target.value)} style={inputStyle} />

        <label style={labelStyle}>Description</label>
        <input value={description} onChange={e => setDescription(e.target.value)} style={inputStyle} />

        <div style={{ borderTop: '1px solid #eee', paddingTop: 12, marginTop: 8 }}>
          <label style={labelStyle}>Content to Keep</label>

          <Checkbox checked={keepText} onChange={setKeepText} label="Article text" description="Keep text content in all text frames" />
          <Checkbox checked={keepImages} onChange={setKeepImages} label="Placed images" description="Keep images in image frames" />
          <Checkbox checked={keepBackground} onChange={setKeepBackground} label="Background & decoration images" description="Keep page backgrounds, borders, banners" />
        </div>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 16 }}>
          <button onClick={onCancel} style={cancelBtnStyle}>Cancel</button>
          <button
            onClick={() => onSave(name, description, {
              keepArticleText: keepText,
              keepPlacedImages: keepImages,
              keepBackgroundImages: keepBackground
            })}
            style={primaryBtnStyle}
          >
            Save Template
          </button>
        </div>
      </div>
    </div>
  )
}

function Checkbox({ checked, onChange, label, description }: {
  checked: boolean; onChange: (v: boolean) => void; label: string; description: string
}) {
  return (
    <div
      onClick={() => onChange(!checked)}
      style={{
        display: 'flex', gap: 10, padding: 8, marginBottom: 6,
        borderRadius: 6, cursor: 'pointer',
        background: checked ? 'rgba(99,102,241,0.08)' : '#f8f8f8',
        border: checked ? '1px solid rgba(99,102,241,0.2)' : '1px solid transparent'
      }}
    >
      <div style={{
        width: 18, height: 18, borderRadius: 4, flexShrink: 0,
        background: checked ? '#6366f1' : 'transparent',
        border: checked ? 'none' : '2px solid #ccc',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        color: '#fff', fontSize: 12, fontWeight: 'bold'
      }}>
        {checked && '✓'}
      </div>
      <div>
        <div style={{ fontWeight: 600, fontSize: 13 }}>{label}</div>
        <div style={{ fontSize: 11, color: '#888', marginTop: 2 }}>{description}</div>
      </div>
    </div>
  )
}

const overlayStyle: React.CSSProperties = {
  position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)',
  display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000
}
const dialogStyle: React.CSSProperties = {
  background: '#fff', borderRadius: 12, padding: 24,
  maxWidth: 420, width: '90%', boxShadow: '0 8px 32px rgba(0,0,0,0.2)'
}
const labelStyle: React.CSSProperties = {
  display: 'block', fontSize: 11, color: '#888', marginBottom: 4, marginTop: 12,
  textTransform: 'uppercase', letterSpacing: 1
}
const inputStyle: React.CSSProperties = {
  width: '100%', padding: '8px 10px', border: '1px solid #ddd', borderRadius: 6,
  fontSize: 13, boxSizing: 'border-box'
}
const primaryBtnStyle: React.CSSProperties = {
  padding: '8px 16px', background: '#6366f1', color: '#fff',
  border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13
}
const cancelBtnStyle: React.CSSProperties = {
  padding: '8px 16px', background: 'transparent', color: '#666',
  border: '1px solid #ddd', borderRadius: 6, cursor: 'pointer', fontSize: 13
}
