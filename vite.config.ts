import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
export default defineConfig({
  plugins: [vue(), tailwindcss()], clearScreen: false,
  server: { host: '127.0.0.1', port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  build: {
    target: 'safari15',
    // Coding mode and terminal mode are pages of their own, so the pet and dashboard never load xterm or the editor.
    rollupOptions: { input: { main: 'index.html', code: 'code.html', terminal: 'terminal.html' } },
  },
})
