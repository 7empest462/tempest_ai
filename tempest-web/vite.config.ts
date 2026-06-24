import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'path';
import fs, { cpSync } from 'fs';
import type { Plugin } from 'vite';

const MONACO_PREFIX = '/monaco-editor/min/vs';

// Custom plugin that:
//  - build: copies node_modules/monaco-editor/min/vs -> dist/monaco-editor/min/vs
//  - dev:   middleware rewrites /monaco-editor/min/vs/* to node_modules path
function monacoEditorPlugin(): Plugin {
  const monacoRoot = path.resolve(__dirname, 'node_modules/monaco-editor/min/vs');

  return {
    name: 'monaco-editor-local',

    // --- Production: copy assets after Rollup finishes writing dist/ ---
    closeBundle() {
      const dest = path.resolve(__dirname, 'dist/monaco-editor/min/vs');
      if (fs.existsSync(monacoRoot)) {
        cpSync(monacoRoot, dest, { recursive: true });
        console.log('[monaco-editor-local] Copied Monaco vs/ assets to dist/');
      }
    },

    // --- Dev: serve Monaco files from node_modules via middleware ---
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (req.url && req.url.startsWith(MONACO_PREFIX)) {
          const filePath = path.join(
            monacoRoot,
            req.url.slice(MONACO_PREFIX.length) || ''
          );

          if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
            const ext = path.extname(filePath).toLowerCase();
            const mimeTypes: Record<string, string> = {
              '.js': 'application/javascript',
              '.css': 'text/css',
              '.svg': 'image/svg+xml',
              '.ttf': 'font/ttf',
              '.woff': 'font/woff',
              '.woff2': 'font/woff2',
              '.json': 'application/json',
            };
            res.setHeader('Content-Type', mimeTypes[ext] || 'application/octet-stream');
            res.setHeader('Cache-Control', 'public, max-age=31536000, immutable');
            fs.createReadStream(filePath).pipe(res);
            return;
          }
        }
        next();
      });
    },
  };
}

export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  plugins: [
    tailwindcss(),
    react(),
    wasm(),
    topLevelAwait(),
    monacoEditorPlugin(),
  ],
  server: {
    port: 3000,
    strictPort: true,
    fs: {
      // Explicitly setting allow overrides Vite defaults, so include the
      // project root alongside the Monaco node_modules path.
      allow: [
        path.resolve(__dirname),
        path.resolve(__dirname, 'node_modules/monaco-editor'),
      ],
    },
  },
  build: {
    target: 'esnext',
    chunkSizeWarningLimit: 1000,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules')) {
            if (id.includes('@monaco-editor') || id.includes('monaco-editor')) {
              return 'vendor-monaco';
            }
            if (id.includes('xterm')) {
              return 'vendor-xterm';
            }
            if (id.includes('framer-motion')) {
              return 'vendor-framer-motion';
            }
            if (id.includes('lucide-react')) {
              return 'vendor-lucide';
            }
            return 'vendor';
          }
        },
      },
    },
  },
});
