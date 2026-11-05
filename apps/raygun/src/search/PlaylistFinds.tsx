import type { MusicPlaylist } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { useSourcePlaylistMenuItems } from '../music/cardMenus';
import { CardMenu } from '../shell/CardMenu';
import type { MenuItem } from '../shell/contextMenuStore';
import { CARD_MENU_BESIDE } from '../manga/CardPlayButton';

import { SourceIssue } from '../shell/SourceIssue';
import { Asking } from './MusicFinds';
import { NoSources } from './NoSources';
import { usePlaylistFinds } from './useMusicFinds';
import { SourceTag } from '../shell/SourceTag';

/** A source's playlist as a cover with its name, author, size and source. */
export const PlaylistTile: FC<{
  playlist: MusicPlaylist;
  /** Its ⋮ menu (made once for the whole list). */
  menu?: () => MenuItem[];
}> = ({ playlist, menu }) => {
  const { t } = useTranslation();
  return (
    <Link
      to="/music/playlist"
      search={{ provider: playlist.provider.id, id: playlist.id }}
      className="hover:bg-surface relative flex flex-col gap-1.5 rounded-xl p-2"
    >
      {menu && (
        <CardMenu
          label={t('menu.options', { name: playlist.title })}
          items={menu}
          className={CARD_MENU_BESIDE}
        />
      )}
      <Artwork url={playlist.artworkUrl} className="w-full rounded-lg shadow" iconSize={36} />
      <span className="line-clamp-2 text-sm font-semibold">{playlist.title}</span>
      <span className="text-muted truncate text-xs">
        {[
          playlist.author,
          playlist.trackCount ? t('library.trackCount', { count: playlist.trackCount }) : null,
        ]
          .filter(Boolean)
          .join(' · ')}
      </span>
      <span className="flex min-w-0">
        <SourceTag name={playlist.provider.name} />
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
  const menu = useSourcePlaylistMenuItems();
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
          // Playlists come only from sources that have them; none may be installed.
          <NoSources label={t('search.playlists.none', { query })} />
        )
      ) : (
        <>
          <ul
            data-testid="playlist-finds"
            className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3"
          >
            {playlists.list.map((playlist) => (
              <li key={`${playlist.provider.id}:${playlist.id}`}>
                <PlaylistTile playlist={playlist} menu={() => menu(playlist)} />
              </li>
            ))}
          </ul>
          {playlists.progress && <Asking progress={playlists.progress} />}
        </>
      )}
    </div>
  );
};
