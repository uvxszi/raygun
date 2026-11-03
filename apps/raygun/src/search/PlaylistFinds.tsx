import type { MusicPlaylist } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { SourceIssue } from '../shell/SourceIssue';
import { Asking } from './MusicFinds';
import { usePlaylistFinds } from './useMusicFinds';

/** A source's playlist as a cover with its name, author, size and source. */
export const PlaylistTile: FC<{ playlist: MusicPlaylist }> = ({ playlist }) => {
  const { t } = useTranslation();
  return (
    <Link
      to="/music/playlist"
      search={{ provider: playlist.provider.id, id: playlist.id }}
      className="hover:bg-surface flex flex-col gap-1.5 rounded-xl p-2"
    >
      <Artwork url={playlist.artworkUrl} className="w-full rounded-lg shadow" iconSize={36} />
      <span className="line-clamp-2 text-sm font-semibold">{playlist.title}</span>
      <span className="text-muted truncate text-xs">
        {[
          playlist.author,
          playlist.trackCount ? t('library.trackCount', { count: playlist.trackCount }) : null,
          playlist.provider.name,
        ]
          .filter(Boolean)
          .join(' · ')}
      </span>
    </Link>
  );
};

/**
 * Search → Music → Playlists (M46.39): the sources' playlists, with a Featured chip for the
 * services' own (YouTube Music's featured playlists, Deezer's editors'), as Kreate and RiMusic
 * offer them. Each opens its songs.
 */
export const PlaylistFinds: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const [featured, setFeatured] = useState(false);
  const playlists = usePlaylistFinds(query, featured);
  const chip = (on: boolean) =>
    `shrink-0 rounded-full px-3 py-1 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`;
  return (
    <div className="flex flex-col gap-3">
      <div role="group" aria-label={t('search.playlists.which')} className="flex gap-2">
        {[false, true].map((value) => (
          <button
            key={String(value)}
            type="button"
            aria-pressed={featured === value}
            onClick={() => setFeatured(value)}
            className={chip(featured === value)}
          >
            {t(value ? 'search.playlists.featured' : 'search.playlists.all')}
          </button>
        ))}
      </div>
      {playlists.error ? (
        <SourceIssue>{playlists.error.message}</SourceIssue>
      ) : playlists.list.length === 0 ? (
        playlists.isFetching ? (
          <Asking progress={playlists.progress} />
        ) : (
          <p className="text-muted text-sm">{t('search.playlists.none', { query })}</p>
        )
      ) : (
        <>
          <ul
            data-testid="playlist-finds"
            className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3"
          >
            {playlists.list.map((playlist) => (
              <li key={`${playlist.provider.id}:${playlist.id}`}>
                <PlaylistTile playlist={playlist} />
              </li>
            ))}
          </ul>
          {playlists.progress && <Asking progress={playlists.progress} />}
        </>
      )}
    </div>
  );
};
