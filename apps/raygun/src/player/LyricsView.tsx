import type { LyricLine as SyncedLine, QueueItem, TextAlign } from '@raygun/bindings';
import { PenLine, TextCursorInput } from 'lucide-react';
import { memo, useEffect, useRef } from 'react';
import type { FC, Ref, RefObject } from 'react';
import { useTranslation } from 'react-i18next';

import { openContextMenu } from '../shell/contextMenuStore';
import { SourceIssue } from '../shell/SourceIssue';
import { LyricsSourceMenu } from '../sound/LyricsSourceMenu';
import { LyricsTranslateMenu } from '../sound/LyricsTranslateMenu';
import { useShownTranslation } from '../sound/lyricsTranslation';
import type { ShownTranslation } from '../sound/lyricsTranslation';
import { TranslatedPlain } from '../sound/TranslatedLines';
import { useSongEditStore } from '../sound/songEditStore';
import { currentLine, trackQueryFor, useLyricsFor } from './lyricsQuery';
import { usePlayerUiStore } from './playerUiStore';
import { useLyricsLook } from './playerView';
import { useLiveValue } from './useLivePosition';
import { sendControl, usePlayerTiming } from './usePlayback';
import { scrollMotion } from '../shell/deviceState';

type Variant = 'sheet' | 'panel';

type LyricsViewProps = {
  item: QueueItem;
  /** `panel`: the smaller card in the desktop side panel (M46.7). */
  variant?: Variant;
};

/** How long after the listener scrolls the lyrics they stay where they were put, ms. */
const HOLD_AFTER_SCROLL_MS = 4000;

/**
 * Lyrics for the playing item: synced lines follow playback, tapping one seeks there. Text is
 * sized by the width it has, at any window size, zoom or orientation; Settings → Player moves it.
 */
