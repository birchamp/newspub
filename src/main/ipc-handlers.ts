// src/main/ipc-handlers.ts
import { ipcMain, dialog } from 'electron'
import { readFile, writeFile, rename, unlink } from 'fs/promises'
import { join, homedir } from 'path'
import { existsSync } from 'fs'

export function registerIpcHandlers(): void {
  ipcMain.handle('file:save', async (_event, data: ArrayBuffer, path: string) => {
    // Atomic write: write to .tmp, then rename
    const tmpPath = path + '.tmp'
    await writeFile(tmpPath, Buffer.from(data))
    await rename(tmpPath, path)
    return { success: true }
  })

  ipcMain.handle('file:open', async (_event, path: string) => {
    const buffer = await readFile(path)
    return buffer.buffer
  })

  ipcMain.handle('dialog:save', async (_event, options: object) => {
    const result = await dialog.showSaveDialog({
      filters: [{ name: 'NewsPub Files', extensions: ['newspub'] }],
      ...options
    })
    return result
  })

  ipcMain.handle('dialog:open', async (_event, options: object) => {
    const result = await dialog.showOpenDialog({
      filters: [{ name: 'NewsPub Files', extensions: ['newspub'] }],
      properties: ['openFile'],
      ...options
    })
    return result
  })

  ipcMain.handle('export:pdf', async (_event, data: ArrayBuffer, path: string) => {
    await writeFile(path, Buffer.from(data))
    return { success: true }
  })

  ipcMain.handle('system:fontsDir', async () => {
    const platform = process.platform
    if (platform === 'darwin') {
      return [
        join(homedir(), 'Library/Fonts'),
        '/System/Library/Fonts',
        '/Library/Fonts'
      ]
    } else if (platform === 'win32') {
      return ['C:\\Windows\\Fonts']
    } else {
      return [
        join(homedir(), '.fonts'),
        '/usr/share/fonts',
        '/usr/local/share/fonts'
      ]
    }
  })
}
