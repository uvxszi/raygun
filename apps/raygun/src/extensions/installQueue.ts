import { commands } from '@raygun/bindings';
import type { CatalogEntry, ExtensionInfo } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { create } from 'zustand';

import { unwrapResult } from '../lib/commandResult';
import { CATALOG, EXTENSIONS } from './useExtensions';

/** An install waiting its turn, running, or failed (finished ones are dropped). */
export type InstallStatus =
  { state: 'queued' } | { state: 'installing' } | { state: 'failed'; message: string };

/** One catalog entry: the same extension can be listed by more than one repository. */
export const entryKey = (entry: Pick<CatalogEntry, 'repoId' | 'id'>) =>
  `${entry.repoId}/${entry.id}`;

type InstallQueueState = {
  status: Record<string, InstallStatus>;
  pending: CatalogEntry[];
  running: boolean;
};

const useQueueStore = create<InstallQueueState>(() => ({
  status: {},
  pending: [],
  running: false,
}));

const setStatus = (key: string, status: InstallStatus | null) =>
  useQueueStore.setState((state) => {
    const next = { ...state.status };
    if (status) next[key] = status;
    else delete next[key];
    return { status: next };
  });

/**
 * Installs one at a time, in the order asked (M46.7): each install reloads the extension host, so
 * they don't overlap. It keeps going when the screen is left; failures stay listed by their rows.
 */
const drain = async (stored: (extensions: ExtensionInfo[]) => void) => {
  if (useQueueStore.getState().running) return;
  useQueueStore.setState({ running: true });
  for (;;) {
    const [entry, ...rest] = useQueueStore.getState().pending;
    if (!entry) break;
    useQueueStore.setState({ pending: rest });
    const key = entryKey(entry);
    setStatus(key, { state: 'installing' });
    try {
      stored(await unwrapResult(commands.extensionInstall(entry.repoId, entry.id)));
      setStatus(key, null);
    } catch (error) {
      setStatus(key, {
        state: 'failed',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
  useQueueStore.setState({ running: false });
};

/** Every install's status, and a way to install entries (one, a selection, every update). */
export const useInstallQueue = () => {
  const queryClient = useQueryClient();
  const status = useQueueStore((state) => state.status);
  const install = (entries: CatalogEntry[]) => {
    const busy = useQueueStore.getState().status;
    const fresh = entries.filter((entry) => {
      const current = busy[entryKey(entry)]?.state;
      return current !== 'queued' && current !== 'installing';
    });
    if (fresh.length === 0) return;
    useQueueStore.setState((state) => ({
      pending: [...state.pending, ...fresh],
      status: {
        ...state.status,
        ...Object.fromEntries(fresh.map((entry) => [entryKey(entry), { state: 'queued' }])),
      },
    }));
    void drain((extensions) => {
      queryClient.setQueryData(EXTENSIONS, extensions);
      void queryClient.invalidateQueries({ queryKey: CATALOG });
    });
  };
  return { status, install };
};

/** Clears the queue between tests. */
export const resetInstallQueue = () =>
  useQueueStore.setState({ status: {}, pending: [], running: false });
