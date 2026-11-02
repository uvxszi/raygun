import { useQueryClient } from '@tanstack/react-query';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

/** Whether a source is on, and a way to switch it (Settings' `disabledSources`, M46.8). */
export const useSourceSwitch = (sourceId: string) => {
  const settings = useSettings();
  const update = useUpdateSetting();
  const client = useQueryClient();
  const disabled = settings.data?.disabledSources ?? [];
  const enabled = !disabled.includes(sourceId);
  const setEnabled = (on: boolean) =>
    update.mutate(
      {
        key: 'disabledSources',
        value: on ? disabled.filter((id) => id !== sourceId) : [...disabled, sourceId],
      },
      {
        // Source lists and searches change with it.
        onSuccess: () => {
          for (const medium of ['manga', 'novel', 'video']) {
            void client.invalidateQueries({ queryKey: [medium] });
          }
          void client.invalidateQueries({ queryKey: ['onlineSearch'] });
        },
      },
    );
  return { enabled, setEnabled, busy: update.isPending };
};
