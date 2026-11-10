import { useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

/**
 * Answers a switch doesn't change: the settings (already saved) and what repositories list (the
 * extension catalogue, thousands of entries with large stores, and the repositories themselves).
 */
const UNSWITCHED = new Set([
  'settings',
  'extensionCatalog',
  'extensionRepos',
  'extensionSuggestedRepos',
  'extensionRepoPreview',
]);

const NONE: string[] = [];

/**
 * Sources and whole extensions that are off (Settings' `disabledSources`, M46.8): a source's id
 * (`<extension>/<provider>`) turns that source off, an extension's id every source it has.
 */
export const useSourceSwitches = () => {
  const settings = useSettings();
  const update = useUpdateSetting();
  const client = useQueryClient();
  const disabled = settings.data?.disabledSources ?? NONE;
  // Asked for every row of long lists: one look-up each.
  const off = useMemo(() => new Set(disabled), [disabled]);
  const isEnabled = (id: string) => !off.has(id);
  const setMany = (ids: string[], on: boolean) => {
    const changing = new Set(ids);
    const value = on
      ? disabled.filter((id) => !changing.has(id))
      : [...disabled, ...ids.filter((id) => !off.has(id))];
    // Source lists, searches and everything a source feeds change with it; read again even when
    // what switched it has gone meanwhile (a source's menu closes as it switches).
    void update
      .mutateAsync({ key: 'disabledSources', value })
      .then(() =>
        client.invalidateQueries({
          predicate: (query) => {
            const [scope] = query.queryKey;
            return typeof scope !== 'string' || !UNSWITCHED.has(scope);
          },
        }),
      )
      .catch(() => undefined);
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
