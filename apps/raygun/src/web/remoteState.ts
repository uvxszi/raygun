// Whether Raygun on the web is using a linked app as its back end (web/remote/), for the screens
// that show it. Outside the web build nothing sets it, so it stays off.
import { useSyncExternalStore } from 'react';

export type RemoteState = {
  /** A linked app could be used (the page is linked with one). */
  available: boolean;
  /** The switch: use a linked app when it's reachable. */
  enabled: boolean;
  /** The app in use, or tried. */
  deviceId: string | null;
  device: string | null;
  connected: boolean;
  connecting: boolean;
  /** Why the last try failed. */
  error: string | null;
};

export type RemoteControls = {
  setEnabled: (enabled: boolean) => void;
  /** Uses this linked app from now on. */
  choose: (deviceId: string) => void;
};

const OFF: RemoteState = {
  available: false,
  enabled: false,
  deviceId: null,
  device: null,
  connected: false,
  connecting: false,
  error: null,
};

let state: RemoteState = OFF;
let controls: RemoteControls | null = null;
const listeners = new Set<() => void>();

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

export const remoteState = (): RemoteState => state;

export const setRemoteState = (next: Partial<RemoteState>) => {
  state = { ...state, ...next };
  for (const listener of [...listeners]) listener();
};

export const setRemoteControls = (next: RemoteControls | null) => {
  controls = next;
};

export const remoteControls = (): RemoteControls | null => controls;

export const useRemoteState = (): RemoteState => useSyncExternalStore(subscribe, remoteState);

export const useRemoteConnected = (): boolean =>
  useSyncExternalStore(subscribe, () => state.connected);

/** Whether a linked app answers for the page now. */
export const remoteConnected = (): boolean => state.connected;

/** Marks a linked app as answering (or not), as `setRemoteState` does with more. */
export const setRemoteConnected = (connected: boolean) => setRemoteState({ connected });
