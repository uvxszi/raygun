import type { OnlineTrack } from '@raygun/bindings';
import { Link, useSearch } from '@tanstack/react-router';
import { ListPlus, LoaderCircle, Play, Shuffle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { mixDescription, mixTitle, useMix, useSaveMix } from '../music/useMadeForYou';
import { OnlineTrackRow } from '../search/OnlineTrackRow';
import { playResults } from '../search/useOnlineSearch';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar } from '../shell/TopBar';
import { PinButton } from '../shell/PinButton';

/** A Made for You mix (M46.20): its songs, played from any of them, and saved as a playlist. */
export const MixPage: FC = () => {
  const { t } = useTranslation();
  const { id } = useSearch({ from: '/music/mix' });
  const mix = useMix(id);
  const save = useSaveMix();
  const list = mix.data?.tracks ?? [];
  const title = mix.data ? mixTitle(t, mix.data) : t('forYou.title');
  const play = (shuffle: boolean) => {
    if (list.length === 0) return;
    const start = shuffle ? list[Math.floor(Math.random() * list.length)] : list[0];
    void playResults(list, start as OnlineTrack);
  };

  return (
    <section data-testid="mix-page" className="flex flex-col gap-6">
      <TopBar
        back
        fallback="/music/for-you"
        plainTitle
        title={t('forYou.title')}
        actions={
          <PinButton
            target={
              mix.data
                ? {
                    kind: 'mix',
                    to: '/music/mix',
                    search: { id },
                    name: title,
                    artworkUrl: mix.data.artworkUrls[0] ?? null,
                  }
                : null
            }
          />
        }
      />
      <header className="flex flex-col items-center gap-4 sm:flex-row sm:items-end sm:gap-6">
        <PlaylistCover
          artworkUrls={mix.data?.artworkUrls ?? []}
          className="size-40 rounded-lg shadow-xl sm:size-48"
        />
        <div className="flex min-w-0 flex-col items-center gap-2 sm:items-start">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t('forYou.title')}
          </span>
          <h1 className="text-center text-3xl font-black tracking-tight sm:text-left sm:text-5xl">
            {title}
          </h1>
          {mix.data && <p className="text-muted text-sm">{mixDescription(t, mix.data)}</p>}
          {list.length > 0 && (
            <p className="text-muted text-sm">{t('discover.songs', { count: list.length })}</p>
          )}
        </div>
      </header>
      <div className="flex flex-wrap gap-3">
        <button
          type="button"
          disabled={list.length === 0}
          onClick={() => play(false)}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold disabled:opacity-50"
        >
          <Play aria-hidden size={18} fill="currentColor" />
          {t('library.playAll')}
        </button>
        <button
          type="button"
          disabled={list.length === 0}
          onClick={() => play(true)}
          className="bg-surface flex items-center gap-2 rounded-full px-6 py-2.5 disabled:opacity-50"
        >
          <Shuffle aria-hidden size={18} />
          {t('library.shuffle')}
        </button>
        <button
          type="button"
          disabled={list.length === 0 || save.isPending || save.isSuccess}
          onClick={() => save.mutate({ id, name: title })}
          className="bg-surface flex items-center gap-2 rounded-full px-6 py-2.5 disabled:opacity-50"
        >
          <ListPlus aria-hidden size={18} />
          {t('forYou.save')}
        </button>
      </div>
      {save.data && (
        <p role="status" className="text-muted text-sm">
          {t('forYou.saved')}{' '}
          <Link
            to="/library/playlist"
            search={{ id: save.data }}
            className="text-primary font-semibold"
          >
            {t('forYou.openPlaylist')}
          </Link>
        </p>
      )}
      {save.error && <SourceIssue>{save.error.message}</SourceIssue>}
      {mix.isPending && (
        <LoaderCircle aria-label={t('discover.loading')} className="text-muted animate-spin" />
      )}
      {mix.error && <SourceIssue>{mix.error.message}</SourceIssue>}
      <ol className="flex flex-col">
        {list.map((song, index) => (
          <li key={`${song.title}-${index}`} className="flex items-center gap-3">
            <span className="text-muted w-6 shrink-0 text-right text-sm tabular-nums">
              {index + 1}
            </span>
            <div className="min-w-0 flex-1">
              <OnlineTrackRow track={song} onPlay={(tapped) => void playResults(list, tapped)} />
            </div>
          </li>
        ))}
      </ol>
    </section>
  );
};
