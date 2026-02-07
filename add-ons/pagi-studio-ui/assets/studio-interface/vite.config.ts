import path from 'path';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig(() => {
    return {
      server: {
        port: 3001,
        host: '127.0.0.1',
        // UI-only on 3001. /api is proxied ONLY to the Rust Gateway (8001). No proxy to 3001.
        proxy: {
          '/api': {
            target: 'http://127.0.0.1:8001',
            changeOrigin: true,
          },
        },
      },
      plugins: [react(), tailwindcss()],
      resolve: {
        alias: {
          '@': path.resolve(__dirname, '.'),
        }
      }
    };
});
