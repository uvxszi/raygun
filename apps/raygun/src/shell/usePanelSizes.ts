import type { PanelPrefs, Settings } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';

import { SETTINGS_KEY, useSettings, useUpdateSetting } from '../settings/useSettings';

/** The saved panel sizes, and a way to change one (Settings' `panels`, M46.8). */
export const usePanelSizes = () => {
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const client = useQueryClient();
  const panels: PanelPrefs = {
    railWidth: 0,
    sidePanelWidth: 0,
    playerSplit: 0,
    nextUpRows: 0,
    railNavHeight: 0,
    ...settings?.panels,
  };
  const save = (patch: Partial<PanelPrefs>) => {
    const value = { ...panels, ...patch };
    // Shown at once (M46.37): a panel let go keeps its size instead of jumping back for a frame
    // while the setting is saved.
    client.setQueryData<Settings>(SETTINGS_KEY, (old) => (old ? { ...old, panels: value } : old));
    update.mutate({ key: 'panels', value });
  };
  return { panels, save };
};
