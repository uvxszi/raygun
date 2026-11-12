import { commands } from '@raygun/bindings';
import type { AiModel, ReadAloudEngine, ReadAloudPrefs, TtsVoice } from '@raygun/bindings';
import { useMutation, useQuery } from '@tanstack/react-query';
import { Download, Trash2, Volume2 } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels, useDeleteAiModel, useDownloadAiModel } from '../../ai/useAi';
import { unwrapResult } from '../../lib/commandResult';
import { formatBytes } from '../../lib/formatBytes';
import {
  primaryLanguage,
  useChooseVoice,
  voiceState,
  voiceStateText,
} from '../../novel/readAloud/voiceChoice';
import type { VoiceState } from '../../novel/readAloud/voiceChoice';
import { DEFAULT_PREFS } from '../../novel/useNovel';
import { Problem } from '../Problem';
import {
  PrefAction,
  PrefBlock,
  PrefChoice,
  PrefGroup,
  PrefInfo,
  PrefSlider,
  PrefSwitch,
} from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';
import { CancelButton, DownloadBar, ModelDownloadRow, ModelRow } from './ModelRows';
import { downloadPercent } from './modelUse';

const ENGINES: ReadAloudEngine[] = ['system', 'piper', 'kokoro'];
const ICON_BUTTON =
  'hover:bg-surface grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-60';

/** A language's name in the app's language. */
const languageName = (tag: string, locale: string) => {
  try {
    return new Intl.DisplayNames([locale], { type: 'language' }).of(tag) ?? tag;
  } catch {
    return tag;
  }
};

/**
 * The voice `language` is read with: the one chosen for it when it's downloaded, else the first
 * downloaded voice that speaks it (as Rust picks), else none.
 */
const voiceInUse = (
  voices: TtsVoice[],
  installed: (voice: TtsVoice) => boolean,
  chosen: string | undefined,
) =>
  voices.find((voice) => voice.key === chosen && installed(voice)) ??
  voices.find((voice) => installed(voice));

/**
 * One voice: chosen for its language (one not downloaded yet is downloaded, then chosen), heard,
 * and (Piper's, one model each) downloaded or removed.
 */
const VoiceRow: FC<{
  voice: TtsVoice;
  model: AiModel | undefined;
  state: VoiceState;
  /** Its model is its own (Piper), so the row downloads and removes it. */
  ownModel: boolean;
  chosen: boolean;
  onChoose: () => void;
}> = ({ voice, model, state, ownModel, chosen, onChoose }) => {
  const { t, i18n } = useTranslation();
  const download = useDownloadAiModel();
  const remove = useDeleteAiModel();
  const preview = useMutation({
    mutationFn: () => unwrapResult(commands.ttsPreview(voice.key, voice.language)),
  });
  const installed = model?.installed ?? false;
  const percent = model ? downloadPercent(model) : null;
  const language = languageName(voice.language, i18n.language);
  const details = [
    language,
    voice.gender ? t(`ai.readAloud.${voice.gender === 'female' ? 'female' : 'male'}`) : null,
    voiceStateText(state, t) ?? (ownModel && model ? formatBytes(model.sizeBytes ?? 0) : null),
    state.kind === 'installed' && chosen
      ? t('ai.readAloud.inUse', {
          language: languageName(primaryLanguage(voice.language), i18n.language),
        })
      : null,
  ].filter(Boolean);

  return (
    <li aria-label={voice.name} className="flex flex-col gap-2 px-1 py-2">
      <div className="flex items-center gap-2">
        <input
          type="radio"
          name={`voice-${primaryLanguage(voice.language)}`}
          aria-label={t('ai.readAloud.useVoice', {
            language: languageName(primaryLanguage(voice.language), i18n.language),
            name: voice.name,
          })}
          checked={chosen}
          disabled={state.kind === 'downloading'}
          onChange={onChoose}
          className="accent-primary size-5 shrink-0"
        />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-medium">{voice.name}</span>
          <span className="text-muted text-sm">{details.join(' · ')}</span>
        </span>
        {installed && (
          <button
            type="button"
            aria-label={t('ai.readAloud.previewVoice', { name: voice.name })}
            title={t('ai.readAloud.previewVoice', { name: voice.name })}
            disabled={preview.isPending}
            onClick={() => preview.mutate()}
            className={ICON_BUTTON}
          >
            <Volume2 aria-hidden size={18} />
          </button>
        )}
        {ownModel &&
          model &&
          (percent !== null ? (
            <CancelButton model={model} />
          ) : installed ? (
            <button
              type="button"
              aria-label={t('ai.deleteModel', { name: model.name })}
              title={t('ai.deleteModel', { name: model.name })}
              disabled={remove.isPending}
              onClick={() => remove.mutate(model.id)}
              className={ICON_BUTTON}
            >
              <Trash2 aria-hidden size={18} />
            </button>
          ) : (
            <button
              type="button"
              aria-label={t('ai.downloadModel', { name: model.name })}
              title={t('ai.downloadModel', { name: model.name })}
              onClick={() => download.mutate(model.id)}
              className={ICON_BUTTON}
            >
              <Download aria-hidden size={18} />
            </button>
          ))}
      </div>
      {ownModel && model && percent !== null && <DownloadBar model={model} />}
      {download.error && model && (
        <Problem
          title={t('ai.downloadFailed', { name: model.name })}
          detail={download.error.message}
          onRetry={() => download.mutate(model.id)}
        />
      )}
      {preview.error && (
        <Problem title={t('ai.readAloud.previewFailed')} detail={preview.error.message} />
      )}
    </li>
  );
};

