import { commands, events } from '@raygun/bindings';
import type { SettingChange, Settings } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

export const SETTINGS_KEY = ['settings'] as const;

export const useSettings = () =>
  useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: () => unwrapResult(commands.settingsGet()),
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

export const useUpdateSetting = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (change: SettingChange) => unwrapResult(commands.settingsUpdate(change)),
    onSuccess: (settings: Settings) => queryClient.setQueryData(SETTINGS_KEY, settings),
  });
};
