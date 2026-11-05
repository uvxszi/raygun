import { commands, events } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';

import { useCollectionSheets } from '../collections/collectionSheets';
import { useRevealStore } from '../music/reveal';
import { usePlayerUiStore } from '../player/playerUiStore';

/**
 * How long a screen a notification opened may take to load the row it names: a discography
 * comes from the music sources, some slow to answer.
 */
const NOTICE_REVEAL_WAIT_MS = 30_000;

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

/** One screen a notification opens, and the row to show there (`music/reveal.ts`), if any. */
export type RouteStep = {
  to: string;
  search: Record<string, string>;
  hash?: string;
  reveal: string | null;
};

/**
 * A notification's route as the screens it opens: routes on separate lines open one over another
 * (a new album over its artist's page, so Back leads there), and a route's `reveal` names the row
 * to scroll to and mark on its page (a new single in the artist's discography).
 */
export const routeSteps = (route: string): RouteStep[] =>
  route
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const { to, search, hash } = splitRoute(line);
      const { reveal = null, ...rest } = search;
      return { to, search: rest, ...(hash ? { hash } : {}), reveal: reveal || null };
    });

/**
 * Opens the screen Rust names: the full player for the playback notification (and a song another
 * app opened with Raygun), Downloads (or where a download is listed) for download notifications,
 * a followed artist's new release, Search for words or a link shared to Raygun, and Import songs
 * for a shared Spotify link (`import?link=`). A route from before the UI started waits in Rust
 * until this asks.
 */
export const useNotificationRoutes = () => {
  const navigate = useNavigate();
  useEffect(() => {
    const open = async (route: string) => {
      if (route === 'player') {
        usePlayerUiStore.getState().openSheet();
        return;
      }
      for (const { to, search, hash, reveal } of routeSteps(route)) {
        if (to === 'import') {
          useCollectionSheets.getState().openImportLink(search.link ?? '');
          return;
        }
        if (reveal) {
          useRevealStore
            .getState()
            .request({ keys: [reveal], to, search, waitMs: NOTICE_REVEAL_WAIT_MS });
        }
        await navigate({ to, search, hash });
      }
    };
    let alive = true;
    void commands.notificationRouteTake().then((route) => {
      if (alive && route) void open(route);
    });
    const unlisten = events.openRouteEvent.listen(({ payload }) => {
      // Taken, so a restart doesn't open it again.
      void commands.notificationRouteTake();
      void open(payload);
    });
    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
    };
  }, [navigate]);
};
