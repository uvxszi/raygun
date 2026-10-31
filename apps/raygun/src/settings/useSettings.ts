import { commands } from '@raygun/bindings';
import type { SettingChange, Settings } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

const SETTINGS_KEY = ['settings'] as const;

export const useSettings = () =>
  useQuery({
    queryKey: SETTINGS_KEY,
    queryFn: () => unwrapResult(commands.settingsGet()),
  });

export const useUpdateSetting = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (change: SettingChange) => unwrapResult(commands.settingsUpdate(change)),
    onSuccess: (settings: Settings) => queryClient.setQueryData(SETTINGS_KEY, settings),
  });
};
