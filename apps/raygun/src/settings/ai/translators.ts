import type { AiProvider } from '@raygun/bindings';
import type { TFunction } from 'i18next';

import { nativeName } from '../../i18n/languages';
import type { Layer } from '../../readerAi/useReaderAi';

/**
 * Who a reader's translation layer translates with: a provider from Settings → AI by its name
 * (`ai:<id>`), else this device (a catalogue model, or the one picked by language).
 */
export const translatorName = (model: string, providers: AiProvider[], t: TFunction) => {
  if (!model.startsWith('ai:')) return t('ai.translation.device');
  const id = model.slice('ai:'.length);
  return providers.find((provider) => provider.id === id)?.name ?? t('ai.translation.online');
};

/** A reader's translation layers that are on, as one line ("Claude · into German"), or Off. */
export const readerTranslation = (layers: Layer[], providers: AiProvider[], t: TFunction) => {
  const on = layers.filter((layer) => layer.kind === 'translate' && layer.enabled);
  if (on.length === 0) return t('ai.translation.off');
  return on
    .map((layer) => {
      const translator = translatorName(layer.model, providers, t);
      return layer.to
        ? t('ai.translation.layer', { translator, language: nativeName(layer.to) })
        : translator;
    })
    .join(', ');
};
