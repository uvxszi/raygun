import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../../ai/useAi';
import { useCollectionSheets } from '../../collections/collectionSheets';
import { PrefAction, PrefGroup, PrefInfo } from '../prefs';
import { ModelDownloadRow } from './ModelRows';
import { ReaderLayersRow } from './ReaderLayersRow';

/**
 * Text reading: what reads the words on pictures (Windows' own reader, elsewhere a model
 * downloaded here), for song lists in screenshots and comic pages to translate.
 */
export const TextReadingSection: FC = () => {
  const { t } = useTranslation();
  const support = useQuery({
    queryKey: ['readerAi', 'support'],
    queryFn: () => commands.readerAiSupport(),
    staleTime: Infinity,
  });
  const models = useAiModels().data ?? [];
  const builtIn = support.data?.textReaderBuiltIn ?? false;
  const model = builtIn
    ? undefined
    : models.find((each) => each.id === support.data?.textReaderModel);
  const reader = builtIn
    ? t('ai.textReading.builtIn')
    : model && [model.name, model.installed ? t('ai.installed') : null].filter(Boolean).join(' · ');

  return (
    <PrefGroup
      id="text-reading"
      title={t('ai.textReading.title')}
      summary={t('ai.textReading.about')}
    >
      <PrefInfo title={t('ai.textReading.reader')} summary={reader || undefined} />
      {model && <ModelDownloadRow model={model} />}
      <PrefAction
        title={t('ai.textReading.import')}
        summary={t('ai.textReading.importSummary')}
        dialog
        onClick={() => useCollectionSheets.getState().openImport()}
      />
      <ReaderLayersRow
        medium="comic"
        title={t('ai.textReading.comics')}
        summary={t('ai.textReading.comicsSummary')}
      />
    </PrefGroup>
  );
};
