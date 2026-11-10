import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels, useAiProviders } from '../../ai/useAi';
import { nativeName } from '../../i18n/languages';
import { useReaderAiLayers } from '../../readerAi/useReaderAi';
import { useTranslationPrefs } from '../../sound/lyricsTranslation';
import { PrefGroup, PrefLink } from '../prefs';
import { ReaderLayersRow } from './ReaderLayersRow';
import { readerTranslation } from './translators';

/**
 * Translation: what each reader's translation layers translate with (their layers open here as
 * in the reader), where lyrics and video subtitles pick theirs, and the translators there are:
 * this device's models and the online providers.
 */
export const TranslationSection: FC = () => {
  const { t } = useTranslation();
  const providers = useAiProviders().data ?? [];
  const models = useAiModels().data ?? [];
  const comic = useReaderAiLayers('comic').layers;
  const novel = useReaderAiLayers('novel').layers;
  const lyrics = useTranslationPrefs();
  const onDevice = models.filter((model) => model.kind === 'translate' && model.installed).length;

  return (
    <PrefGroup
      id="translation"
      title={t('ai.translation.title')}
      summary={t('ai.translation.about')}
    >
      <ReaderLayersRow
        medium="comic"
        title={t('ai.translation.comics')}
        summary={readerTranslation(comic, providers, t)}
      />
      <ReaderLayersRow
        medium="novel"
        title={t('ai.translation.books')}
        summary={readerTranslation(novel, providers, t)}
      />
      <PrefLink
        title={t('ai.translation.lyrics')}
        summary={t('ai.translation.lyricsSummary', { language: nativeName(lyrics.language) })}
        to="/settings/music"
        hash="lyricsTranslate.settingsTitle"
      />
      <PrefLink
        title={t('ai.translation.subtitles')}
        summary={t('ai.translation.subtitlesSummary')}
        to="/library"
        search={{ media: 'video' }}
      />
      <PrefLink
        title={t('ai.translation.device')}
        summary={
          onDevice > 0
            ? t('ai.translation.models', { count: onDevice })
            : t('ai.translation.noModels')
        }
        to="/settings/ai"
        hash="models"
      />
      <PrefLink
        title={t('ai.translation.online')}
        summary={
          providers.length > 0
            ? providers.map((provider) => provider.name).join(', ')
            : t('ai.translation.noProviders')
        }
        to="/settings/ai"
        hash="providers"
      />
    </PrefGroup>
  );
};
