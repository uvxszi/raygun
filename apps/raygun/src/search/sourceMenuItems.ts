import { useNavigate } from '@tanstack/react-router';
import { Ban, CircleCheck, Clock, Compass, Copy, Eraser, Globe, Settings2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import {
  useClearExtensionData,
  useExtension,
  useOpenExtensionWebsite,
} from '../extensions/useExtensions';
import { buildMenu, copyText } from '../music/mediaMenu';
import type { MenuItem } from '../shell/contextMenuStore';
import { useContentRating } from '../extensions/contentRating';
import { useSourceSwitch } from './useSourceSwitch';

type Page = { to: string; search: Record<string, unknown> };

/**
 * A source's long-press and right-click menu in Browse → Sources: open it or its latest, its
 * extension's settings and website, clearing its data, marking it 18+ or not, switching it off, copying its name.
 */
export const useSourceMenuItems = ({
  sourceId,
  extensionId,
  name,
  page,
  latest,
}: {
  sourceId: string;
  extensionId: string;
  name: string;
  page: Page;
  latest?: Page;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { enabled, setEnabled } = useSourceSwitch(sourceId);
  const extension = useExtension(extensionId).data;
  const website = extension?.website ?? null;
  const rating = useContentRating();
  const openWebsite = useOpenExtensionWebsite();
  const clearData = useClearExtensionData();
  return (): MenuItem[] =>
    buildMenu({
      heading: { label: name },
      play: [
        {
          label: t('sources.open'),
          icon: Compass,
          onSelect: () => void navigate({ to: page.to, search: page.search as never }),
        },
        latest && {
          label: t('browse.latestOf', { name }),
          icon: Clock,
          onSelect: () => void navigate({ to: latest.to, search: latest.search as never }),
        },
      ],
      goTo: [
        {
          label: t('sources.settings'),
          icon: Settings2,
          onSelect: () =>
            void navigate({ to: '/extensions/detail', search: { id: extensionId } as never }),
        },
        website && {
          label: t('sources.website'),
          icon: Globe,
          onSelect: () => openWebsite.mutate(extensionId),
        },
      ],
      share: [{ label: t('menu.copyName'), icon: Copy, onSelect: () => copyText(name) }],
      edit: [
        {
          label: t('sources.clearData'),
          icon: Eraser,
          onSelect: () => clearData.mutate(extensionId),
        },
        // Marks the whole extension, so its other sources follow.
        ...rating.menuItems('extensions', extensionId, extension?.adult ?? false),
      ],
      hide: [
        {
          label: t(enabled ? 'sources.disable' : 'sources.enable'),
          icon: enabled ? Ban : CircleCheck,
          onSelect: () => setEnabled(!enabled),
        },
      ],
    });
};
