import { commands, events } from '@raygun/bindings';
import type { SettingChange, Settings } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

export const SETTINGS_KEY = ['settings'] as const;
/** Every settings change carries this key, so a screen can find the ones that failed. */
export const SETTINGS_MUTATION = ['settings', 'update'] as const;
const DEFAULTS_KEY = ['settings', 'defaults'] as const;

export const useSettings = () =>
  useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: () => unwrapResult(commands.settingsGet()),
  });

/** Every setting as a fresh install has it (Rust's defaults), for Reset to defaults. */
export const useSettingsDefaults = () =>
  useQuery({
    queryKey: DEFAULTS_KEY,
    queryFn: () => commands.settingsDefaults(),
    staleTime: Infinity,
  });

/** Settings changed outside the Settings screens (in the video player) are read again. */
export const useSettingsEvents = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.settingsChangedEvent.listen(() => {
      void queryClient.invalidateQueries({ queryKey: SETTINGS_KEY });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
};

/** The settings with `changes` applied, as Rust will store them. */
const withChanges = (settings: Settings, changes: SettingChange[]): Settings =>
  changes.reduce<Settings>((next, change) => ({ ...next, [change.key]: change.value }), settings);

/**
 * Shows changes at once and saves them one after another. Rows build a change from the shown
 * settings (`{ ...prefs, font }`), so a second change made before the first is saved must see
 * the first, and must be written after it.
 */
const useSettingsMutation = <Variables>(
  run: (variables: Variables) => Promise<Settings>,
  changesOf: (variables: Variables) => SettingChange[],
) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: SETTINGS_MUTATION,
    scope: { id: 'settings' },
    mutationFn: run,
    onMutate: async (variables: Variables) => {
      await queryClient.cancelQueries({ queryKey: SETTINGS_KEY, exact: true });
      const shown = queryClient.getQueryData<Settings>(SETTINGS_KEY);
      if (shown) queryClient.setQueryData(SETTINGS_KEY, withChanges(shown, changesOf(variables)));
    },
    onSuccess: (settings: Settings) => {
      // A later change already shows; its own answer replaces this one.
      if (pendingChanges(queryClient) <= 1) queryClient.setQueryData(SETTINGS_KEY, settings);
    },
    // What Rust kept, instead of what was shown.
    onError: () => void queryClient.invalidateQueries({ queryKey: SETTINGS_KEY, exact: true }),
  });
};

const pendingChanges = (queryClient: QueryClient) =>
  queryClient.isMutating({ mutationKey: SETTINGS_MUTATION });

export const useUpdateSetting = () =>
  useSettingsMutation(
    (change: SettingChange) => unwrapResult(commands.settingsUpdate(change)),
    (change) => [change],
  );

/** Several settings at once (a screen's Reset to defaults): all saved, or none. */
export const useUpdateSettings = () =>
  useSettingsMutation(
    (changes: SettingChange[]) => unwrapResult(commands.settingsUpdateMany(changes)),
    (changes) => changes,
  );
