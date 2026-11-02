import { commands } from '@raygun/bindings';
import type { RemoteStatus } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

const REMOTE = ['remote'];

/** While the remote is on, devices pair from elsewhere: refresh to show them and the used code. */
const REFRESH_MS = 3000;

/** The LAN remote (M20), which also lets other Raygun devices play here (M46.15). */
export const useRemoteStatus = () =>
  useQuery({
    queryKey: REMOTE,
    queryFn: () => unwrapResult(commands.remoteStatus()),
    refetchInterval: (query) => (query.state.data?.enabled ? REFRESH_MS : false),
  });

/** Every change answers with the new status, which replaces the cached one. */
export const useRemoteChange = <Variables>(
  run: (variables: Variables) => Promise<RemoteStatus>,
) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: (status) => queryClient.setQueryData(REMOTE, status),
  });
};
