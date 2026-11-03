import type { Captions, PlaybackState, SpokenItem, TextAlign } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { commands } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { AudioLines, FileText, LoaderCircle, Square, Subtitles } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { SourceIssue } from '../shell/SourceIssue';
import { currentCue, useCaptions, useStopCaptions, useTranscribe } from './spoken';
import { useSongEdit } from '../sound/useSound';
import { trackQueryFor } from './lyricsQuery';
import { useLivePosition } from './useLivePosition';
import { sendControl, useNowPlaying } from './usePlayback';

type CaptionsViewProps = {
  spoken: SpokenItem;
  state: PlaybackState | undefined;
  /** `panel`: the smaller card in the side panel and under the phone's player. */
  variant?: 'sheet' | 'panel';
  /** Text size in px; 0 picks it by screen. */
  size?: number;
  align?: TextAlign;
};

const LINE_SIZE = {
  sheet: 'text-xl md:text-2xl lg:text-3xl',
  panel: 'text-lg',
} as const;

/**
 * Captions for a podcast episode or a book's chapter (M46.32), in place of lyrics: the feed's
 * transcript or what AI heard, following playback; a tap on a line goes there. With none yet the
 * card is one slim line with a way to transcribe, or to set transcription up.
 */
export const CaptionsView: FC<CaptionsViewProps> = ({
  spoken,
  state,
  variant = 'sheet',
  size = 0,
  align = 'left',
}) => {
  const { t } = useTranslation();
  const captions = useCaptions(spoken.episodeId);
  const positionMs = useLivePosition(state);
  const containerRef = useRef<HTMLDivElement>(null);
  const currentRef = useRef<HTMLButtonElement>(null);
  const trims = useTrims();
  // Only what plays shows (M46.36): lines the item's trims cut off are left out.
  const cues = (captions.data?.cues ?? []).filter(
    (cue) => cue.endMs > trims.startMs && (trims.endMs === null || cue.startMs < trims.endMs),
  );
  const current = currentCue(cues, positionMs);
  const hasLines = cues.length > 0;
  const alignClass = align === 'center' ? 'text-center' : 'text-left';
  const height = variant === 'panel' ? (hasLines ? 'h-[26rem]' : 'h-auto') : '';

  useEffect(() => {
    const container = containerRef.current;
    const line = currentRef.current;
    if (!container || !line) return;
    container.scrollTo?.({
      top: line.offsetTop - container.clientHeight / 2 + line.clientHeight / 2,
      behavior: 'smooth',
    });
  }, [current]);

  return (
    <div
      ref={containerRef}
      data-testid="captions-view"
      className={`relative flex min-h-0 flex-col overflow-y-auto ${hasLines || variant === 'sheet' ? 'py-4' : 'py-3'} ${
        variant === 'sheet' ? 'w-full flex-1 pr-4' : 'w-full px-4'
      } ${height}`}
    >
      {captions.isPending && (
        <p className="text-muted text-center text-sm">{t('captions.loading')}</p>
      )}
      {captions.error && <SourceIssue compact>{captions.error.message}</SourceIssue>}
      {hasLines && (
        <ol className="flex flex-col gap-3">
          {cues.map((cue, index) => (
            <li key={cue.startMs}>
              <button
                type="button"
                ref={index === current ? currentRef : undefined}
                aria-current={index === current ? 'true' : undefined}
                onClick={() => void sendControl({ type: 'seek', positionMs: cue.startMs })}
                style={size > 0 ? { fontSize: size } : undefined}
                className={`w-full leading-snug font-semibold transition-colors ${alignClass} ${size > 0 ? '' : LINE_SIZE[variant]} ${
                  index === current ? 'text-foreground' : 'text-muted/60'
                }`}
              >
                {cue.text}
              </button>
            </li>
          ))}
        </ol>
      )}
      {captions.data && (
        <CaptionsFooter
          captions={captions.data}
          spoken={spoken}
          positionMs={positionMs}
          slim={!hasLines}
        />
      )}
    </div>
  );
};

/** Where the captions came from, and transcribing: start, follow, stop, or set it up. */
const CaptionsFooter: FC<{
  captions: Captions;
  spoken: SpokenItem;
  positionMs: number;
  slim: boolean;
}> = ({ captions, spoken, positionMs, slim }) => {
  const { t } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const transcribe = useTranscribe();
  const client = useQueryClient();
  const stop = useStopCaptions();
  const [confirming, setConfirming] = useState(false);
  const book = spoken.kind === 'audiobook';
  const online = captions.engine === 'online';
  const confirmed = settings.data?.transcription.onlineConfirmed ?? false;

  const start = () => {
    if (online && !confirmed) {
      setConfirming(true);
      return;
    }
    transcribe.mutate({ episodeId: spoken.episodeId, fromMs: positionMs });
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
  } else if (captions.source === 'feed' && captions.canTranscribe) {
    // The listener picks the captions' source (M46.36): AI in place of the feed's transcript.
    action = (
      <button
        type="button"
        disabled={transcribe.isPending}
        onClick={() => void commands.captionsClear(spoken.episodeId).then(() => start())}
        className={button}
      >
        <AudioLines aria-hidden size={13} />
        {t('captions.useAi')}
      </button>
    );
  } else if (captions.source && captions.source !== 'feed' && captions.hasFeed) {
    action = (
      <button
        type="button"
        onClick={() =>
          void commands
            .captionsUseFeed(spoken.episodeId)
            .then(() => client.invalidateQueries({ queryKey: ['captions', spoken.episodeId] }))
        }
        className={button}
      >
        <FileText aria-hidden size={13} />
        {t('captions.useFeed')}
      </button>
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
                transcribe.mutate({ episodeId: spoken.episodeId, fromMs: positionMs });
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
  const { item } = useNowPlaying();
  const edit = useSongEdit(
    item ? trackQueryFor(item) : { title: '', artist: null, album: null, durationMs: null },
  );
  return {
    startMs: item ? (edit.edit.data?.startMs ?? 0) : 0,
    endMs: item ? (edit.edit.data?.endMs ?? null) : null,
  };
};
