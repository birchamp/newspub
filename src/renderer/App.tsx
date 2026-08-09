import { useEffect, useState, useCallback, useRef } from 'react'
import Toolbar from '@ui/components/Toolbar'
import PageSidebar from '@ui/components/PageSidebar'
import DocumentCanvas from '@ui/components/DocumentCanvas'
import PropertiesPanel from '@ui/components/PropertiesPanel'
import StatusBar from '@ui/components/StatusBar'
import TemplatePickerDialog from '@ui/components/TemplatePickerDialog'
import SaveAsTemplateDialog from '@ui/components/SaveAsTemplateDialog'
import ExportDialog from '@ui/components/ExportDialog'
import DocumentSetupDialog from '@ui/components/DocumentSetupDialog'
import { useEditorStore } from '@ui/store/editor-store'
import { presetTemplates } from '@template/preset-templates'
import { appHistory } from '@history/app-history'
import { undo, redo, toggleStyleFlag, zoomStep } from '@ui/actions'
import { addSpread } from '@model/page'
import { packNewspub, unpackNewspub } from '@file/file-manager'
import { layoutDocument } from '@engine/layout-engine'
import { exportToPdf } from '@export/pdf-exporter'
import { stripToTemplate } from '@template/template-manager'
import type { Document } from '@model/types'

type DialogState =
  | { type: 'none' }
  | { type: 'template-picker' }
  | { type: 'save-as-template' }
  | { type: 'export' }
  | { type: 'document-setup' }

export default function App() {
  const { document: doc, setDocument, filePath, setFilePath, markClean } = useEditorStore()
  const [dialog, setDialog] = useState<DialogState>({ type: 'template-picker' })

  // Latest-callback ref so the one-time menu registration below never
  // captures a stale closure (e.g. doc === null on first render).
  const menuRef = useRef({ handleSave: () => {}, handleSaveAs: () => {}, handleOpen: () => {} })

  // Listen for menu events from main process
  useEffect(() => {
    const api = (window as any).electronAPI
    if (!api?.onMenuEvent) return

    const store = useEditorStore.getState
    const handlers: Record<string, () => void> = {
      'menu:new': () => setDialog({ type: 'template-picker' }),
      'menu:open': () => menuRef.current.handleOpen(),
      'menu:save': () => menuRef.current.handleSave(),
      'menu:save-as': () => menuRef.current.handleSaveAs(),
      'menu:save-template': () => setDialog({ type: 'save-as-template' }),
      'menu:export-pdf': () => setDialog({ type: 'export' }),
      'menu:undo': undo,
      'menu:redo': redo,
      'menu:zoom-in': () => zoomStep(1),
      'menu:zoom-out': () => zoomStep(-1),
      'menu:zoom-fit': () => store().requestFit(),
      'menu:insert-text-frame': () => store().setActiveTool('draw-text-frame'),
      'menu:insert-image-frame': () => store().setActiveTool('draw-image-frame'),
      'menu:add-spread': () => {
        const s = store()
        if (s.document) s.updateDocument(dd => addSpread(dd, dd.pages.length))
      },
      'menu:bold': () => toggleStyleFlag('bold'),
      'menu:italic': () => toggleStyleFlag('italic'),
    }

    for (const [channel, handler] of Object.entries(handlers)) {
      api.onMenuEvent(channel, handler)
    }
  }, [])

  const handleSave = useCallback(async () => {
    if (!doc) return
    const api = (window as any).electronAPI
    let path = filePath
    if (!path) {
      const result = await api.showSaveDialog({})
      if (result.canceled || !result.filePath) return
      path = result.filePath
      setFilePath(path)
    }
    const buffer = await packNewspub(doc)
    await api.saveFile(buffer, path)
    markClean()
  }, [doc, filePath, setFilePath, markClean])

  const handleSaveAs = useCallback(async () => {
    if (!doc) return
    const api = (window as any).electronAPI
    const result = await api.showSaveDialog({})
    if (result.canceled || !result.filePath) return
    setFilePath(result.filePath)
    const buffer = await packNewspub(doc)
    await api.saveFile(buffer, result.filePath)
    markClean()
  }, [doc, setFilePath, markClean])

  const handleOpen = useCallback(async () => {
    const api = (window as any).electronAPI
    const result = await api.showOpenDialog({})
    if (result.canceled || !result.filePaths?.length) return
    const path = result.filePaths[0]
    try {
      const buffer = await api.openFile(path)
      const opened = await unpackNewspub(buffer)
      setDocument(opened)
      setFilePath(path)
      appHistory.clear()
      setDialog({ type: 'none' })
    } catch (err) {
      console.error('Failed to open document:', err)
    }
  }, [setDocument, setFilePath])

  // Keep menu handlers pointing at the latest callbacks
  useEffect(() => {
    menuRef.current = { handleSave, handleSaveAs, handleOpen }
  }, [handleSave, handleSaveAs, handleOpen])

  const handleExport = useCallback(async (options: { allPages: boolean; startPage: number; endPage: number }) => {
    if (!doc) return
    const api = (window as any).electronAPI
    const result = await api.showSaveDialog({ filters: [{ name: 'PDF', extensions: ['pdf'] }] })
    if (result.canceled || !result.filePath) return

    const layout = layoutDocument(doc)
    const pageRange = options.allPages ? undefined : { start: options.startPage, end: options.endPage }
    const { pdfBytes, warnings } = await exportToPdf(doc, layout, { pageRange })

    if (warnings.length > 0) {
      console.warn('PDF export warnings:', warnings)
    }

    await api.exportPDF(pdfBytes, result.filePath)
    setDialog({ type: 'none' })
  }, [doc])

  return (
    <div style={{ width: '100vw', height: '100vh', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      <Toolbar />
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <PageSidebar />
        <div style={{ flex: 1, overflow: 'hidden', position: 'relative' }}>
          <DocumentCanvas />
        </div>
        <PropertiesPanel />
      </div>
      <StatusBar />

      {dialog.type === 'template-picker' && (
        <TemplatePickerDialog
          onSelect={(newDoc) => { setDocument(newDoc); setDialog({ type: 'none' }); appHistory.clear(); }}
          onCancel={() => { if (doc) setDialog({ type: 'none' }) }}
        />
      )}
      {dialog.type === 'save-as-template' && doc && (
        <SaveAsTemplateDialog
          defaultTitle={doc.metadata.title}
          onSave={async (name, desc, options) => {
            const template = stripToTemplate(doc, options)
            template.metadata.title = name
            const buffer = await packNewspub(template)
            const api = (window as any).electronAPI
            const result = await api.showSaveDialog({ filters: [{ name: 'NewsPub Template', extensions: ['newspub'] }] })
            if (!result.canceled && result.filePath) {
              await api.saveFile(buffer, result.filePath)
            }
            setDialog({ type: 'none' })
          }}
          onCancel={() => setDialog({ type: 'none' })}
        />
      )}
      {dialog.type === 'export' && doc && (
        <ExportDialog
          pageCount={doc.pages.length}
          onExport={handleExport}
          onCancel={() => setDialog({ type: 'none' })}
        />
      )}
      {dialog.type === 'document-setup' && doc && (
        <DocumentSetupDialog
          currentSize={doc.metadata.pageSize}
          onApply={(size, scale) => {
            // TODO: scale frames proportionally if scale=true
            setDocument({ ...doc, metadata: { ...doc.metadata, pageSize: size } })
            setDialog({ type: 'none' })
          }}
          onCancel={() => setDialog({ type: 'none' })}
        />
      )}
    </div>
  )
}
