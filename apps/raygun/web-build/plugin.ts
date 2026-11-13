// The Vite plugin behind Raygun on the web (vite.pages.config.ts): the page starts from the web
// entry (src/web/entry.tsx) instead of Tauri's, gets the commands' empty answers from the
// bindings, and the build adds what an installable web app needs (a manifest, icons, a service
// worker) and the YouTube embed host page the player uses.
import { readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import type { Plugin, ResolvedConfig } from 'vite';

import { commandAnswers } from './commandAnswers';

const ANSWERS = 'virtual:raygun-web-answers';
const RESOLVED_ANSWERS = `\0${ANSWERS}`;

const fromHere = (path: string) => fileURLToPath(new URL(path, import.meta.url));

const BINDINGS = fromHere('../../../packages/bindings/src/index.ts');
const EMBED = fromHere('../../../crates/raygun-net/src/embed/youtube.html');
const ICONS: Array<[string, string, string]> = [
  ['icons/icon-256.png', '../src-tauri/icons/128x128@2x.png', '256x256'],
  ['icons/icon-512.png', '../src-tauri/icons/icon.png', '512x512'],
];
/** The dark theme's background (index.html's start screen, styles.css `--color-background`). */
const BACKGROUND = '#0a0d16';

const manifest = (base: string) =>
  JSON.stringify(
    {
      name: 'Raygun',
      short_name: 'Raygun',
      description: 'Music, podcasts, books, comics and video in one app.',
      start_url: base,
      scope: base,
      id: base,
      display: 'standalone',
      background_color: BACKGROUND,
      theme_color: BACKGROUND,
      icons: [
        { src: `${base}icon.svg`, sizes: 'any', type: 'image/svg+xml' },
        ...ICONS.map(([name, , sizes]) => ({
          src: `${base}${name}`,
          sizes,
          type: 'image/png',
          purpose: 'any',
        })),
      ],
    },
    null,
    2,
  );

/** Every file under `dir`, as paths relative to it with forward slashes. */
const filesIn = (dir: string): string[] =>
  readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? filesIn(path).map((inner) => `${name}/${inner}`) : [name];
  });

/**
 * The service worker: the app's files are kept when it installs, so it opens without a network;
 * the page itself is asked for first (a new version shows on the next load), the hashed files
 * come from the cache.
 */
const serviceWorker = (
  files: string[],
  version: string,
) => `// Raygun on the web: keeps the app's files for offline starts (docs/WEB.md).
const CACHE = 'raygun-web-${version}';
const FILES = ${JSON.stringify(files)};

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.addAll(FILES.map((file) => new URL(file, self.registration.scope).href)))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== 'GET' || url.origin !== self.location.origin) return;
  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request).catch(() =>
        caches.match(new URL('index.html', self.registration.scope).href).then(
          (page) => page || Response.error(),
        ),
      ),
    );
    return;
  }
  event.respondWith(caches.match(request).then((kept) => kept || fetch(request)));
});
`;

export const webBuild = (): Plugin => {
  let config: ResolvedConfig | undefined;
  return {
    name: 'raygun-web',
    configResolved: (resolved) => {
      config = resolved;
    },
    resolveId: (id) => (id === ANSWERS ? RESOLVED_ANSWERS : null),
    load: (id) =>
      id === RESOLVED_ANSWERS
        ? `export default ${JSON.stringify(commandAnswers(BINDINGS))};`
        : null,
    transformIndexHtml: {
      order: 'pre',
      handler: (html) => {
        const base = config?.base ?? '/';
        return html
          .replace('/src/main.tsx', '/src/web/entry.tsx')
          .replace(
            '<title>Raygun</title>',
            [
              '<title>Raygun</title>',
              `<link rel="manifest" href="${base}manifest.webmanifest" />`,
              `<meta name="theme-color" content="${BACKGROUND}" />`,
              `<link rel="apple-touch-icon" href="${base}${ICONS[0]?.[0] ?? ''}" />`,
              '<meta name="description" content="Music, podcasts, books, comics and video in one app." />',
            ].join('\n    '),
          );
      },
    },
    configureServer: (server) => {
      // The dev server serves the manifest and the embed page as the build does.
      server.middlewares.use((request, response, next) => {
        const base = config?.base ?? '/';
        const path = (request.url ?? '').split('?')[0] ?? '';
        if (path === `${base}manifest.webmanifest`) {
          response.setHeader('Content-Type', 'application/manifest+json');
          response.end(manifest(base));
          return;
        }
        if (path === `${base}embed/youtube.html`) {
          response.setHeader('Content-Type', 'text/html; charset=utf-8');
          response.end(readFileSync(EMBED));
          return;
        }
        next();
      });
    },
    generateBundle() {
      const base = config?.base ?? '/';
      this.emitFile({ type: 'asset', fileName: 'manifest.webmanifest', source: manifest(base) });
      this.emitFile({ type: 'asset', fileName: 'embed/youtube.html', source: readFileSync(EMBED) });
      for (const [name, source] of ICONS) {
        this.emitFile({ type: 'asset', fileName: name, source: readFileSync(fromHere(source)) });
      }
    },
    writeBundle: () => {
      if (!config) return;
      const outDir = config.build.outDir;
      const files = filesIn(outDir).filter(
        (file) => file !== 'sw.js' && file !== '.nojekyll' && !file.endsWith('.map'),
      );
      const version = String(Date.now());
      writeFileSync(join(outDir, 'sw.js'), serviceWorker(files, version));
      // GitHub Pages runs Jekyll unless told not to, which hides files starting with "_".
      writeFileSync(join(outDir, '.nojekyll'), '');
    },
  };
};
