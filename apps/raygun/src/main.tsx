import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { RouterProvider, createHashHistory } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import './i18n';
import { MiniplayerApp } from './miniplayer/MiniplayerApp';
import { createAppRouter } from './router';
import './styles.css';

// Queries are IPC calls into the local core, not network requests: they must run offline too
// (the default 'online' mode pauses them while the device has no connection).
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { networkMode: 'always' },
    mutations: { networkMode: 'always' },
  },
});
// Hash history: the app is served from a custom protocol where no server resolves deep paths,
// so routes live in the URL fragment.
// The miniplayer window (M46.13) loads the same page and draws only the miniplayer: no router, no
// shell, and no playback engines of its own. Tauri names each window.
const isMiniplayerWindow = () => {
  try {
    return getCurrentWindow().label === 'miniplayer';
  } catch {
    // Not in Tauri (a plain browser).
    return false;
  }
};
const miniplayer = isMiniplayerWindow();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      {miniplayer ? (
        <MiniplayerApp />
      ) : (
        <RouterProvider router={createAppRouter(createHashHistory())} />
      )}
    </QueryClientProvider>
  </StrictMode>,
);
