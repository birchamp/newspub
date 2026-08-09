// src/renderer/ui/components/PageSidebar.tsx
import { useEditorStore } from '@ui/store/editor-store'

export default function PageSidebar() {
  const { document: doc, currentPageIndex, setCurrentPageIndex } = useEditorStore()

  if (!doc) return <div style={containerStyle}><span style={{ color: '#999' }}>No document</span></div>

  // Group pages into spreads
  const spreads: Array<{ label: string; pageIndices: number[] }> = []

  if (doc.pages.length > 0) {
    spreads.push({ label: 'Page 1', pageIndices: [0] })
  }

  for (let i = 1; i < doc.pages.length; i += 2) {
    if (i + 1 < doc.pages.length) {
      spreads.push({ label: `Pages ${i + 1}–${i + 2}`, pageIndices: [i, i + 1] })
    } else {
      spreads.push({ label: `Page ${i + 1}`, pageIndices: [i] })
    }
  }

  return (
    <div style={containerStyle}>
      <div style={{ fontSize: 11, fontWeight: 600, color: '#666', padding: '8px 0', textTransform: 'uppercase', letterSpacing: 1 }}>
        Pages
      </div>
      {spreads.map((spread, i) => {
        const isActive = spread.pageIndices.includes(currentPageIndex)
        return (
          <div
            key={i}
            onClick={() => setCurrentPageIndex(spread.pageIndices[0])}
            style={{
              padding: '8px 4px',
              marginBottom: 4,
              borderRadius: 4,
              cursor: 'pointer',
              background: isActive ? '#eef2ff' : 'transparent',
              border: isActive ? '1px solid #c7d2fe' : '1px solid transparent',
              fontSize: 11,
              color: '#333'
            }}
          >
            {/* Thumbnail placeholder */}
            <div style={{
              display: 'flex',
              gap: 4,
              justifyContent: 'center',
              marginBottom: 4
            }}>
              {spread.pageIndices.map(pi => (
                <div
                  key={pi}
                  style={{
                    width: spread.pageIndices.length > 1 ? 40 : 50,
                    height: spread.pageIndices.length > 1 ? 52 : 65,
                    background: '#fff',
                    border: '1px solid #ccc',
                    borderRadius: 2
                  }}
                />
              ))}
            </div>
            <div style={{ textAlign: 'center' }}>{spread.label}</div>
          </div>
        )
      })}
    </div>
  )
}

const containerStyle: React.CSSProperties = {
  width: 120,
  borderRight: '1px solid #ddd',
  padding: '8px',
  overflowY: 'auto',
  background: '#f8f8f8',
  flexShrink: 0
}
