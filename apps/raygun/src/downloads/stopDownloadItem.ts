import type { TFunction } from 'i18next';
import { CircleX } from 'lucide-react';

import type { MenuItem } from '../shell/contextMenuStore';

/** Stop download in a menu, asked in the menu's place. */
export const stopDownloadItem = (t: TFunction, name: string, onStop: () => void): MenuItem => ({
  label: t('downloads.stop.menu'),
  icon: CircleX,
  confirm: t('downloads.stop.ask', { name }),
  onSelect: onStop,
});
