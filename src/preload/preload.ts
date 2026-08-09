import { contextBridge, ipcRenderer } from 'electron'

contextBridge.exposeInMainWorld('electronAPI', {
  saveFile: (data: ArrayBuffer, path: string) =>
    ipcRenderer.invoke('file:save', data, path),
  openFile: (path: string) =>
    ipcRenderer.invoke('file:open', path),
  showSaveDialog: (options: object) =>
    ipcRenderer.invoke('dialog:save', options),
  showOpenDialog: (options: object) =>
    ipcRenderer.invoke('dialog:open', options),
  exportPDF: (data: ArrayBuffer, path: string) =>
    ipcRenderer.invoke('export:pdf', data, path),
  getSystemFontsDir: () =>
    ipcRenderer.invoke('system:fontsDir'),
  onMenuEvent: (channel: string, callback: (...args: any[]) => void) => {
    ipcRenderer.on(channel, (_event, ...args) => callback(...args))
  }
})
