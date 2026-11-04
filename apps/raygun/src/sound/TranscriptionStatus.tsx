import { LoaderCircle, Square } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import type { useSongTranscription } from './useSongTranscription';

type Transcription = ReturnType<typeof useSongTranscription>;

/** What hearing a song's words is doing now, as one line with Stop; or why it failed. */
export const TranscriptionStatus: FC<{ transcription: Transcription; compact?: boolean }> = ({
  transcription,
  compact = false,
}) => {
  const { t } = useTranslation();
  const { stage, captions, percent, error } = transcription;
  if (stage === 'idle') {
    return error ? (
      <SourceIssue compact={compact} onRetry={transcription.start}>
        {error}
      </SourceIssue>
    ) : null;
  }
  const text =
    stage === 'queued'
      ? t('lyricsEdit.hearQueued', { n: captions?.queued ?? 1 })
      : stage === 'preparing'
        ? t('lyricsEdit.hearPreparing')
        : percent !== null
          ? t('lyricsEdit.hearingPercent', { percent })
          : t('lyricsEdit.hearing', { n: captions?.cues.length ?? 0 });
  return (
    <div
      role="status"
      data-testid="transcription-status"
      className="text-muted flex items-center gap-2 text-xs"
    >
      <LoaderCircle aria-hidden size={14} className="shrink-0 animate-spin" />
      <span className="min-w-0 flex-1 truncate">{text}</span>
      <button
        type="button"
        onClick={transcription.stop}
        className="hover:text-foreground flex shrink-0 items-center gap-1 rounded-full px-2 py-1"
      >
        <Square aria-hidden size={11} />
        {t('lyricsEdit.hearStop')}
      </button>
    </div>
  );
};
