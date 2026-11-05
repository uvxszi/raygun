import { commands, events } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';

import { useCollectionSheets } from '../collections/collectionSheets';
import { usePlayerUiStore } from '../player/playerUiStore';

/** `/downloads?tab=music` as a path and its search. */
export const splitRoute = (route: string): { to: string; search: Record<string, string> } => {
  const [to, query = ''] = route.split('?', 2);
  return { to: to || '/', search: Object.fromEntries(new URLSearchParams(query)) };
};

/**
 * Opens the screen Rust names: the full player for the playback notification (and a song another
 * app opened with Raygun), Downloads (or where a download is listed) for download notifications,
 * Search for words or a link shared to Raygun, and Import songs for a shared Spotify link
 * (`import?link=`). A route from before the UI started waits in Rust until this asks.
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
      if (to === 'import') {
        useCollectionSheets.getState().openImportLink(search.link ?? '');
        return;
      }
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
