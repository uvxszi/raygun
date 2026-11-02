import { commands } from '@raygun/bindings';
import type { PlaybackState, QueueItem, TextAlign, TrackQuery } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useLivePosition } from './useLivePosition';
import { sendControl } from './usePlayback';

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

type LyricsViewProps = {
  item: QueueItem;
  state: PlaybackState | undefined;
  /** `panel`: the smaller card in the desktop side panel (M46.7). */
  variant?: 'sheet' | 'panel';
  /** Text size in px; 0 picks it by screen (Settings → Player, M46.7). */
  size?: number;
  align?: TextAlign;
};

const CONTAINER = {
  // The whole column scrolls, so its scrollbar sits at the column's edge, away from the words.
  sheet: 'w-full flex-1 pr-4',
  panel: 'h-[26rem] w-full px-4',
} as const;

/** Line sizes by screen when no size is chosen. */
const LINE_SIZE = {
  sheet: 'text-xl md:text-2xl lg:text-3xl',
  panel: 'text-lg',
} as const;

/** Lyrics for the playing item: synced lines follow playback, tapping one seeks there. */
export const LyricsView: FC<LyricsViewProps> = ({
  item,
  state,
  variant = 'sheet',
  size = 0,
  align = 'left',
}) => {
  const { t } = useTranslation();
  const lyrics = useLyrics(item);
  const positionMs = useLivePosition(state);
  const containerRef = useRef<HTMLDivElement>(null);
  const currentRef = useRef<HTMLButtonElement>(null);

  const alignClass = align === 'center' ? 'text-center' : 'text-left';
  const synced = lyrics.data?.synced ?? [];
  let current = -1;
  for (const [index, line] of synced.entries()) {
    if (line.timeMs <= positionMs) current = index;
    else break;
  }

  useEffect(() => {
    // Scrolls only the lyrics, not what they sit in (the side panel scrolls too); jsdom has no
    // scrollTo on elements.
    const container = containerRef.current;
    const line = currentRef.current;
    if (!container || !line) return;
    container.scrollTo?.({
      top: line.offsetTop - container.clientHeight / 2 + line.clientHeight / 2,
      behavior: 'smooth',
    });
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
      ref={containerRef}
      data-testid="lyrics-view"
      className={`relative flex min-h-0 flex-col overflow-y-auto py-4 ${CONTAINER[variant]}`}
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
                style={size > 0 ? { fontSize: size } : undefined}
                className={`w-full leading-snug font-semibold transition-colors ${alignClass} ${size > 0 ? '' : LINE_SIZE[variant]} ${
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
          <p
            style={size > 0 ? { fontSize: Math.round(size * 0.75) } : undefined}
            className={`leading-relaxed whitespace-pre-line ${alignClass} ${variant === 'panel' ? 'text-base' : 'text-lg md:text-xl'}`}
          >
            {lyrics.data.plain}
          </p>
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
