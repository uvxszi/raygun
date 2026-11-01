import type { MangaLibraryEntry } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Heart, Library, Search } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { SavedTrackRow } from '../collections/SavedTrackRow';
import {
  HOME_HISTORY_LIMIT,
  playCollection,
  useClearHistory,
  useHistory,
  usePlaylists,
} from '../collections/useCollections';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import { useMangaLibrary } from '../manga/useManga';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { ContinueWatchingRow } from '../watch/ContinueWatchingRow';
import { useContinueWatching } from '../watch/useWatch';

/** Playlists shown as shortcuts next to Favorites. */
const HOME_PLAYLISTS = 5;

const TILE_CLASS = 'bg-surface flex w-32 shrink-0 flex-col gap-2 rounded-xl p-2 text-sm';

const YourMusic: FC = () => {
  const { t } = useTranslation();
  const playlists = usePlaylists();
  return (
    <section className="flex flex-col gap-2">
      <h2 className="text-lg font-semibold">{t('home.yourMusic')}</h2>
      <ul className="-mx-4 flex gap-3 overflow-x-auto px-4 lg:mx-0 lg:px-0">
        <li>
          <Link to="/library" search={{ view: 'favorites' }} className={TILE_CLASS}>
            <span className="bg-primary/15 text-primary grid aspect-square place-items-center rounded-lg">
              <Heart aria-hidden size={32} fill="currentColor" />
            </span>
            <span className="truncate font-medium">{t('collections.favorites')}</span>
          </Link>
        </li>
        {playlists.data?.slice(0, HOME_PLAYLISTS).map((playlist) => (
          <li key={playlist.id}>
            <Link to="/library/playlist" search={{ id: playlist.id }} className={TILE_CLASS}>
              <PlaylistCover
                artworkUrls={playlist.artworkUrls}
                className="aspect-square w-full rounded-lg"
              />
              <span className="truncate font-medium">{playlist.name}</span>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
};

const RecentlyPlayed: FC = () => {
  const { t } = useTranslation();
  const history = useHistory(HOME_HISTORY_LIMIT);
  const clear = useClearHistory();
  if (!history.data?.length) {
    return null;
  }
  return (
    <section data-testid="recently-played" className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">{t('home.recentlyPlayed')}</h2>
        {history.data.length > 0 && (
          <button
            type="button"
            disabled={clear.isPending}
            onClick={() => clear.mutate()}
            className="text-muted text-sm"
          >
            {t('home.clearHistory')}
          </button>
        )}
      </div>
      {history.data.length === 0 ? (
        <p className="text-muted text-sm">{t('home.noHistory')}</p>
      ) : (
        <ul className="flex flex-col">
          {history.data.map((entry, index) => (
            <li key={entry.track.trackKey}>
              <SavedTrackRow
                track={entry.track}
                onPlay={() =>
                  void playCollection({
                    collection: { kind: 'history' },
                    startIndex: index,
                    shuffle: false,
                  })
                }
              />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};

/** Most titles in Continue reading. */
const CONTINUE_READING = 12;

/** Manga and novels read lately, newest first (Mihon's History, as a row). */
const ContinueReading: FC = () => {
  const { t } = useTranslation();
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  const entries: (MangaLibraryEntry & { medium: 'manga' | 'novel' })[] = [
    ...(manga.data ?? []).map((entry) => ({ ...entry, medium: 'manga' as const })),
    ...(novels.data ?? []).map((entry) => ({ ...entry, medium: 'novel' as const })),
  ]
    .filter((entry) => entry.lastReadAt !== null)
    .sort((a, b) => (b.lastReadAt ?? 0) - (a.lastReadAt ?? 0))
    .slice(0, CONTINUE_READING);
  if (entries.length === 0) return null;
  return (
    <section aria-labelledby="continue-reading" className="flex flex-col gap-2">
      <h2 id="continue-reading" className="text-lg font-semibold">
        {t('home.continueReading')}
      </h2>
      <ul className="-mx-4 flex gap-3 overflow-x-auto px-4 lg:mx-0 lg:px-0">
        {entries.map((entry) => (
          <li key={entry.titleId} className="w-28 shrink-0">
            <Link
              to={ROUTES[entry.medium].title}
              search={{ source: entry.sourceId, id: entry.remoteId }}
              aria-label={entry.title}
              className="flex flex-col gap-1.5"
            >
              <span className="relative">
                <MangaCover url={entry.coverUrl} className="w-full rounded-xl" />
                {entry.unread > 0 && (
                  <span className="bg-primary text-background absolute top-1.5 left-1.5 rounded-md px-1.5 text-xs font-bold">
                    {entry.unread}
                  </span>
                )}
              </span>
              <span className="line-clamp-2 text-sm font-medium">{entry.title}</span>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
};

/** Shown while there's nothing to continue: where to start. */
const GetStarted: FC = () => {
  const { t } = useTranslation();
  return (
    <section className="bg-surface flex flex-col gap-3 rounded-3xl px-5 py-6">
      <h2 className="text-lg font-semibold">{t('home.startTitle')}</h2>
      <p className="text-muted text-sm">{t('home.startBody')}</p>
      <div className="flex flex-wrap gap-2">
        <Link
          to="/search"
          className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2.5 text-sm font-semibold"
        >
          <Search aria-hidden size={16} />
          {t('search.title')}
        </Link>
        <Link
          to="/library"
          className="bg-background flex items-center gap-2 rounded-full px-5 py-2.5 text-sm font-medium"
        >
          <Library aria-hidden size={16} />
          {t('library.title')}
        </Link>
      </div>
    </section>
  );
};

/** Home: picking up where you left off, in everything. */
export const HomePage: FC = () => {
  const { t } = useTranslation();
  const watching = useContinueWatching();
  const history = useHistory(HOME_HISTORY_LIMIT);
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  const nothingYet =
    watching.data?.length === 0 &&
    history.data?.length === 0 &&
    !manga.data?.some((entry) => entry.lastReadAt !== null) &&
    !novels.data?.some((entry) => entry.lastReadAt !== null);
  return (
    <section data-testid="home-page" className="flex flex-col gap-7">
      <TopBar
        title={t('home.heading')}
        actions={<TopBarAction label={t('search.title')} icon={Search} to="/search" />}
      />
      {nothingYet && <GetStarted />}
      <ContinueWatchingRow />
      <ContinueReading />
      <YourMusic />
      <RecentlyPlayed />
    </section>
  );
};
