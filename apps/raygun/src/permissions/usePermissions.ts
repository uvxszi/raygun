import { commands } from '@raygun/bindings';
import type { AppPermission, PermissionKind } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

export const PERMISSIONS_KEY = ['permissions'] as const;

/**
 * Every permission Raygun uses on this device and where it stands (Rust asks Android). Read again
 * when Raygun comes back to the screen: the system settings may have changed one meanwhile.
 */
export const usePermissions = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const again = () => {
      if (document.visibilityState === 'visible') {
        void queryClient.invalidateQueries({ queryKey: PERMISSIONS_KEY });
      }
    };
    document.addEventListener('visibilitychange', again);
    return () => document.removeEventListener('visibilitychange', again);
  }, [queryClient]);
  return useQuery({
    queryKey: PERMISSIONS_KEY,
    queryFn: () => unwrapResult(commands.permissionsList()),
  });
};

/** Asks for a permission: Android's question, or its page in the system settings. */
export const useRequestPermission = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (kind: PermissionKind) => unwrapResult(commands.permissionRequest(kind)),
    onSuccess: (states: AppPermission[]) => queryClient.setQueryData(PERMISSIONS_KEY, states),
  });
};

/** Whether asking shows Android's question (rather than its settings page). */
export const canAsk = (permission: AppPermission) =>
  permission.state === 'denied' || permission.state === 'optimized';
