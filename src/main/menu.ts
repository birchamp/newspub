// src/main/menu.ts
import { Menu, BrowserWindow } from 'electron'

export function buildMenu(): Menu {
  const template: Electron.MenuItemConstructorOptions[] = [
    {
      label: 'File',
      submenu: [
        { label: 'New', accelerator: 'CmdOrCtrl+N', click: (_, win) => win?.webContents.send('menu:new') },
        { label: 'Open...', accelerator: 'CmdOrCtrl+O', click: (_, win) => win?.webContents.send('menu:open') },
        { type: 'separator' },
        { label: 'Save', accelerator: 'CmdOrCtrl+S', click: (_, win) => win?.webContents.send('menu:save') },
        { label: 'Save As...', accelerator: 'CmdOrCtrl+Shift+S', click: (_, win) => win?.webContents.send('menu:save-as') },
        { label: 'Save As Template...', click: (_, win) => win?.webContents.send('menu:save-template') },
        { type: 'separator' },
        { label: 'Export PDF...', accelerator: 'CmdOrCtrl+E', click: (_, win) => win?.webContents.send('menu:export-pdf') },
        { type: 'separator' },
        { role: 'quit' }
      ]
    },
    {
      label: 'Edit',
      submenu: [
        { label: 'Undo', accelerator: 'CmdOrCtrl+Z', click: (_, win) => win?.webContents.send('menu:undo') },
        { label: 'Redo', accelerator: 'CmdOrCtrl+Shift+Z', click: (_, win) => win?.webContents.send('menu:redo') },
        { type: 'separator' },
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        { role: 'delete' },
        { type: 'separator' },
        { role: 'selectAll' }
      ]
    },
    {
      label: 'View',
      submenu: [
        { label: 'Zoom In', accelerator: 'CmdOrCtrl+Plus', click: (_, win) => win?.webContents.send('menu:zoom-in') },
        { label: 'Zoom Out', accelerator: 'CmdOrCtrl+-', click: (_, win) => win?.webContents.send('menu:zoom-out') },
        { label: 'Fit to Window', accelerator: 'CmdOrCtrl+0', click: (_, win) => win?.webContents.send('menu:zoom-fit') },
        { type: 'separator' },
        { role: 'toggleDevTools' }
      ]
    },
    {
      label: 'Insert',
      submenu: [
        { label: 'Text Frame', click: (_, win) => win?.webContents.send('menu:insert-text-frame') },
        { label: 'Image Frame', click: (_, win) => win?.webContents.send('menu:insert-image-frame') },
        { type: 'separator' },
        { label: 'Add Spread', click: (_, win) => win?.webContents.send('menu:add-spread') }
      ]
    },
    {
      label: 'Format',
      submenu: [
        { label: 'Bold', accelerator: 'CmdOrCtrl+B', click: (_, win) => win?.webContents.send('menu:bold') },
        { label: 'Italic', accelerator: 'CmdOrCtrl+I', click: (_, win) => win?.webContents.send('menu:italic') }
      ]
    },
    {
      label: 'Help',
      submenu: [
        { label: 'About NewsPub', click: (_, win) => win?.webContents.send('menu:about') }
      ]
    }
  ]

  // macOS app menu
  if (process.platform === 'darwin') {
    template.unshift({
      label: 'NewsPub',
      submenu: [
        { role: 'about' },
        { type: 'separator' },
        { role: 'hide' },
        { role: 'hideOthers' },
        { role: 'unhide' },
        { type: 'separator' },
        { role: 'quit' }
      ]
    })
  }

  return Menu.buildFromTemplate(template)
}
