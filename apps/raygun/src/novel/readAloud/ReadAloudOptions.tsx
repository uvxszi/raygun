import { commands } from '@raygun/bindings';
import type { AiModel, ReadAloudPrefs, TtsVoice } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AudioLines } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../../ai/useAi';
import { unwrapResult } from '../../lib/commandResult';
import { CancelButton, DownloadBar } from '../../settings/ai/ModelRows';
import { downloadPercent } from '../../settings/ai/modelUse';
import { PrefChoice, PrefLink, PrefSlider, PrefSwitch } from '../../settings/prefs';
import { Problem } from '../../settings/Problem';
import { useSettings, useUpdateSetting } from '../../settings/useSettings';
import { DEFAULT_PREFS } from '../useNovel';
import { primaryLanguage, useChooseVoice, voiceState, voiceStateText } from './voiceChoice';

/** A language's name in the app's language (`de` → German). */
const languageName = (tag: string, locale: string) => {
  try {
    return new Intl.DisplayNames([locale], { type: 'language' }).of(tag) ?? tag;
  } catch {
    return tag;
  }
};

/** The device's own voices, in the voice choice. */
const DEVICE = 'device';
/** The book's own language, in the language choice. */
const AUTO = 'auto';

/** A voice (or the voice engine) on its way: its progress, and Cancel. */
const VoiceDownload: FC<{
  model: AiModel;
  /** It brings the voice just chosen, which reads once it's here. */
  chosen: boolean;
}> = ({ model, chosen }) => {
  const { t } = useTranslation();
  const percent = downloadPercent(model) ?? 0;
  return (
    <li aria-label={t('novel.readAloudOptions.downloadingVoice', { name: model.name })}>
      <div className="flex items-center gap-2 px-1 py-2">
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-medium">
            {t('novel.readAloudOptions.downloadingVoice', { name: model.name })}
          </span>
          <span className="text-muted text-sm">
            {chosen
              ? t('novel.readAloudOptions.readsOnceHere', { percent })
              : t('ai.onDevice.progress', { percent })}
          </span>
        </span>
        <CancelButton model={model} />
      </div>
      <div className="px-1 pb-2">
        <DownloadBar model={model} />
      </div>
    </li>
  );
};

/** The language a book is read aloud in: the one chosen for it, and the one it says it's in. */
const useBookLanguage = (titleId: string) =>
  useQuery({
    queryKey: ['tts', 'language', titleId],
    queryFn: () => commands.ttsLanguage(titleId),
    enabled: Boolean(titleId),
    staleTime: 60_000,
  });

/**
 * Reading aloud's options in the reader (its text settings, and the reading-aloud bar): the voice
 * for the book's language, speed, pitch, volume, the language the book is read in, finding
 * passages in other languages, and Roman numerals. Changes apply to the reading at once (Rust
 * goes on with them from the sentence being read); Settings → AI → Reading aloud downloads voices.
 */
