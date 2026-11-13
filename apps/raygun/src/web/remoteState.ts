// Whether a linked Raygun app answers this page's commands (docs/WEB.md). While one does, what
// needs the app works through it, so buttons that say "needs the app" are enabled instead.
import { useSyncExternalStore } from 'react';

let connected = false;
const listeners = new Set<() => void>();

export const remoteConnected = () => connected;

export const setRemoteConnected = (value: boolean) => {
  if (connected === value) return;
  connected = value;
  for (const listener of listeners) listener();
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

export const useRemoteConnected = () =>
  useSyncExternalStore(subscribe, remoteConnected, () => false);
