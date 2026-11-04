import { useQueryClient } from '@tanstack/react-query';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

/**
 * Sources and whole extensions that are off (Settings' `disabledSources`, M46.8): a source's id
 * (`<extension>/<provider>`) turns that source off, an extension's id every source it has.
 */
export const useSourceSwitches = () => {
  const settings = useSettings();
  const update = useUpdateSetting();
  const client = useQueryClient();
  const disabled = settings.data?.disabledSources ?? [];
  const isEnabled = (id: string) => !disabled.includes(id);
  const setMany = (ids: string[], on: boolean) => {
    const changing = new Set(ids);
    const value = on
      ? disabled.filter((id) => !changing.has(id))
      : [...disabled, ...ids.filter((id) => !disabled.includes(id))];
    update.mutate(
      { key: 'disabledSources', value },
      {
        // Source lists, searches and everything a source feeds change with it.
        onSuccess: () =>
          void client.invalidateQueries({ predicate: (query) => query.queryKey[0] !== 'settings' }),
      },
    );
  };
  return { isEnabled, setMany, busy: update.isPending };
};

/** Whether a source (or an extension) is on, and a way to switch it. */
export const useSourceSwitch = (sourceId: string) => {
  const switches = useSourceSwitches();
  return {
    enabled: switches.isEnabled(sourceId),
    setEnabled: (on: boolean) => switches.setMany([sourceId], on),
    busy: switches.busy,
  };
};