export const LyricsView: FC<LyricsViewProps> = ({ item, variant = 'sheet' }) => {
  const { t } = useTranslation();
  const query = trackQueryFor(item);
  const lyrics = useLyricsFor(query);
  const { align, style } = useLyricsLook();
  // Translations and romanized lines under each line (M46.48).
  const shown = useShownTranslation(query, lyrics.data);
  const selectable = usePlayerUiStore((store) => store.lyricsSelectable);
  const setSelectable = usePlayerUiStore((store) => store.setLyricsSelectable);
  const containerRef = useRef<HTMLDivElement>(null);
  const synced = lyrics.data?.synced ?? [];
  const hasLines = synced.length > 0 || Boolean(lyrics.data?.plain);
  // The panel's card shrinks to a line while loading, without lyrics or after an error.
  const height = variant === 'panel' ? (hasLines ? 'h-[min(26rem,60svh)]' : 'h-auto') : '';

  const message = (text: string) => <p className="text-muted text-center text-sm">{text}</p>;

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
      style={style}
      className={`@container relative flex min-h-0 flex-col overflow-y-auto overscroll-contain ${
        hasLines || variant === 'sheet' ? 'py-4' : 'py-3'
      } ${variant === 'sheet' ? 'w-full flex-1 pr-4' : 'w-full px-4'} ${height} ${
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
        <SyncedLyrics
          lines={synced}
          offsetMs={lyrics.data?.offsetMs ?? 0}
          variant={variant}
          align={align}
          container={containerRef}
          shown={shown}
        />
      ) : lyrics.data?.plain && (shown.translation || shown.romanization) ? (
        <TranslatedPlain lyrics={lyrics.data} shown={shown} variant={variant} align={align} />
      ) : (
        lyrics.data?.plain && (
          <p
            data-variant={variant}
            className={`lyric-plain leading-relaxed whitespace-pre-line ${
              align === 'center' ? 'text-center' : 'text-left'
            }`}
          >
            {lyrics.data.plain}
          </p>
        )
      )}
      {/* The lyrics' tools: where they come from, fixing them, and room for one more button. */}
      <div
        data-testid="lyrics-tools"
        className={`flex flex-wrap items-center gap-3 ${
          hasLines || variant === 'sheet' ? 'mt-6' : 'mt-2 justify-center'
        }`}
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

/**
 * The synced lines. Only this follows the playhead, and only the line that starts and the one
 * that ends re-render when it moves on.
 */
const SyncedLyrics: FC<{
  lines: SyncedLine[];
  offsetMs: number;
  variant: Variant;
  align: TextAlign;
  container: RefObject<HTMLDivElement | null>;
  /** Translations and romanized lines under the lines (M46.48). */
  shown: ShownTranslation;
}> = ({ lines, offsetMs, variant, align, container, shown }) => {
  const timing = usePlayerTiming();
  const current = useLiveValue(timing, (position) => currentLine(lines, position, offsetMs));
  const currentRef = useRef<HTMLButtonElement>(null);
  const heldUntil = useRef(0);

  useEffect(() => {
    // The listener scrolled: the lyrics stay where they put them for a moment.
    const box = container.current;
    if (!box) return;
    const hold = () => {
      heldUntil.current = performance.now() + HOLD_AFTER_SCROLL_MS;
    };
    box.addEventListener('wheel', hold, { passive: true });
    box.addEventListener('touchmove', hold, { passive: true });
    return () => {
      box.removeEventListener('wheel', hold);
      box.removeEventListener('touchmove', hold);
    };
  }, [container]);

  useEffect(() => {
    // Scrolls only the lyrics, not what they sit in (the side panel scrolls too); jsdom has no
    // scrollTo on elements.
    const box = container.current;
    const line = currentRef.current;
    if (!box || !line || performance.now() < heldUntil.current) return;
    box.scrollTo?.({
      top: line.offsetTop - box.clientHeight / 2 + line.clientHeight / 2,
      behavior: scrollMotion(),
    });
  }, [current, container]);

  return (
    <ol className="flex flex-col gap-[0.6em]" data-variant={variant}>
      {lines.map((line, index) => (
        <LyricLine
          key={`${line.timeMs}-${index}`}
          text={line.text}
          seekMs={line.timeMs + offsetMs}
          playing={index === current}
          variant={variant}
          align={align}
          translation={shown.translation?.lines[index] || null}
          romanized={shown.romanization?.lines[index] || null}
          lineRef={index === current ? currentRef : undefined}
        />
      ))}
    </ol>
  );
};

type LyricLineProps = {
  text: string;
  /** Where a tap on the line seeks: its time plus the song's lyrics offset. */
  seekMs: number;
  playing: boolean;
  variant: Variant;
  align: TextAlign;
  /** A translation of the line, shown under it. */
  translation?: string | null;
  /** The line in Latin letters, shown under it (M46.48). */
  romanized?: string | null;
  lineRef?: Ref<HTMLButtonElement>;
};

/** One line of synced lyrics; it re-renders only when it starts or stops being the one sung. */
export const LyricLine: FC<LyricLineProps> = memo(function LyricLine({
  text,
  seekMs,
  playing,
  variant,
  align,
  translation,
  romanized,
  lineRef,
}) {
  return (
    <li>
      <button
        type="button"
        ref={lineRef}
        aria-current={playing ? 'true' : undefined}
        onClick={() => void sendControl({ type: 'seek', positionMs: seekMs })}
        data-variant={variant}
        className={`lyric-text w-full leading-snug font-semibold transition-colors ${
          align === 'center' ? 'text-center' : 'text-left'
        } ${playing ? 'text-foreground' : 'text-muted/60'}`}
      >
        {text || '♪'}
        {romanized && (
          <span
            data-testid="lyrics-romanized"
            className="mt-0.5 block text-[0.62em] leading-snug font-medium"
          >
            {romanized}
          </span>
        )}
        {translation && (
          <span
            data-testid="lyrics-translation"
            className="mt-0.5 block text-[0.7em] leading-snug font-medium opacity-80"
          >
            {translation}
          </span>
        )}
      </button>
    </li>
  );
});
