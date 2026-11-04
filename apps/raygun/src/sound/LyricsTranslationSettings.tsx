import { Cpu } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../ai/useAi';
import { LANGUAGES, nativeName } from '../i18n/languages';
import { formatBytes } from '../lib/formatBytes';
import { PrefChoice, PrefGroup, PrefLink, PrefSwitch } from '../settings/prefs';
import { useUpdateSetting } from '../settings/useSettings';
import { useTranslationPrefs } from './lyricsTranslation';
import type { TranslationPrefs } from './lyricsTranslation';

/**
 * Settings → Sound → Lyrics translation (M46.48): the language translations show in, whether
 * they show by themselves, romanized lines, and the model "On this device" uses.
 */
export const LyricsTranslationSettings: FC = () => {
  const { t, i18n } = useTranslation();
  const { prefs, loaded } = useTranslationPrefs();
  const update = useUpdateSetting();
  const models = useAiModels();
  const save = (change: Partial<TranslationPrefs>) =>
    update.mutate({ key: 'lyricsTranslation', value: { ...prefs, ...change } });
  const translators = (models.data ?? []).filter((model) => model.kind === 'translate');

  return (
    <PrefGroup title={t('lyricsTranslate.settingsTitle')}>
      <PrefChoice
        title={t('lyricsTranslate.into')}
        value={prefs.language}
        disabled={!loaded}
        options={[
          {
            value: '',
            label: t('lyricsTranslate.appLanguage', { language: nativeName(i18n.language) }),
          },
          ...LANGUAGES.map((tag) => ({ value: tag, label: nativeName(tag) })),
        ]}
        onChange={(language) => save({ language })}
      />
      <PrefSwitch
        title={t('lyricsTranslate.automatic')}
        summary={t('lyricsTranslate.automaticHint')}
        checked={prefs.automatic}
        disabled={!loaded}
        onChange={(automatic) => save({ automatic })}
      />
      <PrefSwitch
        title={t('lyricsTranslate.romanized')}
        summary={t('lyricsTranslate.romanizedHint')}
        checked={prefs.romanized}
        disabled={!loaded}
        onChange={(romanized) => save({ romanized })}
      />
      <PrefChoice
        title={t('lyricsTranslate.model')}
        value={prefs.localModel}
        disabled={!loaded}
        options={[
          { value: '', label: t('lyricsTranslate.modelAutomatic') },
          ...translators.map((model) => ({
            value: model.id,
            label: model.name,
            hint: model.installed
              ? t('lyricsTranslate.installed')
              : t('lyricsTranslate.download', { size: formatBytes(model.sizeBytes ?? 0) }),
          })),
        ]}
        onChange={(localModel) => save({ localModel })}
      />
      <PrefLink
        title={t('lyricsTranslate.models')}
        summary={t('lyricsTranslate.modelsHint')}
        to="/settings/ai"
        icon={Cpu}
      />
    </PrefGroup>
  );
};
