import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../../ai/useAi';
import { formatBytes } from '../../lib/formatBytes';
import { PrefBlock, PrefGroup, PrefInfo } from '../prefs';
import { Problem } from '../Problem';
import { ModelRow } from './ModelRows';
import { listedModels, modelStorage } from './modelUse';

/**
 * On-device models: every model Raygun runs here, by what it's for, with its size and Download or
 * Delete, and the space the downloaded ones take.
 */
export const ModelsSection: FC = () => {
  const { t } = useTranslation();
  const models = useAiModels();
  const all = models.data ?? [];
  const storage = modelStorage(all);

  return (
    <PrefGroup id="models" title={t('ai.onDevice.title')} summary={t('ai.onDevice.about')}>
      <PrefInfo
        title={t('ai.onDevice.storage')}
        summary={
          storage.count > 0
            ? t('ai.onDevice.storageSummary', {
                count: storage.count,
                size: formatBytes(storage.bytes),
              })
            : t('ai.onDevice.storageNone')
        }
      />
      {listedModels(all).map((model) => (
        <ModelRow key={model.id} model={model} />
      ))}
      {models.error && (
        <PrefBlock>
          <Problem title={t('ai.onDevice.listFailed')} detail={models.error.message} />
        </PrefBlock>
      )}
    </PrefGroup>
  );
};
