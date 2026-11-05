import { commands, events } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';

import { usePlayerUiStore } from '../player/playerUiStore';

/** `/downloads?tab=music` (or `/settings/ai#transcription`) as a path, its search and its hash. */
export const splitRoute = (
  route: string,
): { to: string; search: Record<string, string>; hash?: string } => {
  const [address = '', hash] = route.split('#', 2);
  const [to, query = ''] = address.split('?', 2);
  return {
    to: to || '/',
    search: Object.fromEntries(new URLSearchParams(query)),
    ...(hash ? { hash } : {}),
  };
};

/**
 * Opens the screen a tapped notification names: the full player for the playback notification,
 * Downloads (or where a download is listed) for download notifications. A tap that started Raygun
 * waits in Rust until this asks.
 */
export const useNotificationRoutes = () => {
  const navigate = useNavigate();
  useEffect(() => {
    const open = (route: string) => {
      if (route === 'player') {
        usePlayerUiStore.getState().openSheet();
        return;
      }
      const { to, search, hash } = splitRoute(route);
      void navigate({ to, search, hash });
    };
    let alive = true;
    void commands.notificationRouteTake().then((route) => {
      if (alive && route) open(route);
    });
    const unlisten = events.openRouteEvent.listen(({ payload }) => {
      // Taken, so a restart doesn't open it again.
      void commands.notificationRouteTake();
      open(payload);
    });
    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
    };
  }, [navigate]);
};
