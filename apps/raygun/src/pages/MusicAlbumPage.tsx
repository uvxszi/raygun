import { commands } from '@raygun/bindings';
import type { OnlineTrack } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useSearch } from '@tanstack/react-router';
import { Download, LoaderCircle, Pencil, Play, Shuffle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { fromOnlineTrack } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { useCoverSheetStore } from '../music/coverSheetStore';
import { ArtistLinks } from '../music/ArtistLinks';
import { downloadSongs } from '../music/useSongDownloads';
import { OnlineTrackRow } from '../search/OnlineTrackRow';
import { playResults } from '../search/useOnlineSearch';
import { Sheet } from '../shell/Sheet';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { DetailEditForm } from '../music/DetailEditForm';
import { PinButton } from '../shell/PinButton';
import { SourceIssue } from '../shell/SourceIssue';

/** An album from a music source (M46.9): its songs in order, to play, save or download. */
export const MusicAlbumPage: FC = () => {
  const { t } = useTranslation();
  const { provider, id } = useSearch({ from: '/music/album' });
  const album = useQuery({
    queryKey: ['music', 'album', provider, id],
    queryFn: () => unwrapResult(commands.musicAlbum(provider, id)),
    enabled: provider !== '' && id !== '',
    staleTime: 30 * 60_000,
  });
  const tracks = album.data?.tracks ?? [];
  const info = album.data?.album;
  // Edit album (M46.30): its title and cover over what the source said.
  const [editing, setEditing] = useState(false);
  const play = (shuffle: boolean) => {
    if (tracks.length === 0) return;
    const start = shuffle ? tracks[Math.floor(Math.random() * tracks.length)] : tracks[0];
    void playResults(tracks, start as OnlineTrack);
  };

  return (
    <section data-testid="music-album-page" className="flex flex-col gap-6">
      <TopBar
        back
        fallback="/browse"
        plainTitle
        title={t('album.title')}
        actions={
          <>
            {info && (
              <TopBarAction
                label={t('details.editAlbum')}
                icon={Pencil}
                onClick={() => setEditing(true)}
              />
            )}
            <PinButton
              target={
                info
                  ? {
                      kind: 'album',
                      to: '/music/album',
                      search: { provider: info.provider.id, id: info.id },
                      name: info.title,
                      artworkUrl: info.artworkUrl ?? null,
                    }
                  : null
              }
            />
          </>
        }
      />
      {editing && info && (
        <Sheet title={t('details.editAlbum')} onClose={() => setEditing(false)}>
          <DetailEditForm
            target={{ kind: 'album', artist: info.artist, album: info.title }}
            fields={['title']}
            original={{ title: info.title, artworkUrl: info.artworkUrl }}
            titleLabel={t('details.albumTitle')}
            onSaved={() => setEditing(false)}
          />
        </Sheet>
      )}
      <header className="flex flex-col items-center gap-4 sm:flex-row sm:items-end sm:gap-6">
        {/* The cover large, to save as an image (M46.26). */}
        <button
          type="button"
          aria-label={t('cover.open')}
          disabled={!(info?.artworkUrl ?? tracks[0]?.artworkUrl)}
          onClick={() => {
            const url = info?.artworkUrl ?? tracks[0]?.artworkUrl;
            if (url) {
              useCoverSheetStore.getState().open({
                url,
                title: [info?.artist, info?.title].filter(Boolean).join(' - '),
              });
            }
          }}
          className="shrink-0 cursor-zoom-in disabled:cursor-default"
        >
          <Artwork
            url={info?.artworkUrl ?? tracks[0]?.artworkUrl ?? null}
            className="size-40 rounded-lg shadow-xl sm:size-48"
            iconSize={56}
          />
        </button>
        <div className="flex min-w-0 flex-col items-center gap-2 sm:items-start">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t('album.kind')}
          </span>
          <h1 className="text-center text-3xl font-black tracking-tight sm:text-left sm:text-5xl">
            {info?.title ?? ''}
          </h1>
          <p className="text-muted flex flex-wrap items-center gap-1 text-sm">
            {info?.artist && <ArtistLinks credit={info.artist} />}
            {info?.year && <span>· {info.year}</span>}
            {tracks.length > 0 && <span>· {t('discover.songs', { count: tracks.length })}</span>}
            {info && <span>· {info.provider.name}</span>}
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
          className="bg-surface flex items-center gap-2 rounded-full px-6 py-2.5 disabled:opacity-50"
        >
          <Shuffle aria-hidden size={18} />
          {t('library.shuffle')}
        </button>
        <button
          type="button"
          disabled={tracks.length === 0}
          onClick={() => void downloadSongs(tracks.map(fromOnlineTrack))}
          className="bg-surface flex items-center gap-2 rounded-full px-6 py-2.5 disabled:opacity-50"
        >
          <Download aria-hidden size={18} />
          {t('album.download')}
        </button>
      </div>
      {album.isPending && provider !== '' && (
        <LoaderCircle aria-label={t('album.loading')} className="text-muted animate-spin" />
      )}
      {album.error && <SourceIssue>{album.error.message}</SourceIssue>}
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
