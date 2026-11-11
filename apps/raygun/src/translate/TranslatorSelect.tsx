import { commands } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAiModels } from '../ai/useAi';
import { modelsFor } from '../readerAi/useReaderAi';
import { useTranslatingProviders } from './translators';

/**
 * Who translates, named as the readers' layers name them: '' for this device (its models picked
 * by language), an installed model's id (this device, that model first), or `ai:<provider id>`
 * for a provider from Settings → AI.
 */
export const TranslatorSelect: FC<{
  value: string;
  label: string;
  onChange: (value: string) => void;
  className?: string;
}> = ({ value, label, onChange, className }) => {
  const { t } = useTranslation();
  const models = modelsFor(useAiModels().data ?? [], 'translate').filter(
    (model) => model.installed,
  );
  const providers = useTranslatingProviders();
  // One picked before and removed since still names itself, so the field shows what's asked.
  const known =
    value === '' ||
    models.some((model) => model.id === value) ||
    providers.some((provider) => `ai:${provider.id}` === value);
  return (
    <select
      value={value}
      aria-label={label}
      onChange={(event) => onChange(event.target.value)}
      className={className}
    >
      <option value="">{t('translate.device')}</option>
      {models.map((model) => (
        <option key={model.id} value={model.id}>
          {t('translate.deviceModel', { name: model.name })}
        </option>
      ))}
      {providers.map((provider) => (
        <option key={provider.id} value={`ai:${provider.id}`}>
          {provider.name}
        </option>
      ))}
      {!known && <option value={value}>{t('translate.removed')}</option>}
    </select>
  );
};

/**
 * Google's attribution beside its translations, as its Cloud Translation terms ask (the text
 * style): it links to Google Translate.
 */
export const PoweredByGoogle: FC = () => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      data-testid="google-attribution"
      onClick={() => void commands.appOpenLink('https://translate.google.com')}
      className="text-muted hover:text-foreground self-start text-2xs"
    >
      {t('translate.poweredByGoogle')}
    </button>
  );
};
