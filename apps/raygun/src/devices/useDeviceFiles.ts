import { commands, events } from '@raygun/bindings';
import type { DeviceFileKind, DeviceFilesStatus } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';
const FILES = ['deviceFiles'];

/** The kinds a device may get, in the order they're listed. */
export const FILE_KINDS: DeviceFileKind[] = [
  'songs',
  'chapters',
  'episodes',
  'covers',
  'songEdits',
];

/** Each linked device's file choices and the transfer going on, kept current by Rust's events. */
export const useDeviceFiles = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.deviceFilesEvent.listen(({ payload }) => {
      client.setQueryData(FILES, payload);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  return useQuery({
    queryKey: FILES,
    queryFn: () => unwrapResult(commands.deviceFilesStatus()),
  });
};

export const useFilesChange = <Variables>(
  run: (variables: Variables) => Promise<DeviceFilesStatus>,
) => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSuccess: (status) => client.setQueryData(FILES, status),
  });
};
