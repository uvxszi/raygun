import { commands, events } from '@raygun/bindings';
import type {
  AiModel,
  Lyrics,
  LyricsTranslation,
  LyricsTranslatorOption,
  TrackQuery,
} from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Check, Cpu, Languages, LoaderCircle, RotateCcw, Sparkles } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

import { useAiModels } from '../ai/useAi';
import { LANGUAGES, nativeName } from '../i18n/languages';
import { formatBytes } from '../lib/formatBytes';
import { unwrapResult } from '../lib/commandResult';
import { PrefSwitch } from '../settings/prefs';
import { useUpdateSetting } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { SourceIssue } from '../shell/SourceIssue';
import {
  choiceKey,
  sameLanguage,
  translationsOf,
  useTranslationChoices,
  useTranslationPrefs,
} from './lyricsTranslation';
import type { ShownTranslation, TranslationPrefs } from './lyricsTranslation';

const DEVICE = 'device';

/** A translator's job: translating with it, after downloading its models when it's this device. */
type Job = { option: LyricsTranslatorOption; language: string };

const lyricsKey = (query: TrackQuery) => [
  'lyrics',
  query.title,
  query.artist,
  query.album,
  query.durationMs,
];

/** Who made a kept translation, as a row says it. */
const madeBy = (translation: LyricsTranslation, t: TFunction) => {
  switch (translation.origin) {
    case 'device':
      return t('lyricsTranslate.madeOnDevice');
    case 'ai':
      return t('lyricsTranslate.madeBy', { name: translation.by });
    default:
      return t('lyricsTranslate.from', { source: translation.by });
  }
};

/**
 * The lyrics' Translate button (M46.48): a small icon beside where the lyrics come from. Its menu
 * switches translations on and off, picks the language, shows the translations the song has (its
 * source's, another source's, ones made before) and makes new ones on this device or with a
 * translator from Settings → AI. Making one keeps going when the menu closes; its progress and
 * any issue show under the lyrics.
 */
