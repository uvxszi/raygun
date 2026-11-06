import type { ExtensionInfo } from '@raygun/bindings';
import type { TFunction } from 'i18next';

/**
 * Where an extension came from, as its rows and page show it: the repository it was installed
 * from (`Paraline`), or "added by you" for one added on its own. Raygun ships no sources.
 */
export const originLabel = (extension: Pick<ExtensionInfo, 'repository'>, t: TFunction): string =>
  extension.repository ?? t('extensions.addedByYou');
