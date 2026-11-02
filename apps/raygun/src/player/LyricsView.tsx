import type { PlaybackState, QueueItem, TextAlign } from '@raygun/bindings';
import { PenLine } from 'lucide-react';
import { useEffect, useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSongEditStore } from '../sound/songEditStore';
import { currentLine, trackQueryFor, useLyricsFor } from './lyricsQuery';
import { useLivePosition } from './useLivePosition';
import { sendControl } from './usePlayback';

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
  const query = trackQueryFor(item);
  const lyrics = useLyricsFor(query);
  // The song's own offset (M46.12): lines show this much later, and seek to where they show.
  const offsetMs = lyrics.data?.offsetMs ?? 0;
  const positionMs = useLivePosition(state);
  const containerRef = useRef<HTMLDivElement>(null);
  const currentRef = useRef<HTMLButtonElement>(null);

  const alignClass = align === 'center' ? 'text-center' : 'text-left';
  const synced = lyrics.data?.synced ?? [];
  const current = currentLine(synced, positionMs, offsetMs);

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
                onClick={() =>
                  void sendControl({ type: 'seek', positionMs: line.timeMs + offsetMs })
                }
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
      <div className="mt-6 flex items-center gap-3">
        {lyrics.data && (
          <p className="text-muted text-xs">
            {t('player.lyricsFrom', { source: lyrics.data.providerName })}
          </p>
        )}
        {!lyrics.isPending && (
          <button
            type="button"
            onClick={() => useSongEditStore.getState().open(query, { tab: 'lyrics' })}
            className="text-muted hover:text-foreground flex items-center gap-1 text-xs"
          >
            <PenLine aria-hidden size={12} />
            {t(lyrics.data ? 'lyricsEdit.fix' : 'lyricsEdit.add')}
          </button>
        )}
      </div>
    </div>
  );
};
