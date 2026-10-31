import { commands } from '@raygun/bindings';
import type { PlaybackState, QueueItem, TrackQuery } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { sendControl } from './usePlayback';

/** How often the highlighted line is re-checked between engine progress reports. */
const TICK_MS = 250;

/** The metadata lyrics are looked up by: a lookup's own query, else the item's fields. */
const trackQuery = (item: QueueItem): TrackQuery =>
  item.source.kind === 'lookup'
    ? {
        title: item.source.title,
        artist: item.source.artist,
        album: item.source.album,
        durationMs: item.source.durationMs,
      }
    : { title: item.title, artist: item.artist, album: item.album, durationMs: item.durationMs };

const useLyrics = (item: QueueItem) => {
  const query = trackQuery(item);
  return useQuery({
    queryKey: ['lyrics', query.title, query.artist, query.album, query.durationMs],
    queryFn: () => unwrapResult(commands.lyricsGet(query)),
    staleTime: Infinity,
    // Providers already wait out slow services; one more try is enough before showing the error.
    retry: 1,
  });
};

/**
 * The playback position, advanced locally while playing: engines report progress about once a
 * second, which is too coarse for line-by-line lyrics.
 */
const useLivePosition = (state: PlaybackState | undefined) => {
  const playing = state?.status === 'playing';
  const reported = state?.positionMs ?? 0;
  // How far playback has moved since `base` was reported; only the timer writes it.
  const [live, setLive] = useState({ base: reported, elapsedMs: 0 });

  useEffect(() => {
    if (!playing) return;
    const started = performance.now();
    const timer = setInterval(
      () => setLive({ base: reported, elapsedMs: performance.now() - started }),
      TICK_MS,
    );
    return () => clearInterval(timer);
  }, [playing, reported]);

  return playing && live.base === reported ? reported + live.elapsedMs : reported;
};

type LyricsViewProps = { item: QueueItem; state: PlaybackState | undefined };

/** Lyrics for the playing item: synced lines follow playback, tapping one seeks there. */
export const LyricsView: FC<LyricsViewProps> = ({ item, state }) => {
  const { t } = useTranslation();
  const lyrics = useLyrics(item);
  const positionMs = useLivePosition(state);
  const currentRef = useRef<HTMLButtonElement>(null);

  const synced = lyrics.data?.synced ?? [];
  let current = -1;
  for (const [index, line] of synced.entries()) {
    if (line.timeMs <= positionMs) current = index;
    else break;
  }

  useEffect(() => {
    // jsdom has no scrollIntoView.
    currentRef.current?.scrollIntoView?.({ block: 'center', behavior: 'smooth' });
  }, [current]);

  const message = (text: string, role?: 'alert') => (
    <p
      role={role}
      className={`text-center text-sm ${role ? 'text-primary wrap-anywhere' : 'text-muted'}`}
    >
      {text}
    </p>
  );

  return (
    <div
      data-testid="lyrics-view"
      className="flex min-h-0 w-full max-w-md flex-1 flex-col overflow-y-auto py-4"
    >
      {lyrics.isPending && message(t('player.lyricsLoading'))}
      {lyrics.error &&
        message(t('player.lyricsFailed', { message: lyrics.error.message }), 'alert')}
      {lyrics.isSuccess && !lyrics.data && message(t('player.lyricsNone'))}
      {synced.length > 0 ? (
        <ol className="flex flex-col gap-3">
          {synced.map((line, index) => (
            <li key={`${line.timeMs}-${index}`}>
              <button
                type="button"
                ref={index === current ? currentRef : undefined}
                aria-current={index === current ? 'true' : undefined}
                onClick={() => void sendControl({ type: 'seek', positionMs: line.timeMs })}
                className={`w-full text-left text-xl leading-snug font-semibold transition-colors ${
                  index === current ? 'text-foreground' : 'text-muted/60'
                }`}
              >
                {line.text || '♪'}
              </button>
            </li>
          ))}
        </ol>
      ) : (
        lyrics.data?.plain && (
          <p className="text-lg leading-relaxed whitespace-pre-line">{lyrics.data.plain}</p>
        )
      )}
      {lyrics.data && (
        <p className="text-muted mt-6 text-xs">
          {t('player.lyricsFrom', { source: lyrics.data.providerName })}
        </p>
      )}
    </div>
  );
};
