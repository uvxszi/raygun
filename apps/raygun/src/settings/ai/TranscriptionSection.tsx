import type { TranscriptionEngine, TranscriptionPrefs } from '@raygun/bindings';
import { Plus } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels, useAiProviders } from '../../ai/useAi';
import { formatBytes } from '../../lib/formatBytes';
import { PrefAction, PrefChoice, PrefGroup, PrefInfo, PrefLink, PrefText } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';
import { ModelDownloadRow } from './ModelRows';
import { ProviderForm } from './ProvidersSection';

const ENGINES: TranscriptionEngine[] = ['off', 'local', 'online'];
/** What an online provider hears with until another model is typed. */
const ONLINE_MODEL = 'whisper-1';

/**
 * Transcription (M46.32): who hears podcasts, audiobooks, videos and songs: Whisper on this device
 * (downloaded here), or an OpenAI-compatible provider. The players' "Set up transcription" links
 * here (`#transcription`).
 */
export const TranscriptionSection: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const models = useAiModels().data ?? [];
  const providers = useAiProviders().data ?? [];
  const [adding, setAdding] = useState(false);
  const prefs = settings.data?.transcription;
  const engine = prefs?.engine ?? 'off';
  const save = (change: Partial<TranscriptionPrefs>) =>
    prefs && update.mutate({ key: 'transcription', value: { ...prefs, ...change } });
  const speech = models.filter((model) => model.kind === 'speech');
  const local = speech.find((model) => model.id === prefs?.localModel) ?? speech[0];
  const compatible = providers.filter((provider) => provider.kind === 'openAiCompatible');

  return (
    <>
      <PrefGroup
        id="transcription"
        title={t('ai.transcription.title')}
        summary={t('ai.transcription.about')}
      >
        <PrefChoice
          title={t('ai.transcription.engine')}
          value={engine}
          disabled={!prefs}
          options={ENGINES.map((value) => ({
            value,
            label: t(`ai.transcription.engines.${value}`),
            hint: value === 'off' ? undefined : t(`ai.transcription.engineHints.${value}`),
          }))}
          onChange={(value) => save({ engine: value })}
        />
        {engine === 'local' && local && (
          <>
            <PrefChoice
              title={t('ai.transcription.model')}
              summary={local.installed ? t('ai.installed') : undefined}
              value={local.id}
              options={speech.map((model) => ({
                value: model.id,
                label: model.name,
                hint: model.installed ? t('ai.installed') : formatBytes(model.sizeBytes ?? 0),
              }))}
              onChange={(localModel) => save({ localModel })}
            />
            <ModelDownloadRow model={local} />
          </>
        )}
        {engine === 'online' && (
          <>
            {compatible.length > 0 ? (
              <PrefChoice
                title={t('ai.transcription.provider')}
                summary={t('ai.transcription.onlineHint')}
                value={prefs?.providerId ?? ''}
                options={[
                  { value: '', label: t('ai.transcription.pickProvider') },
                  ...compatible.map((provider) => ({ value: provider.id, label: provider.name })),
                ]}
                onChange={(providerId) => save({ providerId: providerId || null })}
              />
            ) : (
              <PrefAction
                icon={Plus}
                title={t('ai.transcription.addProvider')}
                summary={t('ai.transcription.addProviderSummary')}
                dialog
                onClick={() => setAdding(true)}
              />
            )}
            <PrefText
              title={t('ai.transcription.onlineModel')}
              value={prefs?.onlineModel ?? ONLINE_MODEL}
              placeholder={ONLINE_MODEL}
              onChange={(onlineModel) => save({ onlineModel: onlineModel || ONLINE_MODEL })}
            />
          </>
        )}
        <PrefLink
          title={t('ai.transcription.podcasts')}
          summary={t('ai.transcription.podcastsSummary')}
          to="/library"
          search={{ view: 'podcasts' }}
        />
        <PrefLink
          title={t('ai.transcription.videos')}
          summary={t('ai.transcription.videosSummary')}
          to="/library"
          search={{ media: 'video' }}
        />
        <PrefInfo
          title={t('ai.transcription.lyrics')}
          summary={t('ai.transcription.lyricsSummary')}
        />
      </PrefGroup>
      {adding && <ProviderForm kind="openAiCompatible" onDone={() => setAdding(false)} />}
    </>
  );
};
