import { commands, events } from '@raygun/bindings';
import type { ChipId, ChipInfo } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

export const CHIPS_KEY = ['chips'] as const;

/** Clients whose chip list follows Rust's `ChipsEvent`s (one listener each, however many use it). */
const following = new WeakSet<QueryClient>();

const follow = (client: QueryClient) => {
  if (following.has(client)) return;
  following.add(client);
  void events.chipsEvent.listen(({ payload }) => {
    client.setQueryData<ChipInfo[]>(CHIPS_KEY, payload);
  });
};

/** The Smart Chips catalogue with what this device has of each, live. */
export const useChips = () => {
  const client = useQueryClient();
  useEffect(() => follow(client), [client]);
  return useQuery({ queryKey: CHIPS_KEY, queryFn: () => commands.chipsList() });
};

/** Whether a chip is installed and on: what a feature it gates checks. */
export const isChipEnabled = (chips: ChipInfo[] | undefined, id: ChipId) =>
  chips?.some((chip) => chip.id === id && chip.installed && chip.on) ?? false;

export const useChipEnabled = (id: ChipId) => isChipEnabled(useChips().data, id);

/** The same, outside a component (keyboard shortcuts). */
export const chipEnabledIn = (client: QueryClient, id: ChipId) =>
  isChipEnabled(client.getQueryData<ChipInfo[]>(CHIPS_KEY), id);

/** Install, cancel, remove and switch; each answer replaces the list. */
export const useChipActions = () => {
  const client = useQueryClient();
  const settle = (list: ChipInfo[]) => client.setQueryData<ChipInfo[]>(CHIPS_KEY, list);
  const refresh = () => client.invalidateQueries({ queryKey: CHIPS_KEY });
  const install = useMutation({
    mutationFn: (id: ChipId) => unwrapResult(commands.chipsInstall(id)),
    onSuccess: settle,
    onError: refresh,
  });
  const cancel = useMutation({
    mutationFn: async (id: ChipId) => commands.chipsCancel(id),
    onSuccess: settle,
  });
  const remove = useMutation({
    mutationFn: (id: ChipId) => unwrapResult(commands.chipsRemove(id)),
    onSuccess: settle,
    onError: refresh,
  });
  const setEnabled = useMutation({
    mutationFn: ({ id, enabled }: { id: ChipId; enabled: boolean }) =>
      unwrapResult(commands.chipsSetEnabled(id, enabled)),
    onSuccess: settle,
    onError: refresh,
  });
  return { install, cancel, remove, setEnabled };
};