export const LyricsTranslateMenu: FC<{
  query: TrackQuery;
  lyrics: Lyrics;
  shown: ShownTranslation;
}> = ({ query, lyrics, shown }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const { prefs, language } = useTranslationPrefs();
  const update = useUpdateSetting();
  const choose = useTranslationChoices((state) => state.choose);
  const key = choiceKey(query);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  // While models download: how much of everything to download is in, in percent.
  const [downloadPercent, setDownloadPercent] = useState<number | null>(null);
  const download = useRef({ base: 0, total: 0, models: [] as string[] });

  const savePrefs = (change: Partial<TranslationPrefs>) =>
    update.mutate({ key: 'lyricsTranslation', value: { ...prefs, ...change } });

  const translate = useMutation({
    mutationFn: async ({ option, language: target }: Job) => {
      if (option.id === DEVICE && (option.downloadBytes ?? 0) > 0) {
        // The models this song needs, one after another; the catalogue says which are missing.
        const models = await commands.aiModels();
        const missing = option.models.filter(
          (id) => !models.find((model: AiModel) => model.id === id)?.installed,
        );
        download.current = { base: 0, total: option.downloadBytes ?? 0, models: missing };
        setDownloadPercent(0);
        for (const id of missing) {
          await unwrapResult(commands.aiModelDownload(id));
          download.current.base += models.find((model: AiModel) => model.id === id)?.sizeBytes ?? 0;
        }
        setDownloadPercent(null);
      }
      return unwrapResult(commands.lyricsTranslate(query, target, option.id));
    },
    onSuccess: (translation) => {
      choose(key, { translator: translation.translator ?? '', language: translation.language });
      void client.invalidateQueries({ queryKey: lyricsKey(query) });
      void client.invalidateQueries({ queryKey: ['lyricsCache'] });
    },
    onSettled: () => {
      setProgress(null);
      setDownloadPercent(null);
    },
  });
  const running = translate.isPending;
  const downloading = downloadPercent !== null;

  // Lines done while this device translates the song.
  useEffect(() => {
    if (!running) return;
    const unlisten = events.lyricsTranslationEvent.listen(({ payload }) => {
      if (payload.key === key) setProgress({ done: payload.done, total: payload.total });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [running, key]);

  // The models' download, a percent at a time (the events come every 256 KB).
  useEffect(() => {
    if (!downloading) return;
    const unlisten = events.aiModelEvent.listen(({ payload }) => {
      const { base, total, models } = download.current;
      if (payload.state !== 'downloading' || !models.includes(payload.id) || total <= 0) return;
      setDownloadPercent(
        Math.min(99, Math.floor(((base + (payload.receivedBytes ?? 0)) / total) * 100)),
      );
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [downloading]);

  const pick = (translation: LyricsTranslation) => {
    const translator = translation.translator ?? '';
    choose(key, { translator, language: translation.language });
    void commands
      .lyricsTranslationPick(query, translation.language, translator)
      .then(() => client.invalidateQueries({ queryKey: lyricsKey(query) }));
    setAnchor(null);
  };

  const start = (job: Job) => {
    translate.mutate(job);
    setAnchor(null);
  };

  const failedWith = translate.variables?.option;
  const issueSource = failedWith
    ? failedWith.id === DEVICE
      ? t('lyricsTranslate.thisDevice')
      : failedWith.name
    : null;

  return (
    <>
      <button
        type="button"
        aria-label={t('lyricsTranslate.open')}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={`hover:text-foreground flex items-center gap-1 text-xs ${shown.translation ? 'text-primary' : 'text-muted'}`}
      >
        {running ? (
          <LoaderCircle aria-hidden size={14} className="animate-spin" />
        ) : (
          <Languages aria-hidden size={14} />
        )}
      </button>
      {anchor && (
        <Sheet
          title={t('lyricsTranslate.title')}
          anchor={anchor}
          popoverWidth={320}
          onClose={() => setAnchor(null)}
        >
          <TranslateOptions
            query={query}
            lyrics={lyrics}
            shown={shown}
            prefs={prefs}
            language={language}
            running={running}
            onPrefs={savePrefs}
            onPick={pick}
            onStart={start}
            onHide={() => {
              choose(key, null);
              savePrefs({ automatic: false });
            }}
          />
        </Sheet>
      )}
      {running && (
        <span
          role="status"
          aria-label={
            downloadPercent !== null
              ? t('lyricsTranslate.downloading', { percent: downloadPercent })
              : progress
                ? t('lyricsTranslate.translatingLines', progress)
                : t('lyricsTranslate.translating')
          }
          className="text-muted text-xs tabular-nums"
        >
          {downloadPercent !== null
            ? `${downloadPercent}%`
            : progress
              ? `${progress.done}/${progress.total}`
              : null}
        </span>
      )}
      {translate.error && translate.variables && (
        <SourceIssue
          compact
          source={issueSource}
          onRetry={() => translate.variables && start(translate.variables)}
        >
          {translate.error.message}
        </SourceIssue>
      )}
    </>
  );
};

type OptionsProps = {
  query: TrackQuery;
  lyrics: Lyrics;
  shown: ShownTranslation;
  prefs: TranslationPrefs;
  language: string;
  running: boolean;
  onPrefs: (change: Partial<TranslationPrefs>) => void;
  onPick: (translation: LyricsTranslation) => void;
  onStart: (job: Job) => void;
  onHide: () => void;
};

const ROW =
  'hover:bg-background flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm disabled:opacity-50';

/** The menu's rows: switches, the language, the song's translations and the translators. */
const TranslateOptions: FC<OptionsProps> = ({
  query,
  lyrics,
  shown,
  prefs,
  language,
  running,
  onPrefs,
  onPick,
  onStart,
  onHide,
}) => {
  const { t, i18n } = useTranslation();
  const [confirm, setConfirm] = useState<LyricsTranslatorOption | null>(null);
  // Keeps the models' download progress current while the menu is open.
  useAiModels();
  const options = useQuery({
    queryKey: [
      'lyricsTranslateOptions',
      query.title,
      query.artist,
      query.album,
      query.durationMs,
      language,
    ],
    queryFn: () => unwrapResult(commands.lyricsTranslateOptions(query, language)),
    staleTime: 30_000,
  });
  const all = translationsOf(lyrics);
  const kept = all
    .filter((translation) => translation.kind === 'translation')
    // The ones in the language first.
    .sort(
      (a, b) =>
        Number(sameLanguage(b.language, language)) - Number(sameLanguage(a.language, language)),
    );
  const hasRomanization = all.some((translation) => translation.kind === 'romanization');
  const made = new Set(
    kept
      .filter((translation) => sameLanguage(translation.language, language))
      .map((translation) => translation.translator),
  );
  const showing = shown.translation;
  const again =
    showing && (showing.origin === 'device' || showing.origin === 'ai')
      ? options.data?.find((option) => option.id === showing.translator)
      : undefined;
  const languageName = (tag: string) => nativeName(tag);

  if (confirm) {
    return (
      <div className="flex flex-col gap-3 px-1 pb-1 text-sm">
        <p>{t('lyricsTranslate.downloadAsk', { size: formatBytes(confirm.downloadBytes ?? 0) })}</p>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => onStart({ option: confirm, language })}
            className="bg-primary text-background rounded-full px-4 py-2 font-semibold"
          >
            {t('lyricsTranslate.downloadStart')}
          </button>
          <button
            type="button"
            onClick={() => setConfirm(null)}
            className="bg-background rounded-full px-4 py-2"
          >
            {t('lyricsTranslate.cancel')}
          </button>
        </div>
      </div>
    );
  }

  const translator = (option: LyricsTranslatorOption) => {
    const device = option.id === DEVICE;
    const label = device
      ? t('lyricsTranslate.device')
      : t('lyricsTranslate.with', { name: option.name });
    const hint = !option.supported
      ? device
        ? t('lyricsTranslate.notForLanguage')
        : t('lyricsTranslate.needsKey')
      : device && (option.downloadBytes ?? 0) > 0
        ? t('lyricsTranslate.download', { size: formatBytes(option.downloadBytes ?? 0) })
        : null;
    const Icon = device ? Cpu : Sparkles;
    return (
      <button
        key={option.id}
        type="button"
        disabled={!option.supported || running}
        onClick={() =>
          device && (option.downloadBytes ?? 0) > 0
            ? setConfirm(option)
            : onStart({ option, language })
        }
        className={ROW}
      >
        <Icon aria-hidden size={15} className="text-muted shrink-0" />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate">{label}</span>
          {hint && <span className="text-muted truncate text-xs">{hint}</span>}
        </span>
      </button>
    );
  };

  return (
    <div data-testid="lyrics-translate-menu" className="-mx-2 flex flex-col gap-1">
      <ul className="flex flex-col px-2">
        <PrefSwitch
          title={t('lyricsTranslate.show')}
          checked={prefs.automatic || showing !== null}
          onChange={(on) => (on ? onPrefs({ automatic: true }) : onHide())}
        />
        {hasRomanization && (
          <PrefSwitch
            title={t('lyricsTranslate.romanized')}
            checked={prefs.romanized}
            onChange={(romanized) => onPrefs({ romanized })}
          />
        )}
      </ul>
      <label className="flex items-center gap-3 px-3 py-2 text-sm">
        <span className="flex-1">{t('lyricsTranslate.into')}</span>
        <select
          value={prefs.language}
          onChange={(event) => onPrefs({ language: event.target.value })}
          className="bg-background max-w-40 rounded-lg px-2 py-1.5 outline-none"
        >
          <option value="">
            {t('lyricsTranslate.appLanguage', { language: languageName(i18n.language) })}
          </option>
          {LANGUAGES.map((tag) => (
            <option key={tag} value={tag}>
              {languageName(tag)}
            </option>
          ))}
        </select>
      </label>
      {kept.length > 0 && (
        <div
          role="radiogroup"
          aria-label={t('lyricsTranslate.translations')}
          className="flex flex-col"
        >
          {kept.map((translation) => {
            const on =
              showing !== null &&
              showing.translator === translation.translator &&
              showing.language === translation.language;
            return (
              <button
                key={`${translation.translator}-${translation.language}`}
                type="button"
                role="radio"
                aria-checked={on}
                onClick={() => onPick(translation)}
                className={ROW}
              >
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate">{madeBy(translation, t)}</span>
                  <span className="text-muted truncate text-xs">
                    {languageName(translation.language)}
                  </span>
                </span>
                {on && <Check aria-hidden size={14} className="text-primary shrink-0" />}
              </button>
            );
          })}
        </div>
      )}
      <h3 className="text-muted px-3 pt-2 text-xs font-semibold">{t('lyricsTranslate.make')}</h3>
      {options.isPending && (
        <p className="text-muted flex items-center gap-2 px-3 py-2 text-sm">
          <LoaderCircle aria-hidden size={14} className="animate-spin" />
          {t('lyricsTranslate.loading')}
        </p>
      )}
      {options.error && <SourceIssue compact>{options.error.message}</SourceIssue>}
      {options.data?.filter((option) => !made.has(option.id)).map((option) => translator(option))}
      {again && (
        <button
          type="button"
          disabled={running}
          onClick={() => onStart({ option: again, language: showing?.language ?? language })}
          className={ROW}
        >
          <RotateCcw aria-hidden size={15} className="text-muted shrink-0" />
          <span className="flex-1">{t('lyricsTranslate.again')}</span>
        </button>
      )}
    </div>
  );
};
