import type { Caption, Captions, SpokenItem, TextAlign } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { commands } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import {
  AudioLines,
  Check,
  ChevronDown,
  FileText,
  LoaderCircle,
  Square,
  Subtitles,
} from 'lucide-react';
import { memo, useEffect, useRef, useState } from 'react';
import type { FC, Ref, RefObject } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import { currentCue, useCaptions, useStopCaptions, useTranscribe } from './spoken';
import { useSongEdit } from '../sound/useSound';
import { useLyricsLook } from './playerView';
import { trackQueryFor } from './lyricsQuery';
import { useLiveValue } from './useLivePosition';
import { sendControl, useCurrentItem, usePlayerTiming, useReadPosition } from './usePlayback';
import { scrollMotion } from '../shell/deviceState';

type Variant = 'sheet' | 'panel';

type CaptionsViewProps = {
  spoken: SpokenItem;
  /** `panel`: the smaller card in the side panel and under the phone's player. */
  variant?: Variant;
};

/**
 * Captions for a podcast episode or a book's chapter (M46.32), in place of lyrics: the feed's
 * transcript or what AI heard, following playback; a tap on a line goes there. With none yet the
 * card is one slim line with a way to transcribe, or to set transcription up.
 */
export const CaptionsView: FC<CaptionsViewProps> = ({ spoken, variant = 'sheet' }) => {
  const { t } = useTranslation();
  const captions = useCaptions(spoken.episodeId);
  const { align, style } = useLyricsLook();
  const containerRef = useRef<HTMLDivElement>(null);
  const trims = useTrims();
  // Only what plays shows (M46.36): lines the item's trims cut off are left out.
  const cues = (captions.data?.cues ?? []).filter(
    (cue) => cue.endMs > trims.startMs && (trims.endMs === null || cue.startMs < trims.endMs),
  );
  const hasLines = cues.length > 0;
  const height = variant === 'panel' ? (hasLines ? 'h-[min(26rem,60svh)]' : 'h-auto') : '';

  return (
    <div
      ref={containerRef}
      data-testid="captions-view"
      style={style}
      className={`@container relative flex min-h-0 flex-col overflow-y-auto overscroll-contain ${
        hasLines || variant === 'sheet' ? 'py-4' : 'py-3'
      } ${variant === 'sheet' ? 'w-full flex-1 pr-4' : 'w-full px-4'} ${height}`}
    >
      {captions.isPending && (
        <p className="text-muted text-center text-sm">{t('captions.loading')}</p>
      )}
      {captions.error && <SourceIssue compact>{captions.error.message}</SourceIssue>}
      {hasLines && (
        <CaptionLines cues={cues} variant={variant} align={align} container={containerRef} />
      )}
      {captions.data && (
        <CaptionsFooter captions={captions.data} spoken={spoken} slim={!hasLines} />
      )}
    </div>
  );
};

/** The lines, following the playhead: only the line said and the one before it re-render. */
const CaptionLines: FC<{
  cues: Caption[];
  variant: Variant;
  align: TextAlign;
  container: RefObject<HTMLDivElement | null>;
}> = ({ cues, variant, align, container }) => {
  const timing = usePlayerTiming();
  const current = useLiveValue(timing, (position) => currentCue(cues, position));
  const currentRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const box = container.current;
    const line = currentRef.current;
    if (!box || !line) return;
    box.scrollTo?.({
      top: line.offsetTop - box.clientHeight / 2 + line.clientHeight / 2,
      behavior: scrollMotion(),
    });
  }, [current, container]);

  return (
    <ol className="flex flex-col gap-[0.6em]">
      {cues.map((cue, index) => (
        <CaptionLine
          key={cue.startMs}
          text={cue.text}
          startMs={cue.startMs}
          said={index === current}
          variant={variant}
          align={align}
          lineRef={index === current ? currentRef : undefined}
        />
      ))}
    </ol>
  );
};

const CaptionLine: FC<{
  text: string;
  startMs: number;
  said: boolean;
  variant: Variant;
  align: TextAlign;
  lineRef?: Ref<HTMLButtonElement>;
}> = memo(function CaptionLine({ text, startMs, said, variant, align, lineRef }) {
  return (
    <li>
      <button
        type="button"
        ref={lineRef}
        aria-current={said ? 'true' : undefined}
        onClick={() => void sendControl({ type: 'seek', positionMs: startMs })}
        data-variant={variant}
        className={`lyric-text w-full leading-snug font-semibold transition-colors ${
          align === 'center' ? 'text-center' : 'text-left'
        } ${said ? 'text-foreground' : 'text-muted/60'}`}
      >
        {text}
      </button>
    </li>
  );
});

