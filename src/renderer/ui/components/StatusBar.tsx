// src/renderer/ui/components/StatusBar.tsx
import { useEditorStore } from '@ui/store/editor-store'

export default function StatusBar() {
  const { document: doc, zoom, currentPageIndex } = useEditorStore()

  const pageCount = doc?.pages.length ?? 0

  return (
    <div style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '2px 12px',
      borderTop: '1px solid #ddd',
      background: '#fafafa',
      fontSize: 11,
      color: '#666',
      height: 24
    }}>
      <span>
        {pageCount > 0
          ? `Page ${currentPageIndex + 1} of ${pageCount}`
          : 'No document'
        }
      </span>
      <span>{Math.round(zoom * 100)}%</span>
    </div>
  )
}
