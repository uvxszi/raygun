import { commands } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { LoaderCircle, Sparkles } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels, useCancelAiModel, useDownloadAiModel } from '../ai/useAi';
import { formatBytes } from '../lib/formatBytes';
import { formatTime } from '../player/formatTime';
import { useCaptions, useStopCaptions } from '../player/spoken';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { PanelHeading, PanelOption } from './PlayerPanel';
import { coveredAhead } from './subtitleProgress';

/** The speech model set up first when none is (the catalogue's smallest). */
const FIRST_MODEL = 'whisper-tiny';

type Props = {
  videoKey: string;
  /** What the player plays (the proxy's link): the sound is read from there. */
  url: string;
  format: string | null;
  positionMs: number;
  onStarted: () => void;
  onClose: () => void;
};

const BUTTON =
  'bg-primary text-background flex items-center gap-2 self-start rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-50';

/**
 * Transcribe with AI in the subtitles panel: subtitles made for the video from where the viewer
 * is, staying ahead of them (Rust follows the player's reports); lines show as each part is
 * heard. Without a speech model or an online provider, the model downloads here, or Settings →
 * AI → Transcription is a tap away.
 */
export const AiSubtitles: FC<Props> = ({
  videoKey,
  url,
  format,
  positionMs,
  onStarted,
  onClose,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const settings = useSettings();
  const update = useUpdateSetting();
  const models = useAiModels();
  const download = useDownloadAiModel();
  const cancel = useCancelAiModel();
  const captions = useCaptions(videoKey);
  const stop = useStopCaptions();
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const prefs = settings.data?.transcription;
  const speech = (models.data ?? []).filter((model) => model.kind === 'speech');
  const installed = speech.filter((model) => model.installed);
  const online = prefs?.engine === 'online' && Boolean(prefs.providerId);
  const data = captions.data;
  const running = data?.running ?? false;
  const wanted =
    speech.find((model) => model.id === (prefs?.localModel || FIRST_MODEL)) ??
    speech.find((model) => model.id === FIRST_MODEL);

  const start = async (confirmed = false) => {
    if (!prefs) return;
    if (online && !prefs.onlineConfirmed && !confirmed) {
      setConfirming(true);
      return;
    }
    setConfirming(false);
    setError(null);
    try {
      if (confirmed) {
        await update.mutateAsync({
          key: 'transcription',
          value: { ...prefs, onlineConfirmed: true },
        });
      } else if (!online && prefs.engine !== 'local' && installed[0]) {
        // A model is here but transcription is off: it's on with that model from now on.
        await update.mutateAsync({
          key: 'transcription',
          value: { ...prefs, engine: 'local', localModel: installed[0].id },
        });
      }
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
      return;
    }
    const result = await commands.captionsVideoTranscribe(
      videoKey,
      url,
      format,
      Math.round(positionMs),
    );
    if (result.status === 'error') setError(result.error);
    else onStarted();
  };

  const pick = (model: string) => {
    if (!prefs) return;
    update.mutate({
      key: 'transcription',
      value: { ...prefs, engine: 'local', localModel: model },
    });
  };

  const downloadModel = async () => {
    if (!wanted || !prefs) return;
    setError(null);
    try {
      // Cancelled: nothing changes, and the download is offered again.
      if ((await download.mutateAsync(wanted.id)) === 'cancelled') return;
      await update.mutateAsync({
        key: 'transcription',
        value: { ...prefs, engine: 'local', localModel: wanted.id },
      });
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    }
  };

  const ahead = data ? coveredAhead(data.covered, positionMs) : 0;
  const status = data?.preparing
    ? t('desktopPlayer.ai.preparing')
    : running
      ? t('desktopPlayer.ai.ahead', { time: formatTime(ahead) })
      : data?.complete
        ? t('desktopPlayer.ai.complete')
        : null;
  const problem = error ?? data?.error ?? null;
  const downloaded = wanted?.downloading ?? null;
  const percent =
    downloaded !== null && wanted?.sizeBytes
      ? Math.min(99, Math.floor((downloaded / wanted.sizeBytes) * 100))
      : 0;

  return (
    <>
      <PanelHeading>{t('desktopPlayer.ai.title')}</PanelHeading>
      {installed.length === 0 && !online ? (
        <div className="flex flex-col gap-2 px-3 py-1 text-xs text-white/70">
          <p>{t('desktopPlayer.ai.needsModel')}</p>
          {wanted &&
            (downloaded !== null ? (
              <div className="flex items-center gap-2">
                <p role="status" className="flex flex-1 items-center gap-2">
                  <LoaderCircle aria-hidden size={14} className="animate-spin" />
                  {t('desktopPlayer.ai.downloading', { percent })}
                </p>
                <button
                  type="button"
                  aria-label={t('ai.cancelDownload', { name: wanted.name })}
                  disabled={cancel.isPending}
                  onClick={() => cancel.mutate(wanted.id)}
                  className="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/20 disabled:opacity-50"
                >
                  {t('desktopPlayer.ai.cancelDownload')}
                </button>
              </div>
            ) : (
              <button type="button" onClick={() => void downloadModel()} className={BUTTON}>
                {t('desktopPlayer.ai.download', { size: formatBytes(wanted.sizeBytes ?? 0) })}
              </button>
            ))}
          <button
            type="button"
            onClick={() => {
              onClose();
              void navigate({ to: '/settings/ai', hash: 'transcription' });
            }}
            className="text-primary self-start font-semibold"
          >
            {t('desktopPlayer.ai.setUp')}
          </button>
        </div>
      ) : (
        <>
          {installed.map((model) => (
            <PanelOption
              key={model.id}
              label={model.name}
              selected={prefs?.engine === 'local' && prefs.localModel === model.id}
              onSelect={() => pick(model.id)}
            />
          ))}
          {online && prefs && (
            <PanelOption
              label={t('desktopPlayer.ai.online')}
              hint={prefs.onlineModel}
              selected
              onSelect={() => undefined}
            />
          )}
          <div className="flex items-center gap-2 px-3 py-1.5">
            {running ? (
              <>
                <LoaderCircle aria-hidden size={14} className="animate-spin text-white/70" />
                <span role="status" className="flex-1 text-xs text-white/70 tabular-nums">
                  {status}
                </span>
                <button
                  type="button"
                  onClick={() => stop.mutate(videoKey)}
                  className="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/20"
                >
                  {t('captions.stop')}
                </button>
              </>
            ) : (
              <button type="button" disabled={!url} onClick={() => void start()} className={BUTTON}>
                <Sparkles aria-hidden size={13} />
                {(data?.cues.length ?? 0) > 0
                  ? t('desktopPlayer.ai.continue')
                  : t('desktopPlayer.ai.make')}
              </button>
            )}
          </div>
          {!running && status && (
            <p role="status" className="px-3 pb-1 text-xs text-white/60">
              {status}
            </p>
          )}
          {confirming && (
            <div className="mx-3 my-1 flex flex-col gap-2 rounded-lg bg-white/5 p-3 text-xs">
              <p>{t('desktopPlayer.ai.onlineCost')}</p>
              <div className="flex gap-2">
                <button type="button" onClick={() => void start(true)} className={BUTTON}>
                  {t('desktopPlayer.ai.make')}
                </button>
                <button
                  type="button"
                  onClick={() => setConfirming(false)}
                  className="rounded-full px-3 py-1.5 hover:bg-white/10"
                >
                  {t('videoPlayer.cancel')}
                </button>
              </div>
            </div>
          )}
        </>
      )}
      {problem && (
        <p role="alert" className="px-3 pb-1 text-xs text-white/70">
          {problem}
        </p>
      )}
    </>
  );
};