/** Where the captions came from, and transcribing: start, follow, stop, or set it up. */
const CaptionsFooter: FC<{
  captions: Captions;
  spoken: SpokenItem;
  slim: boolean;
}> = ({ captions, spoken, slim }) => {
  // Transcribing starts from where the listener is, read when they ask.
  const readPosition = useReadPosition();
  const { t } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const transcribe = useTranscribe();
  const client = useQueryClient();
  const stop = useStopCaptions();
  const [confirming, setConfirming] = useState(false);
  const [menuAnchor, setMenuAnchor] = useState<HTMLElement | null>(null);
  const book = spoken.kind === 'audiobook';
  const online = captions.engine === 'online';
  const confirmed = settings.data?.transcription.onlineConfirmed ?? false;

  const start = () => {
    if (online && !confirmed) {
      setConfirming(true);
      return;
    }
    transcribe.mutate({ episodeId: spoken.episodeId, fromMs: readPosition() });
  };

  const status = captions.running
    ? captions.preparing
      ? t('captions.preparing')
      : t('captions.listening')
    : slim
      ? t(book ? 'captions.noneChapter' : 'captions.noneEpisode')
      : captions.source
        ? t(`captions.from.${captions.source}`)
        : null;

  const button = 'text-muted hover:text-foreground flex items-center gap-1 text-xs font-medium';
  let action = null;
  if (captions.running) {
    action = (
      <button type="button" onClick={() => stop.mutate(spoken.episodeId)} className={button}>
        <Square aria-hidden size={11} />
        {t('captions.stop')}
      </button>
    );
  } else if (captions.source && captions.hasFeed && captions.canTranscribe) {
    // The captions' source as one dropdown (M46.36, M46.45): the feed's transcript or AI's;
    // each is kept while the other shows, so switching back is instant.
    const feed = captions.source === 'feed';
    const choose = (ai: boolean) => {
      setMenuAnchor(null);
      if (ai === !feed) return;
      if (ai && online && !confirmed) {
        setConfirming(true);
        return;
      }
      void (
        ai
          ? commands.captionsUseAi(spoken.episodeId, readPosition())
          : commands.captionsUseFeed(spoken.episodeId)
      ).then(() => client.invalidateQueries({ queryKey: ['captions', spoken.episodeId] }));
    };
    const option = (ai: boolean) => (
      <button
        type="button"
        role="radio"
        aria-checked={ai === !feed}
        onClick={() => choose(ai)}
        className="hover:bg-background flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm"
      >
        {ai ? (
          <AudioLines aria-hidden size={15} className="text-muted" />
        ) : (
          <FileText aria-hidden size={15} className="text-muted" />
        )}
        <span className="flex-1">{t(ai ? 'captions.sourceAi' : 'captions.sourceFeed')}</span>
        {ai === !feed && <Check aria-hidden size={14} className="text-primary" />}
      </button>
    );
    action = (
      <>
        <button
          type="button"
          aria-haspopup="dialog"
          aria-expanded={menuAnchor !== null}
          onClick={(event) => setMenuAnchor(menuAnchor ? null : event.currentTarget)}
          className={button}
        >
          {t('captions.source')}
          <ChevronDown aria-hidden size={12} />
        </button>
        {menuAnchor && (
          <Sheet
            title={t('captions.source')}
            anchor={menuAnchor}
            popoverWidth={260}
            onClose={() => setMenuAnchor(null)}
          >
            <div
              role="radiogroup"
              aria-label={t('captions.source')}
              className="-mx-2 flex flex-col"
            >
              {option(false)}
              {option(true)}
            </div>
          </Sheet>
        )}
      </>
    );
  } else if (!captions.complete && captions.source !== 'feed') {
    action = captions.canTranscribe ? (
      <button
        type="button"
        disabled={transcribe.isPending}
        onClick={start}
        className={`${button} ${slim ? 'text-primary' : ''}`}
      >
        <AudioLines aria-hidden size={13} />
        {t(slim ? 'captions.transcribe' : 'captions.transcribeRest')}
      </button>
    ) : (
      <Link to="/settings/ai" hash="transcription" className={`${button} text-primary`}>
        <Subtitles aria-hidden size={13} />
        {t('captions.setUp')}
      </Link>
    );
  }

  return (
    <div className={`flex flex-col gap-2 ${slim ? '' : 'mt-6'}`}>
      <div
        className={`flex flex-wrap items-center gap-x-3 gap-y-1 ${slim ? 'justify-between' : ''}`}
      >
        {status && (
          <p role="status" className="text-muted flex items-center gap-1.5 text-xs">
            {captions.running && <LoaderCircle aria-hidden size={12} className="animate-spin" />}
            {status}
          </p>
        )}
        {action}
      </div>
      {confirming && (
        <div className="bg-background/60 flex flex-col gap-2 rounded-lg p-3 text-xs">
          <p>{t('captions.onlineCost')}</p>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => {
                setConfirming(false);
                const prefs = settings.data?.transcription;
                if (prefs) {
                  update.mutate({
                    key: 'transcription',
                    value: { ...prefs, onlineConfirmed: true },
                  });
                }
                // From the feed's transcript, it's kept aside first (M46.45).
                if (captions.source === 'feed') {
                  void commands
                    .captionsUseAi(spoken.episodeId, readPosition())
                    .then(() =>
                      client.invalidateQueries({ queryKey: ['captions', spoken.episodeId] }),
                    );
                } else {
                  transcribe.mutate({ episodeId: spoken.episodeId, fromMs: readPosition() });
                }
              }}
              className="bg-primary text-background rounded-full px-3 py-1 font-semibold"
            >
              {t('captions.transcribe')}
            </button>
            <button
              type="button"
              onClick={() => setConfirming(false)}
              className="rounded-full px-3 py-1"
            >
              {t('common.cancel')}
            </button>
          </div>
        </div>
      )}
      {(captions.error ?? transcribe.error?.message) && (
        <SourceIssue compact>{captions.error ?? transcribe.error?.message}</SourceIssue>
      )}
    </div>
  );
};

/** The playing item's trims (M46.36), so captions show only what plays. */
const useTrims = () => {
  const item = useCurrentItem();
  const edit = useSongEdit(
    item ? trackQueryFor(item) : { title: '', artist: null, album: null, durationMs: null },
  );
  return {
    startMs: item ? (edit.edit.data?.startMs ?? 0) : 0,
    endMs: item ? (edit.edit.data?.endMs ?? null) : null,
  };
};
