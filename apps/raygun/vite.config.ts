import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import type { Plugin } from 'vite';
import { defineConfig } from 'vitest/config';

// Set by `tauri android dev` so the emulator can reach the dev server over the LAN.
const host = process.env.TAURI_DEV_HOST;

/**
 * Puts Raygun's mark (public/icon.svg) into the start screen in index.html as markup, so it shows
 * with the page's first paint instead of after a request for the picture.
 */
const splashMark = (): Plugin => ({
  name: 'raygun-splash-mark',
  transformIndexHtml: (html) => {
    const mark = readFileSync(fileURLToPath(new URL('./public/icon.svg', import.meta.url)), 'utf8')
      .replace(/<\?xml[^>]*\?>\s*/, '')
      .replace('<svg ', '<svg aria-hidden="true" focusable="false" ');
    return html.replace('<!-- raygun:splash-mark -->', mark);
  },
});

export default defineConfig({
  plugins: [react(), tailwindcss(), splashMark()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  // The saved query cache belongs to one build (M46.47).
  define: { __RAYGUN_BUILD__: JSON.stringify(String(Date.now())) },
  build: {
    target: 'es2022',
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
  },
});
