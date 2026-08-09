// src/renderer/ui/components/Toolbar.tsx
import { useEditorStore } from '@ui/store/editor-store'

export default function Toolbar() {
  const { activeTool, setActiveTool, zoom, setZoom } = useEditorStore()

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      gap: 8,
      padding: '4px 12px',
      borderBottom: '1px solid #ddd',
      background: '#fafafa',
      fontSize: 13
    }}>
      {/* Frame tools */}
      <div style={{ display: 'flex', gap: 2 }}>
        <ToolButton label="Select" active={activeTool === 'select'} onClick={() => setActiveTool('select')} />
        <ToolButton label="Text Frame" active={activeTool === 'draw-text-frame'} onClick={() => setActiveTool('draw-text-frame')} />
        <ToolButton label="Image Frame" active={activeTool === 'draw-image-frame'} onClick={() => setActiveTool('draw-image-frame')} />
      </div>

      <div style={{ width: 1, height: 20, background: '#ddd' }} />

      {/* Text formatting (active when editing text) */}
      <button style={btnStyle}>B</button>
      <button style={btnStyle}>I</button>

      <div style={{ flex: 1 }} />

      {/* Zoom */}
      <span style={{ color: '#666' }}>{Math.round(zoom * 100)}%</span>
      <input
        type="range"
        min={25}
        max={400}
        value={zoom * 100}
        onChange={e => setZoom(parseInt(e.target.value) / 100)}
        style={{ width: 100 }}
      />
    </div>
  )
}

function ToolButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        ...btnStyle,
        background: active ? '#6366f1' : 'transparent',
        color: active ? '#fff' : '#333'
      }}
    >
      {label}
    </button>
  )
}

const btnStyle: React.CSSProperties = {
  padding: '4px 8px',
  border: '1px solid #ddd',
  borderRadius: 4,
  cursor: 'pointer',
  fontSize: 12,
  background: 'transparent'
}
