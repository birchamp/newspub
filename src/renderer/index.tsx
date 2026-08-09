import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { useEditorStore } from '@ui/store/editor-store'

// Expose the store for integration tests and debugging
;(window as any).__newspubStore = useEditorStore

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
