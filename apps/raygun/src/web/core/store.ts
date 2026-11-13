// Raygun on the web keeps its data in the browser's IndexedDB. Everything is read into memory
// when the page opens (a library is a few thousand rows at most) and each change is written
// through, so commands answer at once and the data survives reloads.

/** The tables kept. The synced ones have the same names and columns as Rust's (raygun-db). */
export const TABLES = [
  'meta',
  'setting',
  'music_favorite',
  'music_playlist',
  'music_playlist_item',
  'music_play_history',
  'sync_log',
  'sync_peer',
  'sync_session',
  'sync_undo',
  'files',
  'extensions',
  'ext_storage',
] as const;

export type TableName = (typeof TABLES)[number];

export type Change = { table: TableName; key: string; value: unknown | undefined };

/** Where the tables live between visits. */
export type Backend = {
  load: () => Promise<Partial<Record<TableName, Array<[string, unknown]>>>>;
  write: (changes: Change[]) => Promise<void>;
};

/** Tables in memory, written through to a backend. */
export class Store {
  private readonly tables = new Map<TableName, Map<string, unknown>>();
  private pending: Change[] = [];
  private flushing: Promise<void> = Promise.resolve();
  private scheduled = false;

  private constructor(private readonly backend: Backend) {
    for (const table of TABLES) this.tables.set(table, new Map());
  }

  static async open(backend: Backend): Promise<Store> {
    const store = new Store(backend);
    const loaded = await backend.load();
    for (const table of TABLES) {
      const rows = loaded[table];
      if (!rows) continue;
      const map = store.table(table);
      for (const [key, value] of rows) map.set(key, value);
    }
    return store;
  }

  private table(table: TableName) {
    const map = this.tables.get(table);
    if (!map) throw new Error(`no table ${table}`);
    return map;
  }

  get<T>(table: TableName, key: string): T | undefined {
    return this.table(table).get(key) as T | undefined;
  }

  all<T>(table: TableName): T[] {
    return [...this.table(table).values()] as T[];
  }

  entries<T>(table: TableName): Array<[string, T]> {
    return [...this.table(table).entries()] as Array<[string, T]>;
  }

  put(table: TableName, key: string, value: unknown) {
    this.table(table).set(key, value);
    this.queue({ table, key, value });
  }

  remove(table: TableName, key: string) {
    if (!this.table(table).delete(key)) return;
    this.queue({ table, key, value: undefined });
  }

  clear(table: TableName) {
    for (const key of [...this.table(table).keys()]) this.remove(table, key);
  }

  /** Resolves once everything changed so far is written. */
  flush(): Promise<void> {
    this.scheduled = false;
    const changes = this.pending;
    this.pending = [];
    if (changes.length > 0) {
      this.flushing = this.flushing
        .then(() => this.backend.write(changes))
        .catch((error) => console.error('[raygun web] saving failed', error));
    }
    return this.flushing;
  }

  private queue(change: Change) {
    this.pending.push(change);
    if (this.scheduled) return;
    this.scheduled = true;
    queueMicrotask(() => void this.flush());
  }
}

/** Keeps nothing (tests, and browsers that refuse storage). */
export const memoryBackend = (
  initial: Partial<Record<TableName, Array<[string, unknown]>>> = {},
): Backend & { written: Change[] } => {
  const written: Change[] = [];
  return {
    written,
    load: async () => initial,
    write: async (changes) => {
      written.push(...changes);
    },
  };
};

/** Apart from the page's saved query cache (`raygun-cache`, shell/queryPersist.ts). */
export const DB_NAME = 'raygun-web';

const request = <T>(asked: IDBRequest<T>) =>
  new Promise<T>((resolve, reject) => {
    asked.onsuccess = () => resolve(asked.result);
    asked.onerror = () => reject(asked.error);
  });

/**
 * The database with a store for every table: a version that lacks some (an older build's) is
 * upgraded by one, adding them. Another tab holding the old version lets go of it.
 */
const openDatabase = async (): Promise<IDBDatabase> => {
  const current = await request(indexedDB.open(DB_NAME));
  if (TABLES.every((table) => current.objectStoreNames.contains(table))) {
    current.onversionchange = () => current.close();
    return current;
  }
  const version = current.version + 1;
  current.close();
  const opening = indexedDB.open(DB_NAME, version);
  opening.onupgradeneeded = () => {
    const db = opening.result;
    for (const table of TABLES) {
      if (!db.objectStoreNames.contains(table)) db.createObjectStore(table);
    }
  };
  const db = await request(opening);
  db.onversionchange = () => db.close();
  return db;
};

/** IndexedDB, one object store per table with out-of-line keys. */
export const indexedDbBackend = async (): Promise<Backend> => {
  const db = await openDatabase();
  return {
    load: async () => {
      const transaction = db.transaction([...TABLES], 'readonly');
      const loaded: Partial<Record<TableName, Array<[string, unknown]>>> = {};
      await Promise.all(
        TABLES.map(async (table) => {
          const store = transaction.objectStore(table);
          const [keys, values] = await Promise.all([
            request(store.getAllKeys()),
            request(store.getAll()),
          ]);
          loaded[table] = keys.map((key, index) => [String(key), values[index]]);
        }),
      );
      return loaded;
    },
    write: (changes) =>
      new Promise<void>((resolve, reject) => {
        const tables = [...new Set(changes.map((change) => change.table))];
        const transaction = db.transaction(tables, 'readwrite');
        for (const change of changes) {
          const store = transaction.objectStore(change.table);
          if (change.value === undefined) store.delete(change.key);
          else store.put(change.value, change.key);
        }
        transaction.oncomplete = () => resolve();
        transaction.onerror = () => reject(transaction.error);
        transaction.onabort = () => reject(transaction.error);
      }),
  };
};
