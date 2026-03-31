import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@model': resolve(__dirname, './src/renderer/model'),
      '@engine': resolve(__dirname, './src/renderer/engine'),
      '@canvas': resolve(__dirname, './src/renderer/canvas'),
      '@input': resolve(__dirname, './src/renderer/input'),
      '@history': resolve(__dirname, './src/renderer/history'),
      '@file': resolve(__dirname, './src/renderer/file'),
      '@template': resolve(__dirname, './src/renderer/template'),
      '@export': resolve(__dirname, './src/renderer/export'),
      '@ui': resolve(__dirname, './src/renderer/ui')
    }
  },
  test: {
    globals: true,
    environment: 'node'
  }
})
