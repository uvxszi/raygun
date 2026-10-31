import { Link } from '@tanstack/react-router';
import { Heart } from 'lucide-react';
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
  if (!history.data) {
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

export const HomePage: FC = () => {
  const { t } = useTranslation();
  return (
    <section data-testid="home-page" className="flex flex-col gap-6">
      <div className="flex flex-col gap-2">
        <h1 className="text-primary text-3xl font-black tracking-tight lg:text-4xl">
          {t('home.title')}
        </h1>
        <p className="text-muted max-w-prose">{t('home.subtitle')}</p>
      </div>
      <YourMusic />
      <RecentlyPlayed />
    </section>
  );
};
