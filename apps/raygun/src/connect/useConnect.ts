import { commands, events } from '@raygun/bindings';
import type { ConnectSession, ConnectStatus } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const STATUS_KEY = ['connect', 'status'] as const;
const DEVICES_KEY = ['connect', 'devices'] as const;

/** While the device list shows, devices come and go on the network. */
const DEVICES_REFRESH_MS = 3000;

/** Keeps Connect's status in the query cache as Rust reports it. Mount once, in the app shell. */
export const useConnectSync = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.connectEvent.listen((event) =>
      queryClient.setQueryData(STATUS_KEY, event.payload),
    );
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
};

/** Where this device's player plays (M46.15). */
export const useConnectStatus = () =>
  useQuery<ConnectStatus>({
    queryKey: STATUS_KEY,
    queryFn: () => commands.connectStatus(),
    staleTime: Infinity,
  });

/** The device this one plays on, with its player as last reported; `null` when it plays here. */
export const useConnectSession = (): ConnectSession | null =>
  useConnectStatus().data?.session ?? null;

/** Other devices to play on, refreshed while `shown`. */
export const useConnectDevices = (shown: boolean) =>
  useQuery({
    queryKey: DEVICES_KEY,
    queryFn: () => commands.connectDevices(),
    enabled: shown,
    refetchInterval: shown ? DEVICES_REFRESH_MS : false,
  });

/** Each change answers with the new status. */
const useStatusChange = <Variables>(run: (variables: Variables) => Promise<ConnectStatus>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: (status) => queryClient.setQueryData(STATUS_KEY, status),
    onSettled: () => queryClient.invalidateQueries({ queryKey: DEVICES_KEY }),
  });
};

export const useConnectTo = () =>
  useStatusChange((id: string) => unwrapResult(commands.connectTo(id)));

/** `true` brings the queue back here; `false` leaves the other device playing. */
export const useDisconnect = () =>
  useStatusChange((playHere: boolean) => unwrapResult(commands.connectDisconnect(playHere)));

export const useAddDevice = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (address: string) => unwrapResult(commands.connectAdd(address)),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: DEVICES_KEY }),
  });
};

export const useForgetDevice = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrapResult(commands.connectForget(id)),
    onSuccess: (devices) => queryClient.setQueryData(DEVICES_KEY, devices),
  });
};
