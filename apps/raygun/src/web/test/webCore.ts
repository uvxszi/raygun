// Test helpers for Raygun on the web: a web core over memory storage behind the IPC stand-in, so
// tests call the real generated bindings and the page's code paths end to end.
import { createWebCore } from '../core';
import type { WebCore, WebCoreOptions } from '../core';
import type { CommandAnswers } from '../core/fallback';
import { TABLES, memoryBackend } from '../core/store';
import type { Change, TableName } from '../core/store';
import { installWebIpc } from '../ipc';
import type { WebIpc } from '../ipc';

/** A few empty answers, as web-build/commandAnswers makes them from the bindings. */
export const TEST_ANSWERS: CommandAnswers = {
  library_status: { result: true, empty: { roots: [], trackCount: 0, scanning: false } },
  extensions_list: { result: true, empty: [] },
  podcast_set_subscribed: { result: true, empty: null },
  tts_set_available: { result: false, empty: null },
};

export type Tables = Partial<Record<TableName, Array<[string, unknown]>>>;

/** What a backend would hold after `changes`, as a reload reads it. */
export const tablesAfter = (changes: Change[], before: Tables = {}): Tables => {
  const maps = new Map<TableName, Map<string, unknown>>();
  for (const table of TABLES) maps.set(table, new Map(before[table] ?? []));
  for (const change of changes) {
    const map = maps.get(change.table);
    if (!map) continue;
    if (change.value === undefined) map.delete(change.key);
    else map.set(change.key, structuredClone(change.value));
  }
  const tables: Tables = {};
  for (const [table, map] of maps) tables[table] = [...map.entries()];
  return tables;
};

export type TestCore = {
  core: WebCore;
  ipc: WebIpc;
  /** Everything written so far, as a reload would find it. */
  saved: () => Promise<Tables>;
};

/** A web core with memory storage (`tables` from an earlier run), the IPC stand-in installed. */
export const openCore = async (
  tables: Tables = {},
  options: Partial<WebCoreOptions> = {},
): Promise<TestCore> => {
  const backend = memoryBackend(tables);
  // A clock that always moves on, so orders by time are the same on every run.
  let time = 1_800_000_000_000;
  const core = await createWebCore({
    backend,
    now: () => (time += 10),
    answers: TEST_ANSWERS,
    version: '9.9.9',
    youTubeEmbed: 'https://raygun.test/embed/youtube.html',
    ...options,
  });
  const ipc = installWebIpc(core.invoke);
  return {
    core,
    ipc,
    saved: async () => {
      await core.flush();
      return tablesAfter(backend.written, tables);
    },
  };
};
