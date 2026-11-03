import { commands, events } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { HatGlasses } from 'lucide-react';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { noteIncognito } from '../shell/queryPersist';
import type { OverflowItem } from '../shell/OverflowMenu';

export const INCOGNITO_KEY = ['incognito'] as const;

const statusQuery = {
  queryKey: INCOGNITO_KEY,
  queryFn: () => commands.incognitoStatus(),
  staleTime: Infinity,
};

/**
 * Incognito (M47): while it's on, Rust records nothing that's played, watched, read or searched.
 * Whether it's on, and a way to turn it on or off.
 */
export const useIncognito = () => {
  const client = useQueryClient();
  const status = useQuery(statusQuery);
  const change = useMutation({
    mutationFn: (on: boolean) => unwrapResult(commands.incognitoSetEnabled(on)),
    onSuccess: (next) => client.setQueryData(INCOGNITO_KEY, next),
  });
  return {
    on: status.data?.on ?? false,
    set: (on: boolean) => change.mutate(on),
    busy: change.isPending,
  };
};

/**
 * Follows incognito turned on or off anywhere (another window, the app menu), and keeps answers
 * that come while it's on out of the saved query cache. Mounted once by the shell; returns
 * whether it's on, once known.
 */
export const useIncognitoSync = () => {
  const client = useQueryClient();
  const status = useQuery(statusQuery);
  useEffect(() => {
    const unlisten = events.incognitoEvent.listen((event) => {
      client.setQueryData(INCOGNITO_KEY, event.payload);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  useEffect(() => {
    if (status.data) noteIncognito(status.data);
  }, [status.data]);
  return status.data?.on;
};

/** Incognito in a ⋮ menu: turns it on, or off while it's on. */
export const useIncognitoItem = (): OverflowItem => {
  const { t } = useTranslation();
  const incognito = useIncognito();
  return {
    label: t(incognito.on ? 'incognito.turnOff' : 'incognito.turnOn'),
    icon: HatGlasses,
    onSelect: () => incognito.set(!incognito.on),
  };
};
