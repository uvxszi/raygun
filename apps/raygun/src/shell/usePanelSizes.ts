import type { PanelPrefs } from '@raygun/bindings';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

/** The saved panel sizes, and a way to change one (Settings' `panels`, M46.8). */
export const usePanelSizes = () => {
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const panels: PanelPrefs = {
    railWidth: 0,
    sidePanelWidth: 0,
    playerSplit: 0,
    ...settings?.panels,
  };
  const save = (patch: Partial<PanelPrefs>) =>
    update.mutate({ key: 'panels', value: { ...panels, ...patch } });
  return { panels, save };
};
