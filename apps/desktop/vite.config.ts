import react from '@vitejs/plugin-react';
import process from 'node:process';
import { defineConfig } from 'vite';

const host = process.env['TAURI_DEV_HOST'];

// Options tailored for `tauri dev` / `tauri build`: https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [react()],
  // Keep Rust errors visible.
  clearScreen: false,
  server: {
    // Tauri expects a fixed port.
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
});
