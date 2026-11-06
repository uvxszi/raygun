import { commands, events } from '@raygun/bindings';
import type { CatalogEntry, ExtensionInstallPhase, ExtensionInstallStatus } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';
import { create } from 'zustand';

import { CATALOG, EXTENSIONS } from './useExtensions';

/** An install waiting its turn, running, just finished (shown a moment), or failed. */
export type InstallStatus =
  | { state: 'queued' }
  | { state: 'installing'; phase: ExtensionInstallPhase | null }
  | { state: 'done' }
  | { state: 'failed'; message: string };

/** How long a finished install says so on its row. */
const DONE_SHOWN_MS = 4000;

/** One catalog entry: the same extension can be listed by more than one repository. */
export const entryKey = (entry: { repoId: string; id: string }) => `${entry.repoId}/${entry.id}`;

const itemKey = (item: { repoId: string; extensionId: string }) =>
  `${item.repoId}/${item.extensionId}`;

export const EMPTY_INSTALLS: ExtensionInstallStatus = {
  batch: 0,
  items: [],
  total: 0,
  done: 0,
  failed: 0,
  cancelled: 0,
  running: false,
  current: null,
  phase: null,
};

type InstallQueueState = {
  queue: ExtensionInstallStatus;
  /** Installs that just finished, shown as such for a moment. */
  justDone: ReadonlySet<string>;
};

const useQueueStore = create<InstallQueueState>(() => ({
  queue: EMPTY_INSTALLS,
  justDone: new Set(),
}));

let follower: { client: QueryClient; unlisten: Promise<() => void> } | null = null;
const timers = new Set<ReturnType<typeof setTimeout>>();

/** Rust's queue as it is now; finished installs refresh the catalogue's states. */
const receive = (queue: ExtensionInstallStatus) => {
  const before = useQueueStore.getState();
  const wasDone = new Set(before.queue.items.filter((item) => item.state === 'done').map(itemKey));
  const fresh = queue.items
    .filter((item) => item.state === 'done' && !wasDone.has(itemKey(item)))
    .map(itemKey);
  const ended = queue.items.some(
    (item) =>
      (item.state === 'done' || item.state === 'failed') &&
      before.queue.items.find((old) => itemKey(old) === itemKey(item))?.state !== item.state,
  );
  useQueueStore.setState({
    queue,
    justDone: fresh.length ? new Set([...before.justDone, ...fresh]) : before.justDone,
  });
  if (fresh.length) {
    const timer = setTimeout(() => {
      timers.delete(timer);
      useQueueStore.setState((state) => {
        const next = new Set(state.justDone);
        for (const key of fresh) next.delete(key);
        return { justDone: next };
      });
    }, DONE_SHOWN_MS);
    timers.add(timer);
  }
  if (ended) void follower?.client.invalidateQueries({ queryKey: CATALOG });
  // Installed extensions load once the whole queue is through (the last step of `running`), so
  // the catalogue and the installed list are read again then too; read only when an install
  // ends, they still showed just-installed ones as not installed.
  if (before.queue.running && !queue.running) {
    void follower?.client.invalidateQueries({ queryKey: CATALOG });
    void follower?.client.invalidateQueries({ queryKey: EXTENSIONS });
    // A repository's Stremio addon installs as an addon.
    void follower?.client.invalidateQueries({ queryKey: ['stremio'] });
  }
};

/** Follows Rust's queue once for the whole app, from its state now (the UI may have reloaded). */
const follow = (client: QueryClient) => {
  if (follower) {
    follower.client = client;
    return;
  }
  follower = {
    client,
    unlisten: events.extensionInstallsEvent.listen((event) => receive(event.payload)),
  };
  void Promise.resolve(commands.extensionInstallsStatus())
    .then((queue) => {
      if (queue) receive(queue);
    })
    .catch(() => undefined);
};

/** The queue, each entry's status, and ways to add, cancel and retry installs. */
export const useInstallQueue = () => {
  const queryClient = useQueryClient();
  useEffect(() => follow(queryClient), [queryClient]);
  const queue = useQueueStore((state) => state.queue);
  const justDone = useQueueStore((state) => state.justDone);
  const status = useMemo(() => {
    const byKey: Record<string, InstallStatus> = {};
    for (const item of queue.items) {
      const key = itemKey(item);
      if (item.state === 'queued') byKey[key] = { state: 'queued' };
      else if (item.state === 'installing') byKey[key] = { state: 'installing', phase: item.phase };
      else if (item.state === 'failed') byKey[key] = { state: 'failed', message: item.error ?? '' };
      // Done stays shown until the queue is through and the extensions have loaded.
      else if (item.state === 'done' && (justDone.has(key) || queue.running))
        byKey[key] = { state: 'done' };
    }
    return byKey;
  }, [queue, justDone]);

  const answer = (promise: Promise<ExtensionInstallStatus>) =>
    void promise.then(receive).catch(() => undefined);

  return {
    queue,
    status,
    /** One, a selection, every update: queued in Rust, installed one at a time. */
    install: (entries: CatalogEntry[]) => {
      if (entries.length === 0) return;
      answer(
        commands.extensionInstallsEnqueue(
          entries.map((entry) => ({
            repoId: entry.repoId,
            extensionId: entry.id,
            name: entry.name,
          })),
        ),
      );
    },
    cancel: (entry: { repoId: string; id: string }) =>
      answer(commands.extensionInstallsCancel(entry.repoId, entry.id)),
    cancelAll: () => answer(commands.extensionInstallsCancelAll()),
    retry: () => answer(commands.extensionInstallsRetry()),
    dismiss: () => answer(commands.extensionInstallsDismiss(null, null)),
  };
};

/** Clears the queue between tests. */
export const resetInstallQueue = () => {
  if (follower) void follower.unlisten.then((unlisten) => unlisten?.());
  follower = null;
  for (const timer of timers) clearTimeout(timer);
  timers.clear();
  useQueueStore.setState({ queue: EMPTY_INSTALLS, justDone: new Set() });
};
