import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { menuHandler, useSourcePlaylistMenuItems } from '../music/cardMenus';
import { SourceIssue } from '../shell/SourceIssue';
import { titleScore } from './rankResults';

/** The sources' playlists for a search (M46.39); `featured` keeps the services' own. */
const usePlaylistFinds = (query: string, featured: boolean) => {
  const playlists = useQuery({
    queryKey: ['music', 'searchPlaylists', query.trim(), featured],
    queryFn: () => unwrapResult(commands.musicSearchPlaylists(query.trim(), featured)),
    enabled: query.trim() !== '',
    staleTime: 5 * 60_000,
  });
  const list = [...(playlists.data ?? [])].sort(
    (a, b) => titleScore(b.title, query) - titleScore(a.title, query),
  );
  return { ...playlists, list };
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
      {playlists.isPending ? (
        <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />
      ) : playlists.error ? (
        <SourceIssue>{playlists.error.message}</SourceIssue>
      ) : playlists.list.length === 0 ? (
        <p className="text-muted text-sm">{t('search.playlists.none', { query })}</p>
      ) : (
        <ul
          data-testid="playlist-finds"
          className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3"
        >
          {playlists.list.map((playlist) => (
            <li key={`${playlist.provider.id}:${playlist.id}`}>
              <Link
                to="/music/playlist"
                search={{ provider: playlist.provider.id, id: playlist.id }}
                onContextMenu={menuHandler(() => menu(playlist))}
                className="hover:bg-surface flex flex-col gap-1.5 rounded-xl p-2"
              >
                <Artwork
                  url={playlist.artworkUrl}
                  className="w-full rounded-lg shadow"
                  iconSize={36}
                />
                <span className="line-clamp-2 text-sm font-semibold">{playlist.title}</span>
                <span className="text-muted truncate text-xs">
                  {[
                    playlist.author,
                    playlist.trackCount
                      ? t('library.trackCount', { count: playlist.trackCount })
                      : null,
                    playlist.provider.name,
                  ]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};
