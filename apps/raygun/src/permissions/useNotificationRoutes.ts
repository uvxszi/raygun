import { commands, events } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';

import { usePlayerUiStore } from '../player/playerUiStore';

/** `/downloads?tab=music` as a path and its search. */
export const splitRoute = (route: string): { to: string; search: Record<string, string> } => {
  const [to, query = ''] = route.split('?', 2);
  return { to: to || '/', search: Object.fromEntries(new URLSearchParams(query)) };
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
      const { to, search } = splitRoute(route);
      void navigate({ to, search });
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
