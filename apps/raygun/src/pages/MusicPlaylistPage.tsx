import { commands } from '@raygun/bindings';
import type { MusicPlaylist, OnlineTrack } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { Download, LibraryBig, LoaderCircle, Play, Shuffle } from 'lucide-react';
import { useMemo } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { COLLECTIONS, fromOnlineTrack } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { usePlayContext } from '../music/playContext';
import { downloadSongs } from '../music/useSongDownloads';
import { OnlineTrackRow } from '../search/OnlineTrackRow';
import { playResults } from '../search/useOnlineSearch';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar } from '../shell/TopBar';
import { findInCache } from '../shell/queryMemory';

/**
 * A source's playlist (M46.39): its songs to play, shuffle or download, and Save to library,
 * which makes a Raygun playlist of them under the same name.
 */
export const MusicPlaylistPage: FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const client = useQueryClient();
  const { provider, id } = useSearch({ from: '/music/playlist' });
  const playlist = useQuery({
    queryKey: ['music', 'playlist', provider, id],
    queryFn: () => unwrapResult(commands.musicPlaylist(provider, id)),
    enabled: provider !== '' && id !== '',
    staleTime: 30 * 60_000,
  });
  const tracks = playlist.data?.tracks ?? [];
  // The playlist as the search showed it (M46.40), until its songs come.
  const known = useMemo(
    () =>
      findInCache(client, ['music'], (record) =>
        typeof record.title === 'string' &&
        record.id === id &&
        (record.provider as { id?: string } | undefined)?.id === provider &&
        'featured' in record
          ? (record as unknown as MusicPlaylist)
          : null,
      ),
    [client, provider, id],
  );
  const info = playlist.data?.playlist ?? known ?? undefined;
  usePlayContext(
    info
      ? {
          kind: 'playlist',
          name: info.title,
          subtitle: info.author,
          artworkUrl: info.artworkUrl ?? tracks[0]?.artworkUrl ?? null,
          to: '/music/playlist',
          search: { provider, id },
        }
      : null,
  );
  const save = useMutation({
    mutationFn: async () => {
      const created = await unwrapResult(
        commands.playlistCreate(info?.title ?? t('playlist.saved')),
      );
      await unwrapResult(commands.playlistAdd(created.id, tracks.map(fromOnlineTrack)));
      return created.id;
    },
    onSuccess: (playlistId) => {
      void client.invalidateQueries({ queryKey: [COLLECTIONS] });
      void navigate({ to: '/library/playlist', search: { id: playlistId } });
    },
  });
  const play = (shuffle: boolean) => {
    if (tracks.length === 0) return;
    const start = shuffle ? tracks[Math.floor(Math.random() * tracks.length)] : tracks[0];
    void playResults(tracks, start as OnlineTrack);
  };
  const button = 'bg-surface flex items-center gap-2 rounded-full px-6 py-2.5 disabled:opacity-50';

  return (
    <section data-testid="music-playlist-page" className="flex flex-col gap-6">
      <TopBar back fallback="/browse" plainTitle title={t('playlist.title')} />
      <header className="flex flex-col items-center gap-4 sm:flex-row sm:items-end sm:gap-6">
        <Artwork
          url={info?.artworkUrl ?? tracks[0]?.artworkUrl ?? null}
          className="size-40 shrink-0 rounded-lg shadow-xl sm:size-48"
          iconSize={56}
        />
        <div className="flex min-w-0 flex-col items-center gap-2 sm:items-start">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t(info?.featured ? 'playlist.featuredKind' : 'playlist.kind')}
          </span>
          <h1 className="text-center text-3xl font-black tracking-tight sm:text-left sm:text-5xl">
            {info?.title ?? ''}
          </h1>
          <p className="text-muted text-sm">
            {[
              info?.author,
              tracks.length > 0 ? t('discover.songs', { count: tracks.length }) : null,
              info?.provider.name,
            ]
              .filter(Boolean)
              .join(' · ')}
          </p>
        </div>
      </header>
      <div className="flex flex-wrap gap-3">
        <button
          type="button"
          disabled={tracks.length === 0}
          onClick={() => play(false)}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold disabled:opacity-50"
        >
          <Play aria-hidden size={18} fill="currentColor" />
          {t('library.playAll')}
        </button>
        <button
          type="button"
          disabled={tracks.length === 0}
          onClick={() => play(true)}
          className={button}
        >
          <Shuffle aria-hidden size={18} />
          {t('library.shuffle')}
        </button>
        <button
          type="button"
          disabled={tracks.length === 0 || save.isPending}
          onClick={() => save.mutate()}
          className={button}
        >
          <LibraryBig aria-hidden size={18} />
          {t('playlist.save')}
        </button>
        <button
          type="button"
          disabled={tracks.length === 0}
          onClick={() => void downloadSongs(tracks.map(fromOnlineTrack))}
          className={button}
        >
          <Download aria-hidden size={18} />
          {t('playlist.download')}
        </button>
      </div>
      {playlist.isPending && provider !== '' && (
        <LoaderCircle aria-label={t('playlist.loading')} className="text-muted animate-spin" />
      )}
      {(playlist.error ?? save.error) && (
        <SourceIssue>{(playlist.error ?? save.error)?.message}</SourceIssue>
      )}
      <ol className="flex flex-col">
        {tracks.map((track, index) => (
          <li key={`${track.title}-${index}`} className="flex items-center gap-3">
            <span className="text-muted w-6 shrink-0 text-right text-sm tabular-nums">
              {index + 1}
            </span>
            <div className="min-w-0 flex-1">
              <OnlineTrackRow track={track} onPlay={(tapped) => void playResults(tracks, tapped)} />
            </div>
          </li>
        ))}
      </ol>
    </section>
  );
};
