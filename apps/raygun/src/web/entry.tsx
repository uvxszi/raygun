// Raygun on the web starts here (vite.pages.config.ts puts this in index.html instead of
// main.tsx): the web core opens the browser's storage, stands in for Tauri's IPC, and then the
// same app as on a phone or computer starts.
import answers from 'virtual:raygun-web-answers';

import { markWebBuild } from '../lib/webBuild';
import { createWebCore } from './core';
import { indexedDbBackend, memoryBackend } from './core/store';
import type { Backend } from './core/store';
import { installWebIpc } from './ipc';

const openStorage = async (): Promise<Backend> => {
  try {
    return await indexedDbBackend();
  } catch (error) {
    // A private window that refuses storage still runs; nothing is kept.
    console.warn('[raygun web] browser storage refused; nothing will be kept', error);
    return memoryBackend();
  }
};

const registerServiceWorker = () => {
  if (import.meta.env.DEV || !('serviceWorker' in navigator)) return;
  void navigator.serviceWorker
    .register(`${import.meta.env.BASE_URL}sw.js`, { scope: import.meta.env.BASE_URL })
    .catch((error: unknown) => console.warn('[raygun web] no offline start', error));
};

const start = async () => {
  markWebBuild();
  // Asks the browser not to clear Raygun's data when space runs low.
  void navigator.storage?.persist?.().catch(() => false);
  const core = await createWebCore({
    backend: await openStorage(),
    answers,
    version: __RAYGUN_VERSION__,
    youTubeEmbed: new URL(
      `${import.meta.env.BASE_URL}embed/youtube.html?audio=default&codec=auto`,
      location.href,
    ).href,
    debug: import.meta.env.DEV,
  });
  installWebIpc(core.invoke);
  core.start();
  registerServiceWorker();
  await import('../main');
};

void start();
