import type { ExtensionInfo } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { Copy, Eraser, Globe, Settings2, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { buildMenu, copyText } from '../music/mediaMenu';
import type { MenuItem } from '../shell/contextMenuStore';
import {
  useClearExtensionData,
  useOpenExtensionWebsite,
  useUninstallExtension,
} from './useExtensions';

/**
 * An installed extension's long-press and right-click menu: its page and settings, its website,
 * copying its name, clearing its data, uninstalling (asked first; built-in ones stay).
 */
export const useExtensionMenuItems = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const openWebsite = useOpenExtensionWebsite();
  const clearData = useClearExtensionData();
  const uninstall = useUninstallExtension();
  return (extension: ExtensionInfo): MenuItem[] =>
    buildMenu({
      heading: {
        label: extension.name,
        subtitle: extension.version,
        artworkUrl: extension.icon ?? null,
      },
      play: [
        {
          label: t('menu.openExtension'),
          icon: Settings2,
          onSelect: () =>
            void navigate({ to: '/extensions/detail', search: { id: extension.id } as never }),
        },
      ],
      goTo: [
        extension.website && {
          label: t('sources.website'),
          icon: Globe,
          onSelect: () => openWebsite.mutate(extension.id),
        },
      ],
      share: [{ label: t('menu.copyName'), icon: Copy, onSelect: () => copyText(extension.name) }],
      edit: [
        {
          label: t('sources.clearData'),
          icon: Eraser,
          onSelect: () => clearData.mutate(extension.id),
        },
      ],
      remove: [
        extension.origin === 'installed' && {
          label: t('extensions.uninstall'),
          icon: Trash2,
          danger: true,
          confirm: t('menu.uninstallConfirm', { name: extension.name }),
          onSelect: () => uninstall.mutate(extension.id),
        },
      ],
    });
};
