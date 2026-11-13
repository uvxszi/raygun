// Raygun on the web (docs/WEB.md): the same app built for a browser tab, served from GitHub Pages
// under /raygun/ by default (RAYGUN_WEB_BASE to change it). `pnpm build:pages` builds it into
// dist-web; the Tauri builds keep using vite.config.ts.
import { readFileSync } from 'node:fs';

import { defineConfig, mergeConfig } from 'vite';

import app from './vite.config';
import { webBuild } from './web-build/plugin';

const version = (
  JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as {
    version: string;
  }
).version;

export default mergeConfig(
  app,
  defineConfig({
    base: process.env.RAYGUN_WEB_BASE ?? '/raygun/',
    plugins: [webBuild()],
    server: { port: 1430, strictPort: false },
    preview: { port: 1431 },
    define: { __RAYGUN_VERSION__: JSON.stringify(version) },
    build: { outDir: 'dist-web', emptyOutDir: true, sourcemap: true },
  }),
);
