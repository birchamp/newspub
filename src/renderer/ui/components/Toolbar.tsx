// src/renderer/ui/components/Toolbar.tsx
import { useEditorStore } from '@ui/store/editor-store'
import {
  toggleStyleFlag, selectionStyleFlag, selectionStyleValue, applyTextStyle,
  setAlignment, undo, redo, canUndo, canRedo, zoomStep
} from '@ui/actions'

const FONT_FAMILIES = [
  'sans-serif', 'serif', 'Arial', 'Helvetica', 'Georgia',
  'Times New Roman', 'Courier New', 'monospace'
]
const FONT_SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 21, 24, 28, 32, 36, 48, 64]

export default function Toolbar() {
  const { activeTool, setActiveTool, zoom, zoomAtCenter, selection, requestFit } = useEditorStore()
  // Subscribing keeps the undo/redo buttons in sync with the history stack
  useEditorStore(s => s.historyVersion)

  const editingText = selection?.type === 'text'
  const boldActive = editingText && selectionStyleFlag('bold')
  const italicActive = editingText && selectionStyleFlag('italic')
  const fontFamily = (editingText && selectionStyleValue('fontFamily')) || 'sans-serif'
  const fontSize = (editingText && selectionStyleValue('fontSize')) || 14
  const color = (editingText && selectionStyleValue('color')) || '#000000'
  const alignment = (editingText && selectionStyleValue('alignment')) || 'left'

  // Keep focus in the canvas's hidden textarea while clicking format buttons
  const keepFocus = (e: React.MouseEvent) => e.preventDefault()

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      gap: 8,
      padding: '4px 12px',
      borderBottom: '1px solid #ddd',
      background: '#fafafa',
      fontSize: 13,
      flexShrink: 0
    }}>
      {/* Frame tools */}
      <div style={{ display: 'flex', gap: 2 }}>
        <ToolButton label="Select" active={activeTool === 'select'} onClick={() => setActiveTool('select')} title="Select and move frames (V)" />
        <ToolButton label="Text Frame" active={activeTool === 'draw-text-frame'} onClick={() => setActiveTool('draw-text-frame')} title="Drag on a page to draw a text frame (T)" />
        <ToolButton label="Image Frame" active={activeTool === 'draw-image-frame'} onClick={() => setActiveTool('draw-image-frame')} title="Drag on a page to draw an image frame" />
      </div>

      <Divider />

      {/* Undo / redo */}
      <button style={btn(false, !canUndo())} disabled={!canUndo()} onClick={undo} title="Undo (Ctrl+Z)">↩</button>
      <button style={btn(false, !canRedo())} disabled={!canRedo()} onClick={redo} title="Redo (Ctrl+Shift+Z)">↪</button>

      <Divider />

      {/* Text formatting (active when editing text) */}
      <button
        style={{ ...btn(boldActive, !editingText), fontWeight: 700 }}
        disabled={!editingText}
        onMouseDown={keepFocus}
        onClick={() => toggleStyleFlag('bold')}
        title="Bold (Ctrl+B)"
      >B</button>
      <button
        style={{ ...btn(italicActive, !editingText), fontStyle: 'italic' }}
        disabled={!editingText}
        onMouseDown={keepFocus}
        onClick={() => toggleStyleFlag('italic')}
        title="Italic (Ctrl+I)"
      >I</button>

      {/* Font family / size / color */}
      <select
        value={fontFamily}
        disabled={!editingText}
        onMouseDown={e => e.stopPropagation()}
        onChange={e => applyTextStyle({ fontFamily: e.target.value })}
        style={{ ...btn(false, !editingText), width: 120, padding: '3px 4px' }}
        title="Font family"
      >
        {FONT_FAMILIES.map(f => <option key={f} value={f}>{f}</option>)}
        {!FONT_FAMILIES.includes(fontFamily) && <option value={fontFamily}>{fontFamily}</option>}
      </select>
      <select
        value={FONT_SIZES.includes(fontSize) ? fontSize : fontSize}
        disabled={!editingText}
        onChange={e => applyTextStyle({ fontSize: parseInt(e.target.value, 10) })}
        style={{ ...btn(false, !editingText), width: 52, padding: '3px 4px' }}
        title="Font size"
      >
        {FONT_SIZES.map(s => <option key={s} value={s}>{s}</option>)}
        {!FONT_SIZES.includes(fontSize) && <option value={fontSize}>{fontSize}</option>}
      </select>
      <input
        type="color"
        value={color}
        disabled={!editingText}
        onChange={e => applyTextStyle({ color: e.target.value })}
        style={{ width: 26, height: 24, padding: 1, border: '1px solid #ddd', borderRadius: 4, background: 'transparent', cursor: editingText ? 'pointer' : 'default' }}
        title="Text color"
      />

      <Divider />

      {/* Alignment */}
      <button style={btn(alignment === 'left', !editingText)} disabled={!editingText} onMouseDown={keepFocus} onClick={() => setAlignment('left')} title="Align left">L</button>
      <button style={btn(alignment === 'center', !editingText)} disabled={!editingText} onMouseDown={keepFocus} onClick={() => setAlignment('center')} title="Align center">C</button>
      <button style={btn(alignment === 'right', !editingText)} disabled={!editingText} onMouseDown={keepFocus} onClick={() => setAlignment('right')} title="Align right">R</button>

      <div style={{ flex: 1 }} />

      {/* Zoom */}
      <button style={btn(false, false)} onClick={() => zoomStep(-1)} title="Zoom out (Ctrl+-)">−</button>
      <span style={{ color: '#666', width: 42, textAlign: 'center', fontVariantNumeric: 'tabular-nums' }}>{Math.round(zoom * 100)}%</span>
      <button style={btn(false, false)} onClick={() => zoomStep(1)} title="Zoom in (Ctrl+=)">+</button>
      <input
        type="range"
        min={25}
        max={400}
        value={Math.round(zoom * 100)}
        onChange={e => zoomAtCenter(parseInt(e.target.value, 10) / 100)}
        style={{ width: 100 }}
        title="Zoom"
      />
      <button style={btn(false, false)} onClick={requestFit} title="Fit page to window (Ctrl+0)">Fit</button>
    </div>
  )
}

function Divider() {
  return <div style={{ width: 1, height: 20, background: '#ddd' }} />
}

function ToolButton({ label, active, onClick, title }: { label: string; active: boolean; onClick: () => void; title?: string }) {
  return (
    <button
      onClick={onClick}
      title={title}
      style={{
        ...btn(active, false),
        background: active ? '#6366f1' : 'transparent',
        color: active ? '#fff' : '#333'
      }}
    >
      {label}
    </button>
  )
}

function btn(active: boolean, disabled: boolean): React.CSSProperties {
  return {
    padding: '4px 8px',
    border: '1px solid #ddd',
    borderRadius: 4,
    cursor: disabled ? 'default' : 'pointer',
    fontSize: 12,
    background: active ? '#e0e7ff' : 'transparent',
    color: disabled ? '#bbb' : '#333',
    minWidth: 26
  }
}
