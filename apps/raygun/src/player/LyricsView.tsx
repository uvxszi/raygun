import type { PlaybackState, QueueItem, TextAlign } from '@raygun/bindings';
import { PenLine, TextCursorInput } from 'lucide-react';
import { useEffect, useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { openContextMenu } from '../shell/contextMenuStore';
import { useSongEditStore } from '../sound/songEditStore';
import { usePlayerUiStore } from './playerUiStore';
import { currentLine, trackQueryFor, useLyricsFor } from './lyricsQuery';
import { useLivePosition } from './useLivePosition';
import { sendControl } from './usePlayback';
import { SourceIssue } from '../shell/SourceIssue';
import { LyricsSourceMenu } from '../sound/LyricsSourceMenu';
import { LyricsTranslateMenu } from '../sound/LyricsTranslateMenu';
import { useShownTranslation } from '../sound/lyricsTranslation';
import { TranslatedLine, TranslatedPlain } from '../sound/TranslatedLines';

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
  // Tall only once there are lines to show (M46.24); see `hasLines`.
  panel: 'w-full px-4',
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
  // Translations and romanized lines under each line (M46.48).
  const shown = useShownTranslation(query, lyrics.data);
  // The song's own offset (M46.12): lines show this much later, and seek to where they show.
  const offsetMs = lyrics.data?.offsetMs ?? 0;
  const positionMs = useLivePosition(state);
  const containerRef = useRef<HTMLDivElement>(null);
  const currentRef = useRef<HTMLButtonElement>(null);
  const selectable = usePlayerUiStore((store) => store.lyricsSelectable);
  const setSelectable = usePlayerUiStore((store) => store.setLyricsSelectable);

  const alignClass = align === 'center' ? 'text-center' : 'text-left';
  const synced = lyrics.data?.synced ?? [];
  const hasLines = synced.length > 0 || Boolean(lyrics.data?.plain);
  // The panel's card shrinks to a line while loading, without lyrics or after an error.
  const height = variant === 'panel' ? (hasLines ? 'h-[26rem]' : 'h-auto') : '';
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
      onContextMenu={(event) =>
        openContextMenu(event, [
          {
            label: t(selectable ? 'lyricsSelect.off' : 'lyricsSelect.on'),
            icon: TextCursorInput,
            onSelect: () => setSelectable(!selectable),
          },
        ])
      }
      className={`relative flex min-h-0 flex-col overflow-y-auto ${hasLines || variant === 'sheet' ? 'py-4' : 'py-3'} ${CONTAINER[variant]} ${height} ${
        selectable ? 'selectable' : ''
      }`}
    >
      {lyrics.isPending && message(t('player.lyricsLoading'))}
      {lyrics.error && (
        <div className="px-4">
          <SourceIssue>{lyrics.error.message}</SourceIssue>
        </div>
      )}
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
                <TranslatedLine shown={shown} index={index} />
              </button>
            </li>
          ))}
        </ol>
      ) : lyrics.data?.plain && (shown.translation || shown.romanization) ? (
        <TranslatedPlain
          lyrics={lyrics.data}
          shown={shown}
          size={size}
          align={align}
          compact={variant === 'panel'}
        />
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
      <div
        className={`flex flex-wrap items-center gap-3 ${hasLines || variant === 'sheet' ? 'mt-6' : 'mt-2 justify-center'}`}
      >
        {/* One dropdown to switch where the lyrics come from (M46.45). */}
        {query && !lyrics.isPending && (
          <LyricsSourceMenu query={query} current={lyrics.data ?? undefined} />
        )}
        {lyrics.data && hasLines && (
          <LyricsTranslateMenu query={query} lyrics={lyrics.data} shown={shown} />
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
