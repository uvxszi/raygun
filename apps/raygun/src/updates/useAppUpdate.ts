import { commands, events } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const KEY = ['appUpdate'] as const;

/** Raygun's newest release next to this copy (M46.47); follows the updater's events. */
export const useAppUpdate = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.appUpdateEvent.listen((event) => {
      client.setQueryData(KEY, event.payload);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  return useQuery({
    queryKey: KEY,
    queryFn: () => unwrapResult(commands.appUpdate(false)),
  });
};

export const useAppUpdateActions = () => {
  const client = useQueryClient();
  return {
    check: useMutation({
      mutationFn: () => unwrapResult(commands.appUpdate(true)),
      onSuccess: (update) => client.setQueryData(KEY, update),
    }),
    install: useMutation({
      mutationFn: () => unwrapResult(commands.appUpdateInstall()),
      onSuccess: (update) => client.setQueryData(KEY, update),
    }),
    openPage: useMutation({
      mutationFn: (url: string) => unwrapResult(commands.appOpenPage(url)),
    }),
  };
};
