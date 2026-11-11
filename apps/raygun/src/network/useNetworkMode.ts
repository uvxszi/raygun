import { commands, events } from '@raygun/bindings';
import type { DataSaverPrefs, NetworkStatus } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { WifiOff } from 'lucide-react';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import type { OverflowItem } from '../shell/OverflowMenu';
import { setImagesWhenVisible } from '../shell/imageMemory';
import { SETTINGS_KEY, useSettings } from '../settings/useSettings';

export const NETWORK_KEY = ['network'] as const;

/** Whether an error comes from Offline Mode (Rust's "Offline mode is on"). */
export const isOfflineError = (text: unknown) =>
  typeof text === 'string' && /offline mode is on/i.test(text);

/** Before Rust answers: online, nothing saved. */
const ONLINE: NetworkStatus = { offline: false, dataSaver: false, mobileData: false };

const statusQuery = {
  queryKey: NETWORK_KEY,
  queryFn: () => commands.networkStatus(),
  staleTime: Infinity,
};

/** Whether Offline Mode is on and Data Saver applies now, as Rust says. */
export const useNetworkStatus = (): NetworkStatus => useQuery(statusQuery).data ?? ONLINE;

/**
 * Offline Mode: while it's on, Raygun sends nothing over the network; what's on the device keeps
 * working and downloads wait. Whether it's on, and a way to turn it on or off.
 */
export const useOfflineMode = () => {
  const client = useQueryClient();
  const status = useNetworkStatus();
  const change = useMutation({
    mutationFn: (offline: boolean) => unwrapResult(commands.networkSetOffline(offline)),
    onSuccess: (next) => {
      client.setQueryData(NETWORK_KEY, next);
      void client.invalidateQueries({ queryKey: SETTINGS_KEY, exact: true });
    },
  });
  return {
    on: status.offline,
    set: (offline: boolean) => change.mutate(offline),
    busy: change.isPending,
  };
};

/**
 * Whether a part of Data Saver applies now: Data Saver applies (its switch, and mobile data when
 * it's only for that) and that part of it is on.
 */
export const useDataSaving = (part: keyof Omit<DataSaverPrefs, 'enabled' | 'mobileDataOnly'>) => {
  const status = useNetworkStatus();
  const saver = useSettings().data?.network?.dataSaver;
  return status.dataSaver && Boolean(saver?.[part]);
};

/**
 * Follows Offline Mode and Data Saver turned on or off anywhere (another window, the app menu, a
 * setting) and marks the document with them (`data-offline`, `data-data-saver`). Mounted once by
 * the shell.
 */
export const useNetworkSync = () => {
  const client = useQueryClient();
  const status = useNetworkStatus();
  const whenVisible = useDataSaving('imagesWhenVisible');
  useEffect(() => {
    const unlisten = events.networkStatusEvent.listen((event) => {
      client.setQueryData(NETWORK_KEY, event.payload);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  useEffect(() => {
    const root = document.documentElement;
    root.toggleAttribute('data-offline', status.offline);
    root.toggleAttribute('data-data-saver', status.dataSaver);
  }, [status.offline, status.dataSaver]);
  useEffect(() => setImagesWhenVisible(whenVisible), [whenVisible]);
};

/** Offline Mode in a ⋮ menu: turns it on, or off while it's on. */
export const useOfflineItem = (): OverflowItem => {
  const { t } = useTranslation();
  const offline = useOfflineMode();
  return {
    label: t(offline.on ? 'offline.turnOff' : 'offline.turnOn'),
    icon: WifiOff,
    onSelect: () => offline.set(!offline.on),
  };
};
