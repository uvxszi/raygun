import { commands } from '@raygun/bindings';
import type { ReadAloudPrefs, TtsVoice } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { AudioLines } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../../ai/useAi';
import { unwrapResult } from '../../lib/commandResult';
import { PrefChoice, PrefLink, PrefSlider, PrefSwitch } from '../../settings/prefs';
import { useSettings, useUpdateSetting } from '../../settings/useSettings';
import { DEFAULT_PREFS } from '../useNovel';

/** A language tag's primary language (`pt-BR` → `pt`; Norwegian's tags as `nb`), as Rust keys it. */
const primaryLanguage = (tag: string) => {
  const primary = (tag.split(/[-_]/)[0] ?? '').toLowerCase();
  return primary === 'no' || primary === 'nn' ? 'nb' : primary;
};

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

  // Voices: the device's, and the downloaded ones that speak the language.
  const installed = (voice: TtsVoice) =>
    models.find((model) => model.id === voice.modelId)?.installed ?? false;
  const downloaded = (catalogue?.voices ?? []).filter(
    (voice) => installed(voice) && (!primary || primaryLanguage(voice.language) === primary),
  );
  const engine = prefs?.engine ?? 'system';
  const ofEngine = downloaded.filter((voice) => voice.engine === engine);
  const chosenVoice = primary ? prefs?.voices?.[primary] : undefined;
  const inUse =
    engine === 'system'
      ? undefined
      : (ofEngine.find((voice) => voice.key === chosenVoice) ?? ofEngine[0]);
  const voiceOptions = [
    { value: DEVICE, label: t('novel.readAloudOptions.deviceVoice') },
    ...downloaded.map((voice) => ({
      value: voice.key,
      label: voice.name,
      hint: [t(`ai.readAloud.engines.${voice.engine}`), name(voice.language)].join(' · '),
    })),
  ];
  const chooseVoice = (key: string) => {
    if (key === DEVICE) {
      save({ engine: 'system' });
      return;
    }
    const voice = downloaded.find((candidate) => candidate.key === key);
    if (!voice) return;
    save({
      engine: voice.engine,
      voices: { ...prefs?.voices, [primaryLanguage(voice.language)]: voice.key },
    });
  };

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
