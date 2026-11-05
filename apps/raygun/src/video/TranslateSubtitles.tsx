import { commands } from '@raygun/bindings';
import type { LyricsTranslatorOption, SubtitleSource, SubtitleTranslation } from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LANGUAGES, nativeName } from '../i18n/languages';
import { unwrapResult } from '../lib/commandResult';
import { formatBytes } from '../lib/formatBytes';
import { PanelHeading, PanelOption } from './PlayerPanel';
import { translationName, translationStatus, translationsKey } from './subtitleProgress';

const DEVICE = 'device';

type Props = {
  videoKey: string;
  /** The main subtitles' name, when they can be translated. */
  sourceName: string | null;
  /** Why the main subtitles can't be translated; `null` when they can. */
  blocked: string | null;
  /** The main subtitles' lines, read when a translation starts. */
  source: () => SubtitleSource | null;
  /** The language picked first: the subtitle language, else the app's. */
  defaultLanguage: string;
  positionMs: number;
  translations: SubtitleTranslation[];
  /** Shows a translation as the main subtitles. */
  onShow: (id: string) => void;
};

type Asking = { option: LyricsTranslatorOption; bytes: number };

/**
 * Translate subtitles in the subtitles panel: the main subtitles (a file, an addon's, ones added
 * here or the AI subtitles) translated into a language ahead of the viewer, on this device or with
 * a translator from Settings → AI. The translation is a track of its own ("Deutsch (translated)"),
 * kept for next time; embedded tracks can't be read as text.
 */
export const TranslateSubtitles: FC<Props> = ({
  videoKey,
  sourceName,
  blocked,
  source,
  defaultLanguage,
  positionMs,
  translations,
  onShow,
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [language, setLanguage] = useState(defaultLanguage);
  const [asking, setAsking] = useState<Asking | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const translators = useQuery({
    queryKey: ['videoSubtitlesTranslators', language],
    queryFn: () => unwrapResult(commands.videoSubtitlesTranslators(language)),
    enabled: blocked === null,
    staleTime: 30_000,
  });
  const languages = LANGUAGES.includes(language) ? LANGUAGES : [language, ...LANGUAGES];

  const start = async (option: LyricsTranslatorOption, download = false) => {
    const lines = source();
    if (!lines) return;
    setBusy(true);
    setError(null);
    setAsking(null);
    const result = await commands.videoSubtitlesTranslate({
      key: videoKey,
      source: lines,
      language,
      translator: option.id,
      fromMs: Math.round(positionMs),
      download,
    });
    setBusy(false);
    if (result.status === 'error') {
      setError(result.error);
      return;
    }
    if (!result.data.running && (result.data.downloadBytes ?? 0) > 0) {
      setAsking({ option, bytes: result.data.downloadBytes ?? 0 });
      return;
    }
    void client.invalidateQueries({ queryKey: translationsKey(videoKey) });
    onShow(result.data.id);
  };

  const hint = (option: LyricsTranslatorOption) => {
    if (option.id === DEVICE) {
      if (!option.supported) return t('desktopPlayer.translate.noModel');
      return (option.downloadBytes ?? 0) > 0
        ? t('desktopPlayer.translate.downloadsFirst')
        : t('desktopPlayer.translate.offline');
    }
    return option.supported ? null : t('desktopPlayer.translate.needsKey');
  };

  return (
    <>
      <PanelHeading>{t('desktopPlayer.translate.title')}</PanelHeading>
      {/* Translations being made, or that stopped on a problem; all of them are tracks above. */}
      {translations
        .filter((translation) => translation.running || translation.error)
        .map((translation) => (
          <div
            key={translation.id}
            className="flex items-center gap-2 px-3 py-1.5 text-xs text-white/70"
          >
            <span role="status" className="min-w-0 flex-1">
              {translationName(translation, t)} · {translationStatus(translation, t, positionMs)}
            </span>
            {translation.running && (
              <button
                type="button"
                onClick={() => void commands.videoSubtitlesStop(videoKey, translation.id)}
                className="rounded-lg bg-white/10 px-3 py-1.5 hover:bg-white/20"
              >
                {t('desktopPlayer.translate.stop')}
              </button>
            )}
          </div>
        ))}
      {blocked ? (
        <p className="px-3 py-1 text-xs text-white/60">{blocked}</p>
      ) : (
        <>
          <p className="px-3 py-1 text-xs text-white/60">
            {t('desktopPlayer.translate.from', { name: sourceName ?? '' })}
          </p>
          <label className="flex items-center gap-3 px-3 py-1.5 text-sm">
            <span className="flex-1">{t('desktopPlayer.translate.into')}</span>
            <select
              value={language}
              onChange={(event) => setLanguage(event.target.value)}
              className="max-w-40 rounded-lg bg-white/10 px-2 py-1 outline-none"
            >
              {languages.map((tag) => (
                <option key={tag} value={tag} className="bg-neutral-900">
                  {nativeName(tag)}
                </option>
              ))}
            </select>
          </label>
          {(translators.data ?? []).map((option) => (
            <PanelOption
              key={option.id}
              label={
                option.id === DEVICE
                  ? t('desktopPlayer.translate.device')
                  : t('desktopPlayer.translate.with', { name: option.name })
              }
              hint={hint(option)}
              disabled={!option.supported || busy}
              onSelect={() => void start(option)}
            />
          ))}
        </>
      )}
      {asking && (
        <div className="mx-3 my-1 flex flex-col gap-2 rounded-lg bg-white/5 p-3 text-xs">
          <p>{t('desktopPlayer.translate.downloadAsk', { size: formatBytes(asking.bytes) })}</p>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => void start(asking.option, true)}
              className="bg-primary text-background rounded-full px-3 py-1.5 font-semibold"
            >
              {t('desktopPlayer.translate.downloadStart')}
            </button>
            <button
              type="button"
              onClick={() => setAsking(null)}
              className="rounded-full px-3 py-1.5 hover:bg-white/10"
            >
              {t('videoPlayer.cancel')}
            </button>
          </div>
        </div>
      )}
      {error && (
        <p role="alert" className="px-3 pb-1 text-xs text-white/70">
          {error}
        </p>
      )}
    </>
  );
};
