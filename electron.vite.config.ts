import { defineConfig, externalizeDepsPlugin } from 'electron-vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  main: {
    plugins: [externalizeDepsPlugin()]
  },
  preload: {
    plugins: [externalizeDepsPlugin()]
  },
  renderer: {
    plugins: [react()],
    resolve: {
      alias: {
        '@model': resolve('src/renderer/model'),
        '@engine': resolve('src/renderer/engine'),
        '@canvas': resolve('src/renderer/canvas'),
        '@input': resolve('src/renderer/input'),
        '@history': resolve('src/renderer/history'),
        '@file': resolve('src/renderer/file'),
        '@template': resolve('src/renderer/template'),
        '@export': resolve('src/renderer/export'),
        '@ui': resolve('src/renderer/ui')
      }
    }
  }
})
