import type { Settings } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { EyeOff, LayoutList } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import { withLayout } from '../layout/layout';
import { SETTINGS_KEY, useUpdateSetting } from '../settings/useSettings';
import type { MenuItem } from '../shell/contextMenuStore';
import { showToast } from '../shell/toastStore';

/** A page a row leads to (its title and Show all). */
export type RowLink = { to: string; search?: Record<string, unknown> };

/**
 * A Home row's menu: Hide this row, at once and with Undo, and Edit Home, which brings rows back
 * too. Reads the layout as it is when pressed, so an Undo after other changes keeps them.
 */
export const useHomeRowActions = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const update = useUpdateSetting();
  const setShown = (id: string, visible: boolean) => {
    const settings = client.getQueryData<Settings>(SETTINGS_KEY);
    if (!settings) return;
    const layout = withLayout(settings.layout);
    const next = {
      ...layout,
      homeSections: layout.homeSections.map((item) =>
        item.id === id ? { ...item, visible } : item,
      ),
    };
    client.setQueryData<Settings>(SETTINGS_KEY, { ...settings, layout: next });
    update.mutate({ key: 'layout', value: next });
  };
  return (id: string, title: string): MenuItem[] => [
    { kind: 'heading', label: title },
    {
      label: t('home.hideRow'),
      icon: EyeOff,
      onSelect: () => {
        setShown(id, false);
        showToast(t('home.rowHidden', { name: title }), () => setShown(id, true));
      },
    },
    {
      label: t('layout.editHome'),
      icon: LayoutList,
      onSelect: () => void navigate({ to: '/settings/layout/homeSections' }),
    },
  ];
};
