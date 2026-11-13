import type { Website } from '@raygun/bindings';
import { Pencil, RotateCcw, ShieldCheck, ShieldOff, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import type { MenuItem } from '../shell/contextMenuStore';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import { useResetWebsite, useSetWebsiteBlocking } from './useWebsites';
import { websitePin } from './websitePin';

export type WebsiteMenuItem = Extract<MenuItem, { onSelect: () => void }>;

const isItem = (item: MenuItem): item is WebsiteMenuItem => 'onSelect' in item;

/**
 * A saved site's menu items (M46.3): edit it, pin it to quick access, switch its ad blocker,
 * start over from its home page, remove it. Editing and removing open the lists' own sheets
 * (`WebsiteSheet`, `RemoveWebsiteSheet`); `reset` and `blocking` say when a change failed.
 */
export const useWebsiteMenuItems = () => {
  const { t } = useTranslation();
  const reset = useResetWebsite();
  const blocking = useSetWebsiteBlocking();
  const pinItems = usePinMenuItem();
  const items = (
    site: Website,
    sheets: { edit: () => void; remove: () => void },
  ): WebsiteMenuItem[] => [
    { label: t('websites.edit'), icon: Pencil, onSelect: sheets.edit },
    ...pinItems(websitePin(site)).filter(isItem),
    site.blockAds
      ? {
          label: t('websites.allowHere'),
          icon: ShieldOff,
          onSelect: () => blocking.mutate({ id: site.id, on: false }),
        }
      : {
          label: t('websites.blockHere'),
          icon: ShieldCheck,
          onSelect: () => blocking.mutate({ id: site.id, on: true }),
        },
    ...(site.lastUrl
      ? [{ label: t('websites.startOver'), icon: RotateCcw, onSelect: () => reset.mutate(site.id) }]
      : []),
    { label: t('websites.remove'), icon: Trash2, onSelect: sheets.remove },
  ];
  return { items, reset, blocking };
};
