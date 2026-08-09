// src/renderer/ui/components/ExportDialog.tsx
import { useState } from 'react'

interface Props {
  pageCount: number
  onExport: (options: { allPages: boolean; startPage: number; endPage: number }) => void
  onCancel: () => void
}

export default function ExportDialog({ pageCount, onExport, onCancel }: Props) {
  const [allPages, setAllPages] = useState(true)
  const [startPage, setStartPage] = useState(1)
  const [endPage, setEndPage] = useState(pageCount)

  return (
    <div style={overlayStyle}>
      <div style={dialogStyle}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: 18 }}>Export PDF</h2>

        <label style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13, cursor: 'pointer', marginBottom: 8 }}>
          <input type="radio" checked={allPages} onChange={() => setAllPages(true)} />
          All pages ({pageCount})
        </label>
        <label style={{ display: 'flex', gap: 8, alignItems: 'center', fontSize: 13, cursor: 'pointer' }}>
          <input type="radio" checked={!allPages} onChange={() => setAllPages(false)} />
          Pages
          <input type="number" min={1} max={pageCount} value={startPage} onChange={e => setStartPage(Number(e.target.value))} disabled={allPages} style={{ width: 50, padding: '4px 6px', border: '1px solid #ddd', borderRadius: 4 }} />
          to
          <input type="number" min={1} max={pageCount} value={endPage} onChange={e => setEndPage(Number(e.target.value))} disabled={allPages} style={{ width: 50, padding: '4px 6px', border: '1px solid #ddd', borderRadius: 4 }} />
        </label>

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', marginTop: 20 }}>
          <button onClick={onCancel} style={cancelBtnStyle}>Cancel</button>
          <button onClick={() => onExport({
            allPages,
            startPage: startPage - 1,
            endPage: allPages ? pageCount : endPage
          })} style={primaryBtnStyle}>Export</button>
        </div>
      </div>
    </div>
  )
}

const overlayStyle: React.CSSProperties = { position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }
const dialogStyle: React.CSSProperties = { background: '#fff', borderRadius: 12, padding: 24, maxWidth: 350, width: '90%', boxShadow: '0 8px 32px rgba(0,0,0,0.2)' }
const primaryBtnStyle: React.CSSProperties = { padding: '8px 16px', background: '#6366f1', color: '#fff', border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13 }
const cancelBtnStyle: React.CSSProperties = { padding: '8px 16px', background: 'transparent', color: '#666', border: '1px solid #ddd', borderRadius: 6, cursor: 'pointer', fontSize: 13 }
