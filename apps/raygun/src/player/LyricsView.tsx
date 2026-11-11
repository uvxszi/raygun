import type { LyricLine as SyncedLine, QueueItem, TextAlign } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Package, PenLine, TextCursorInput } from 'lucide-react';
import { memo, useMemo, useRef } from 'react';
import type { FC, Ref, RefObject } from 'react';
import { useTranslation } from 'react-i18next';

import { useHasProvider } from '../extensions/useExtensions';
import { SourceIssue } from '../shell/SourceIssue';
import { LyricsSourceMenu } from '../sound/LyricsSourceMenu';
import { LyricsTranslateMenu } from '../sound/LyricsTranslateMenu';
import { NOTHING_SHOWN, useShownTranslation } from '../sound/lyricsTranslation';
import type { ShownTranslation } from '../sound/lyricsTranslation';
import { TranslatedPlain } from '../sound/TranslatedLines';
import { useSongEditStore } from '../sound/songEditStore';
import { heardLines, useSongTranscription } from '../sound/useSongTranscription';
import { trackQueryFor, useLyricsFor } from './lyricsQuery';
import { usePlayerUiStore } from './playerUiStore';
import { useLyricsLook } from './playerView';
import { useCurrentLine, useFollowLine } from './useFollowLine';
import { sendControl, useReadPosition } from './usePlayback';

type Variant = 'sheet' | 'panel';

type LyricsViewProps = {
  item: QueueItem;
  /** `panel`: the smaller card in the desktop side panel (M46.7). */
  variant?: Variant;
};

/**
 * Lyrics for the playing item: synced lines follow playback, tapping one seeks there. Text is
 * sized by the width it has, at any window size, zoom or orientation; Settings → Player moves it.
 */
export const LyricsView: FC<LyricsViewProps> = ({ item, variant = 'sheet' }) => {
  const { t } = useTranslation();
  const query = trackQueryFor(item);
  const lyrics = useLyricsFor(query);
  // Without a lyrics source, the way to one sits with the tools.
  const noSources = useHasProvider('lyrics') === false;
  const { align, style } = useLyricsLook();
  // Translations and romanized lines under each line (M46.48).
  const shown = useShownTranslation(query, lyrics.data);
  const selectable = usePlayerUiStore((store) => store.lyricsSelectable);
  const setSelectable = usePlayerUiStore((store) => store.setLyricsSelectable);
  const containerRef = useRef<HTMLDivElement>(null);
  const readPosition = useReadPosition();
  // While the song's words are being heard, each window's lines show as it's done.
  const transcription = useSongTranscription(query);
  const heard = useMemo(() => heardLines(transcription.captions), [transcription.captions]);
  const synced = heard ?? lyrics.data?.synced ?? [];
  const hasLines = synced.length > 0 || Boolean(lyrics.data?.plain);
  // The panel's card shrinks to a line while loading, without lyrics or after an error.
  const height = variant === 'panel' ? (hasLines ? 'h-[min(26rem,60svh)]' : 'h-auto') : '';

  const message = (text: string) => <p className="text-muted text-center text-sm">{text}</p>;

  return (
    <div
      ref={containerRef}
      data-testid="lyrics-view"
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
          offsetMs={heard ? 0 : (lyrics.data?.offsetMs ?? 0)}
          variant={variant}
          align={align}
          container={containerRef}
          shown={heard ? NOTHING_SHOWN : shown}
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
          <LyricsTranslateMenu
            query={query}
            lyrics={lyrics.data}
            shown={shown}
            readPosition={readPosition}
          />
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
        {hasLines && (
          <button
            type="button"
            aria-pressed={selectable}
            onClick={() => setSelectable(!selectable)}
            className={`hover:text-foreground flex items-center gap-1 text-xs ${selectable ? 'text-primary' : 'text-muted'}`}
          >
            <TextCursorInput aria-hidden size={12} />
            {t(selectable ? 'lyricsSelect.off' : 'lyricsSelect.on')}
          </button>
        )}
        {noSources && lyrics.isSuccess && !lyrics.data && (
          <Link
            to="/browse"
            search={{ tab: 'extensions', list: 'all' }}
            className="text-muted hover:text-foreground flex items-center gap-1 text-xs"
          >
            <Package aria-hidden size={12} />
            {t('search.findExtensions')}
          </Link>
        )}
      </div>
    </div>
  );
};

/**
 * The synced lines. Only this follows the playhead, and only the line that starts and the one
 * that ends re-render when it moves on: the line sung eases in (styles.css `.lyric-line`) as the
 * lines glide to put it in the middle (useFollowLine). Only the lyrics scroll, never what they sit
 * in (the side panel scrolls too).
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
  const starts = useMemo(() => lines.map((line) => line.timeMs + offsetMs), [lines, offsetMs]);
  const current = useCurrentLine(starts);
  const { lineRef, listRef } = useFollowLine(container, current, lines);

  return (
    <ol ref={listRef} className="flex flex-col gap-[0.6em]" data-variant={variant}>
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
          lineRef={index === current ? lineRef : undefined}
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

/**
 * One line of synced lyrics; it re-renders only when it starts or stops being the one sung, and
 * eases between lit and resting (styles.css `.lyric-line`).
 */
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
        data-align={align}
        className={`lyric-text lyric-line text-foreground w-full leading-snug font-semibold ${
          align === 'center' ? 'text-center' : 'text-left'
        }`}
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