/**
 * Reading aloud: what reads books aloud (the device's own voices, or Piper's or Kokoro's voices
 * downloaded here with the engine that runs them), the voice each language is read with, the
 * speed, pitch and volume, language changes and Roman numerals, and a preview. The reader's own
 * options set the same settings.
 */
export const ReadAloudSection: FC = () => {
  const { t, i18n } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const models = useAiModels().data ?? [];
  const voices = useQuery({
    queryKey: ['tts', 'voices'],
    queryFn: () => commands.ttsVoices(),
    staleTime: Infinity,
  }).data;
  const prefs = settings.data?.readAloud;
  const reader = settings.data?.novelReader ?? DEFAULT_PREFS;
  const engine = prefs?.engine ?? 'system';
  const save = (change: Partial<ReadAloudPrefs>) =>
    prefs && update.mutate({ key: 'readAloud', value: { ...prefs, ...change } });
  const preview = useMutation({
    mutationFn: () => unwrapResult(commands.ttsPreview(null, i18n.language)),
  });

  const supported = Boolean(voices?.runtimeId);
  const runtime = models.find((model) => model.id === voices?.runtimeId);
  const ofEngine = voices?.voices.filter((voice) => voice.engine === engine) ?? [];
  const modelOf = (voice: TtsVoice) => models.find((model) => model.id === voice.modelId);
  const installed = (voice: TtsVoice) => modelOf(voice)?.installed ?? false;
  const languages = [...new Set(ofEngine.map((voice) => primaryLanguage(voice.language)))].sort(
    (first, second) =>
      languageName(first, i18n.language).localeCompare(languageName(second, i18n.language)),
  );
  const appLanguage = primaryLanguage(i18n.language);
  const [picked, setPicked] = useState<string | null>(null);
  const language =
    (picked && languages.includes(picked) ? picked : null) ??
    (languages.includes(appLanguage) ? appLanguage : languages[0]);
  const shown = ofEngine.filter((voice) => primaryLanguage(voice.language) === language);
  const inUse = language ? voiceInUse(shown, installed, prefs?.voices?.[language]) : undefined;
  // Kokoro's voices are one model; each of Piper's is its own.
  const shared = engine === 'kokoro' ? shown[0] && modelOf(shown[0]) : undefined;
  const choose = useChooseVoice(voices?.runtimeId);
  const failed = choose.error && choose.variables;

  return (
    <PrefGroup id="read-aloud" title={t('ai.readAloud.title')} summary={t('ai.readAloud.about')}>
      <PrefChoice
        title={t('ai.readAloud.engine')}
        value={engine}
        disabled={!prefs}
        options={ENGINES.filter((value) => value === 'system' || supported).map((value) => ({
          value,
          label: t(`ai.readAloud.engines.${value}`),
          hint: t(`ai.readAloud.engineHints.${value}`),
        }))}
        onChange={(value) => save({ engine: value })}
      />
      {!supported && <PrefInfo title={t('ai.readAloud.notHere')} />}
      {engine !== 'system' && runtime && !runtime.installed && (
        <>
          <PrefInfo
            title={t('ai.readAloud.runtime')}
            summary={t('ai.readAloud.runtimeSummary', {
              size: formatBytes(runtime.sizeBytes ?? 0),
            })}
          />
          {runtime.downloading !== null && runtime.downloading !== undefined && (
            <ModelDownloadRow model={runtime} />
          )}
        </>
      )}
      {engine !== 'system' && languages.length > 0 && language && (
        <>
          <PrefChoice
            title={t('ai.readAloud.language')}
            value={language}
            options={languages.map((value) => ({
              value,
              label: languageName(value, i18n.language),
            }))}
            onChange={setPicked}
          />
          {shared && <ModelRow model={shared} />}
          <PrefBlock>
            <ul
              aria-label={t('ai.readAloud.voicesOf', {
                language: languageName(language, i18n.language),
              })}
            >
              {shown.map((voice) => (
                <VoiceRow
                  key={voice.key}
                  voice={voice}
                  model={modelOf(voice)}
                  state={voiceState(voice, models, voices?.runtimeId)}
                  ownModel={engine === 'piper'}
                  chosen={voice.key === inUse?.key}
                  onChoose={() => choose.mutate(voice)}
                />
              ))}
            </ul>
            {failed && (
              <Problem
                title={t('ai.downloadFailed', { name: failed.name })}
                detail={choose.error.message}
                onRetry={() => choose.mutate(failed)}
              />
            )}
          </PrefBlock>
        </>
      )}
      <PrefSlider
        title={t('settings.speechRate')}
        value={reader.ttsRate ?? 10}
        min={5}
        max={20}
        step={1}
        format={(value) => `${(value / 10).toFixed(1)}×`}
        onChange={(ttsRate) => update.mutate({ key: 'novelReader', value: { ...reader, ttsRate } })}
      />
      <PrefSlider
        title={t('ai.readAloud.pitch')}
        value={prefs?.pitch ?? 10}
        min={5}
        max={20}
        step={1}
        format={(value) => `${(value / 10).toFixed(1)}×`}
        disabled={!prefs}
        onChange={(pitch) => save({ pitch })}
      />
      <PrefSlider
        title={t('ai.readAloud.volume')}
        value={prefs?.volume ?? 100}
        min={10}
        max={100}
        step={5}
        format={(value) => `${value}%`}
        disabled={!prefs}
        onChange={(volume) => save({ volume })}
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
        checked={prefs?.romanTitles ?? true}
        disabled={!prefs}
        onChange={(romanTitles) => save({ romanTitles })}
      />
      <PrefSwitch
        title={t('novel.readAloudOptions.romanEverywhere')}
        summary={t('novel.readAloudOptions.romanEverywhereHint')}
        checked={(prefs?.romanTitles ?? true) && (prefs?.romanEverywhere ?? false)}
        disabled={!prefs || !(prefs.romanTitles ?? true)}
        onChange={(romanEverywhere) => save({ romanEverywhere })}
      />
      {engine === 'system' && (
        <PrefAction
          icon={Volume2}
          title={t('ai.readAloud.preview')}
          summary={preview.error ? preview.error.message : t('ai.readAloud.previewSummary')}
          disabled={preview.isPending}
          onClick={() => preview.mutate()}
        />
      )}
    </PrefGroup>
  );
};
