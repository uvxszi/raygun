import { commands, events } from '@raygun/bindings';
import type {
  DetectedKind,
  SourceQueueAdded,
  SourceQueueItem,
  SourceQueueState,
  SourceQueueStatus,
} from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { create } from 'zustand';

export const EMPTY_SOURCE_LIST: SourceQueueStatus = {
  batch: 0,
  items: [],
  total: 0,
  added: 0,
  alreadyAdded: 0,
  needsSignIn: 0,
  needsSetup: 0,
  failed: 0,
  cancelled: 0,
  running: false,
  paused: false,
  loading: false,
  current: null,
};

/** `lines` after what a list's field holds, each on its own line, the caret's line left empty. */
export const withLines = (text: string, lines: string[]): string => {
  if (lines.length === 0) return text;
  const before = text === '' || text.endsWith('\n') ? text : `${text}\n`;
  return `${before}${lines.join('\n')}\n`;
};

/** Waiting, being looked at or being added. */
export const isBusy = (state: SourceQueueState) =>
  state === 'waiting' || state === 'checking' || state === 'adding';

/** What each kind added changes: the lists read again at once. */
const ADDED_NOW: Record<DetectedKind, string[][]> = {
  addon: [['stremio']],
  repository: [['extensionRepos'], ['extensionSuggestedRepos']],
  extension: [],
  server: [['servers']],
};

const useListStore = create<{ queue: SourceQueueStatus }>(() => ({ queue: EMPTY_SOURCE_LIST }));

let follower: { client: QueryClient; unlisten: Promise<() => void> } | null = null;
/** Repositories or extensions were added: the catalogue and the installed ones are read again once the list is through. */
let catalogueChanged = false;

/**
 * Rust's queue as it is now. What was just added shows at once (addons, repositories); the
 * extension catalogue and the installed extensions are read again once the list is through, as
 * Rust loads what it installed then.
 */
const receive = (queue: SourceQueueStatus) => {
  const before = new Map(useListStore.getState().queue.items.map((item) => [item.id, item.state]));
  useListStore.setState({ queue });
  const client = follower?.client;
  if (!client) return;
  const fresh = queue.items.filter(
    (item: SourceQueueItem) => item.state === 'added' && before.get(item.id) !== 'added',
  );
  const keys = new Set<string>();
  for (const item of fresh) {
    if (!item.kind) continue;
    for (const key of ADDED_NOW[item.kind]) keys.add(JSON.stringify(key));
    if (item.kind === 'repository' || item.kind === 'extension') catalogueChanged = true;
  }
  for (const key of keys) void client.invalidateQueries({ queryKey: JSON.parse(key) as string[] });
  if (!queue.running && catalogueChanged) {
    catalogueChanged = false;
    void client.invalidateQueries({ queryKey: ['extensionCatalog'] });
    void client.invalidateQueries({ queryKey: ['extensions'] });
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
    unlisten: events.sourceQueueEvent.listen((event) => receive(event.payload)),
  };
  void Promise.resolve(commands.sourcesQueueStatus())
    .then((queue) => {
      if (queue) receive(queue);
    })
    .catch(() => undefined);
};

const answer = (promise: Promise<SourceQueueStatus>) =>
  void promise.then(receive).catch(() => undefined);

/** The queue's actions: the same functions every time, so rows given them aren't drawn again. */
const ACTIONS = {
  /** Queues a list, one address a line; answers how many joined it. */
  add: async (text: string): Promise<SourceQueueAdded> => {
    const added = await commands.sourcesQueueAdd(text);
    receive(added.status);
    return added;
  },
  /** One address waiting or being looked at, or every one. */
  cancel: (id: number | null = null) => answer(commands.sourcesQueueCancel(id)),
  retry: () => answer(commands.sourcesQueueRetry()),
  /** Forgets one address that ended, or every one. */
  dismiss: (id: number | null = null) => answer(commands.sourcesQueueDismiss(id)),
  /** A server of the list was connected. */
  signedIn: (id: number) => answer(commands.sourcesQueueSignedIn(id)),
};

/** Sources → Add several: the addresses queued in Rust, and ways to add, cancel and retry them. */
export const useSourceQueue = () => {
  const queryClient = useQueryClient();
  useEffect(() => follow(queryClient), [queryClient]);
  const queue = useListStore((state) => state.queue);
  return { queue, ...ACTIONS };
};

/** Clears the queue between tests. */
export const resetSourceQueue = () => {
  if (follower) void follower.unlisten.then((unlisten) => unlisten?.());
  follower = null;
  catalogueChanged = false;
  useListStore.setState({ queue: EMPTY_SOURCE_LIST });
};
