import { commands } from '@raygun/bindings';
import type { SyncKind, SyncStatus } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

const SYNC = ['sync'];
/** Other devices pair and sync on their own: refresh while sync is on. */
const REFRESH_MS = 3000;

export const useSyncStatus = () =>
  useQuery({
    queryKey: SYNC,
    queryFn: () => unwrapResult(commands.syncStatus()),
    refetchInterval: (query) => (query.state.data?.enabled ? REFRESH_MS : false),
  });

/** Every change answers with the new status, which replaces the cached one. */
const useSyncChange = <Variables>(run: (variables: Variables) => Promise<SyncStatus>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: (status) => queryClient.setQueryData(SYNC, status),
  });
};

export const useSetSyncEnabled = () =>
  useSyncChange((enabled: boolean) => unwrapResult(commands.syncSetEnabled(enabled)));

export const useOpenPairing = () => useSyncChange(() => unwrapResult(commands.syncOpenPairing()));

export const useJoinDevice = () =>
  useSyncChange((text: string) => unwrapResult(commands.syncJoin(text)));

export const useSyncNow = () =>
  useSyncChange((deviceId: string | null) => unwrapResult(commands.syncNow(deviceId)));

export const useRemovePeer = () =>
  useSyncChange((deviceId: string) => unwrapResult(commands.syncRemovePeer(deviceId)));

export const useSetSyncKinds = () =>
  useSyncChange((kinds: SyncKind[]) => unwrapResult(commands.syncSetKinds(kinds)));

export const useRenameDevice = () =>
  useSyncChange((name: string) => unwrapResult(commands.syncSetDeviceName(name)));

export const useUndoLastSync = () => useSyncChange(() => unwrapResult(commands.syncUndoLast()));