export const ReadAloudOptions: FC<{
  titleId: string;
  /** The language being read now, as reading aloud reports it. */
  readingLanguage?: string | null;
}> = ({ titleId, readingLanguage = null }) => {
  const { t, i18n } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const queryClient = useQueryClient();
  const models = useAiModels().data ?? [];
  const catalogue = useQuery({
    queryKey: ['tts', 'voices'],
    queryFn: () => commands.ttsVoices(),
    staleTime: Infinity,
  }).data;
  const book = useBookLanguage(titleId).data;
  const setLanguage = useMutation({
    mutationFn: (language: string | null) =>
      unwrapResult(commands.ttsSetLanguage(titleId, language)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: ['tts', 'language', titleId] }),
  });
  const prefs = settings.data?.readAloud;
  const reader = settings.data?.novelReader ?? DEFAULT_PREFS;
  const save = (change: Partial<ReadAloudPrefs>) =>
    prefs && update.mutate({ key: 'readAloud', value: { ...prefs, ...change } });
  const name = (tag: string) => languageName(tag, i18n.language);

  // The language read: the one chosen for the book, else its own (or what reading found).
  const chosen = book?.chosen ?? null;
  const auto = book?.book ?? readingLanguage;
  const language = chosen ?? auto;
  const primary = language ? primaryLanguage(language) : null;

  // Voices: the device's, the downloaded ones that speak the language, then the ones that could
  // be downloaded for it (for the app's language while the book's isn't known).
  const runtimeId = catalogue?.runtimeId;
  const choose = useChooseVoice(runtimeId);
  const state = (voice: TtsVoice) => voiceState(voice, models, runtimeId);
  const speaks = (voice: TtsVoice, tag: string | null) =>
    !tag || primaryLanguage(voice.language) === tag;
  const voices = catalogue?.voices ?? [];
  const downloaded = voices.filter(
    (voice) => state(voice).kind === 'installed' && speaks(voice, primary),
  );
  // Only where downloaded voices run (the voice engine is made for this device).
  const offered = runtimeId
    ? voices.filter(
        (voice) =>
          state(voice).kind !== 'installed' &&
          speaks(voice, primary ?? primaryLanguage(i18n.language)),
      )
    : [];
  const engine = prefs?.engine ?? 'system';
  const ofEngine = downloaded.filter((voice) => voice.engine === engine);
  const chosenVoice = primary ? prefs?.voices?.[primary] : undefined;
  const inUse =
    engine === 'system'
      ? undefined
      : (ofEngine.find((voice) => voice.key === chosenVoice) ?? ofEngine[0]);
  const hint = (voice: TtsVoice) =>
    [
      t(`ai.readAloud.engines.${voice.engine}`),
      name(voice.language),
      voiceStateText(state(voice), t),
    ]
      .filter(Boolean)
      .join(' · ');
  const voiceOptions = [
    { value: DEVICE, label: t('novel.readAloudOptions.deviceVoice') },
    ...[...downloaded, ...offered].map((voice) => ({
      value: voice.key,
      label: voice.name,
      hint: hint(voice),
    })),
  ];
  const chooseVoice = (key: string) => {
    if (key === DEVICE) {
      choose.mutate(null);
      return;
    }
    const voice = voices.find((candidate) => candidate.key === key);
    if (voice) choose.mutate(voice);
  };
  // Voices (and the engine) on their way: each reads once it's here.
  const arriving = models.filter(
    (model) => (model.kind === 'tts' || model.id === runtimeId) && downloadPercent(model) !== null,
  );
  const failed = choose.error && choose.variables;

  // Languages: those voices speak, and the book's.
  const tags = new Set(
    [
      ...(catalogue?.voices ?? []).map((voice) => primaryLanguage(voice.language)),
      primaryLanguage(i18n.language),
      ...(auto ? [primaryLanguage(auto)] : []),
      ...(chosen ? [chosen] : []),
    ].filter(Boolean),
  );
  const languageOptions = [
    {
      value: AUTO,
      label: auto
        ? t('novel.readAloudOptions.autoLanguage', { language: name(auto) })
        : t('novel.readAloudOptions.auto'),
      hint: t('novel.readAloudOptions.autoHint'),
    },
    ...[...tags]
      .map((tag) => ({ value: tag, label: name(tag) }))
      .sort((first, second) => first.label.localeCompare(second.label)),
  ];

  const romanTitles = prefs?.romanTitles ?? true;
  const percent = (value: number) => `${value}%`;
  const times = (value: number) => `${(value / 10).toFixed(1)}×`;

  return (
    <ul aria-label={t('novel.readAloudOptions.title')} className="flex flex-col">
      <PrefChoice
        title={t('novel.readAloudOptions.voice')}
        value={inUse?.key ?? DEVICE}
        options={voiceOptions}
        disabled={!prefs}
        summary={
          engine !== 'system' && !inUse && language
            ? t('novel.readAloudOptions.noVoiceFor', { language: name(language) })
            : undefined
        }
        onChange={chooseVoice}
      />
      {arriving.map((model) => (
        <VoiceDownload
          key={model.id}
          model={model}
          chosen={
            choose.isPending &&
            Boolean(choose.variables) &&
            (choose.variables?.modelId === model.id || model.id === runtimeId)
          }
        />
      ))}
      {failed && (
        <li className="px-1 pb-2">
          <Problem
            title={t('ai.downloadFailed', { name: failed.name })}
            detail={choose.error.message}
            onRetry={() => choose.mutate(failed)}
          />
        </li>
      )}
      <PrefSlider
        title={t('novel.speed')}
        value={reader.ttsRate ?? 10}
        min={5}
        max={20}
        step={1}
        format={times}
        onChange={(ttsRate) => update.mutate({ key: 'novelReader', value: { ...reader, ttsRate } })}
      />
      <PrefSlider
        title={t('ai.readAloud.pitch')}
        value={prefs?.pitch ?? 10}
        min={5}
        max={20}
        step={1}
        format={times}
        disabled={!prefs}
        onChange={(pitch) => save({ pitch })}
      />
      <PrefSlider
        title={t('novel.readAloudOptions.volume')}
        value={prefs?.volume ?? 100}
        min={10}
        max={100}
        step={5}
        format={percent}
        disabled={!prefs}
        onChange={(volume) => save({ volume })}
      />
      <PrefChoice
        title={t('novel.readAloudOptions.language')}
        value={chosen ?? AUTO}
        options={languageOptions}
        disabled={!titleId}
        onChange={(value) => setLanguage.mutate(value === AUTO ? null : value)}
      />
      <PrefSwitch
        title={t('novel.readAloudOptions.detectLanguages')}
        summary={t('novel.readAloudOptions.detectLanguagesHint')}
        checked={prefs?.detectLanguages ?? false}
        disabled={!prefs}
        onChange={(detectLanguages) => save({ detectLanguages })}
      />
      <PrefSwitch
        title={t('novel.readAloudOptions.romanTitles')}
        summary={t('novel.readAloudOptions.romanTitlesHint')}
        checked={romanTitles}
        disabled={!prefs}
        onChange={(checked) => save({ romanTitles: checked })}
      />
      <PrefSwitch
        title={t('novel.readAloudOptions.romanEverywhere')}
        summary={t('novel.readAloudOptions.romanEverywhereHint')}
        checked={romanTitles && (prefs?.romanEverywhere ?? false)}
        disabled={!prefs || !romanTitles}
        onChange={(romanEverywhere) => save({ romanEverywhere })}
      />
      <PrefSwitch
        title={t('novel.readAloudOptions.romanOrdinals')}
        summary={t('novel.readAloudOptions.romanOrdinalsHint')}
        checked={romanTitles && (prefs?.romanOrdinals ?? false)}
        disabled={!prefs || !romanTitles}
        onChange={(romanOrdinals) => save({ romanOrdinals })}
      />
      <PrefLink
        icon={AudioLines}
        title={t('novel.readAloudOptions.voices')}
        summary={t('novel.readAloudOptions.voicesHint')}
        to="/settings/ai"
        hash="read-aloud"
      />
    </ul>
  );
};
