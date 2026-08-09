// src/renderer/ui/components/DocumentSetupDialog.tsx
import { useState } from 'react'
import { PAGE_SIZES } from '@model/page-sizes'
import type { Size } from '@model/types'

interface Props {
  currentSize: Size
  onApply: (size: Size, scaleFrames: boolean) => void
  onCancel: () => void
}

export default function DocumentSetupDialog({ currentSize, onApply, onCancel }: Props) {
  const [selectedPreset, setSelectedPreset] = useState<string | 'custom'>(
    Object.entries(PAGE_SIZES).find(
      ([_, s]) => s.width === currentSize.width && s.height === currentSize.height
    )?.[0] ?? 'custom'
  )
  const [customW, setCustomW] = useState(currentSize.width)
  const [customH, setCustomH] = useState(currentSize.height)
  const [scaleFrames, setScaleFrames] = useState(true)

  const resolvedSize = selectedPreset === 'custom'
    ? { width: customW, height: customH }
    : PAGE_SIZES[selectedPreset]

  return (
    <div style={overlayStyle}>
      <div style={dialogStyle}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>Document Setup</h2>

        <label style={labelStyle}>Page Size</label>
        <select
          value={selectedPreset}
          onChange={e => setSelectedPreset(e.target.value)}
          style={{ ...inputStyle, cursor: 'pointer' }}
        >
          {Object.keys(PAGE_SIZES).map(name => (
            <option key={name} value={name}>{name}</option>
          ))}
          <option value="custom">Custom</option>
        </select>

        {selectedPreset === 'custom' && (
          <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
            <div>
              <label style={labelStyle}>Width (pt)</label>
              <input type="number" value={customW} onChange={e => setCustomW(Number(e.target.value))} style={inputStyle} />
            </div>
            <div>
              <label style={labelStyle}>Height (pt)</label>
              <input type="number" value={customH} onChange={e => setCustomH(Number(e.target.value))} style={inputStyle} />
            </div>
          </div>
        )}

        <div style={{ marginTop: 12 }}>
          <label style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13, cursor: 'pointer' }}>
            <input type="checkbox" checked={scaleFrames} onChange={e => setScaleFrames(e.target.checked)} />
            Scale frame positions proportionally
          </label>
        </div>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 16 }}>
          <button onClick={onCancel} style={cancelBtnStyle}>Cancel</button>
          <button onClick={() => onApply(resolvedSize, scaleFrames)} style={primaryBtnStyle}>Apply</button>
        </div>
      </div>
    </div>
  )
}

const overlayStyle: React.CSSProperties = { position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }
const dialogStyle: React.CSSProperties = { background: '#fff', borderRadius: 12, padding: 24, maxWidth: 380, width: '90%', boxShadow: '0 8px 32px rgba(0,0,0,0.2)' }
const labelStyle: React.CSSProperties = { display: 'block', fontSize: 11, color: '#888', marginBottom: 4, marginTop: 8, textTransform: 'uppercase', letterSpacing: 1 }
const inputStyle: React.CSSProperties = { width: '100%', padding: '8px 10px', border: '1px solid #ddd', borderRadius: 6, fontSize: 13, boxSizing: 'border-box' }
const primaryBtnStyle: React.CSSProperties = { padding: '8px 16px', background: '#6366f1', color: '#fff', border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13 }
const cancelBtnStyle: React.CSSProperties = { padding: '8px 16px', background: 'transparent', color: '#666', border: '1px solid #ddd', borderRadius: 6, cursor: 'pointer', fontSize: 13 }
