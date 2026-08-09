// src/renderer/ui/components/TemplatePickerDialog.tsx
import { useState } from 'react'
import { presetTemplates } from '@template/preset-templates'
import type { Document } from '@model/types'

interface Props {
  onSelect: (doc: Document) => void
  onCancel: () => void
}

export default function TemplatePickerDialog({ onSelect, onCancel }: Props) {
  const [selected, setSelected] = useState(0)

  return (
    <div style={overlayStyle}>
      <div style={dialogStyle}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>New Document</h2>
        <p style={{ color: '#666', marginBottom: 16, fontSize: 13 }}>Choose a template to get started</p>

        <div style={{ display: 'flex', gap: 12, marginBottom: 24 }}>
          {presetTemplates.map((tmpl, i) => (
            <div
              key={i}
              onClick={() => setSelected(i)}
              style={{
                padding: 12,
                border: selected === i ? '2px solid #6366f1' : '2px solid #ddd',
                borderRadius: 8,
                cursor: 'pointer',
                width: 160,
                textAlign: 'center'
              }}
            >
              <div style={{
                width: '100%',
                height: 100,
                background: '#f5f5f5',
                borderRadius: 4,
                marginBottom: 8
              }} />
              <div style={{ fontWeight: 600, fontSize: 13 }}>{tmpl.metadata.name}</div>
              <div style={{ color: '#888', fontSize: 11, marginTop: 4 }}>{tmpl.metadata.description}</div>
            </div>
          ))}
        </div>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          <button onClick={onCancel} style={cancelBtnStyle}>Cancel</button>
          <button onClick={() => onSelect(presetTemplates[selected].build())} style={primaryBtnStyle}>
            Create
          </button>
        </div>
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
  maxWidth: 500, width: '90%', boxShadow: '0 8px 32px rgba(0,0,0,0.2)'
}

const primaryBtnStyle: React.CSSProperties = {
  padding: '8px 16px', background: '#6366f1', color: '#fff',
  border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13
}

const cancelBtnStyle: React.CSSProperties = {
  padding: '8px 16px', background: 'transparent', color: '#666',
  border: '1px solid #ddd', borderRadius: 6, cursor: 'pointer', fontSize: 13
}
