import { commands } from '@raygun/bindings';
import type { AiModel, ReadAloudPrefs, Settings, TtsVoice } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import type { TFunction } from 'i18next';

import { unwrapResult } from '../../lib/commandResult';
import { formatBytes } from '../../lib/formatBytes';
import { downloadPercent } from '../../settings/ai/modelUse';
import { SETTINGS_KEY } from '../../settings/useSettings';

/** The query `useAiModels` keeps the model catalogue under. */
const MODELS_KEY = ['ai', 'models'];

/** A language tag's primary language (`pt-BR` → `pt`; Norwegian's tags as `nb`), as Rust keys it. */
export const primaryLanguage = (tag: string) => {
  const primary = (tag.split(/[-_]/)[0] ?? '').toLowerCase();
  return primary === 'no' || primary === 'nn' ? 'nb' : primary;
};

/**
 * Where a voice stands on this device: downloaded, on its way (percent), or what it takes: its
 * model (with the voice engine when that isn't here), or only the engine.
 */
export type VoiceState =
  | { kind: 'installed' }
  | { kind: 'downloading'; percent: number }
  | { kind: 'missing'; bytes: number }
  | { kind: 'needsEngine'; bytes: number };

/** What a voice's state says beside its name, when it isn't simply here. */
export const voiceStateText = (state: VoiceState, t: TFunction): string | null => {
  switch (state.kind) {
    case 'downloading':
      return t('ai.onDevice.progress', { percent: state.percent });
    case 'missing':
      return t('novel.readAloudOptions.notDownloaded', { size: formatBytes(state.bytes) });
    case 'needsEngine':
      return t('novel.readAloudOptions.needsEngine', { size: formatBytes(state.bytes) });
    default:
      return null;
  }
};

/**
 * `voice`'s state among `models`: its model's, with the voice engine (`runtimeId`) counted in
 * while that isn't here (it comes with the first voice).
 */
export const voiceState = (
  voice: TtsVoice,
  models: AiModel[],
  runtimeId: string | null | undefined,
): VoiceState => {
  const model = models.find((candidate) => candidate.id === voice.modelId);
  const runtime = models.find((candidate) => candidate.id === runtimeId);
  const percent = model ? downloadPercent(model) : null;
  if (percent !== null) return { kind: 'downloading', percent };
  const runtimePercent = runtime ? downloadPercent(runtime) : null;
  if (model?.installed && runtimePercent !== null)
    return { kind: 'downloading', percent: runtimePercent };
  if (model?.installed && (runtime?.installed ?? true)) return { kind: 'installed' };
  const engine = runtime && !runtime.installed ? (runtime.sizeBytes ?? 0) : 0;
  if (model?.installed) return { kind: 'needsEngine', bytes: engine };
  return { kind: 'missing', bytes: (model?.sizeBytes ?? 0) + engine };
};

/** `prefs` with `voice` reading its language, as Rust saves them (none: the device's voices). */
const withVoice = (prefs: ReadAloudPrefs, voice: TtsVoice | null): ReadAloudPrefs =>
  voice
    ? {
        ...prefs,
        engine: voice.engine,
        voices: { ...prefs.voices, [primaryLanguage(voice.language)]: voice.key },
      }
    : { ...prefs, engine: 'system' };

/**
 * Chooses the voice books are read with (`null`: the device's own). A downloaded one shows as
 * chosen at once; one that isn't is downloaded by Rust (its progress shows on its model) and
 * chosen once it's here, unless another voice is chosen meanwhile. Resolves to how it ended.
 */
export const useChooseVoice = (runtimeId: string | null | undefined) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (voice: TtsVoice | null) =>
      unwrapResult(commands.ttsUseVoice(voice ? voice.key : null)),
    onMutate: (voice) => {
      const models = queryClient.getQueryData<AiModel[]>(MODELS_KEY) ?? [];
      const state = voice ? voiceState(voice, models, runtimeId) : { kind: 'installed' };
      if (state.kind === 'installed') {
        queryClient.setQueryData<Settings>(SETTINGS_KEY, (settings) =>
          settings ? { ...settings, readAloud: withVoice(settings.readAloud, voice) } : settings,
        );
      } else if (voice && state.kind !== 'downloading') {
        // What's fetched shows its progress at once: the voice, or the engine it needs.
        const fetched = state.kind === 'needsEngine' ? runtimeId : voice.modelId;
        queryClient.setQueryData<AiModel[]>(MODELS_KEY, (list) =>
          list?.map((candidate) =>
            candidate.id === fetched ? { ...candidate, downloading: 0 } : candidate,
          ),
        );
      }
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: SETTINGS_KEY });
      void queryClient.invalidateQueries({ queryKey: MODELS_KEY });
    },
  });
};
