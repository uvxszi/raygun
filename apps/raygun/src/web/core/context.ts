// What every part of the web core shares: the tables, the clock, this browser's device id, and
// the way to tell the page that something changed (Rust's events).
import { events } from '@raygun/bindings';

import { Clock, uuidV7 } from './clock';
import type { Store, TableName } from './store';

type Events = typeof events;
export type EventName = keyof Events;
export type EventPayload<K extends EventName> = Parameters<
  Parameters<Events[K]['listen']>[0]
>[0]['payload'];

/** A command's error, worded for people (Rust answers its errors as strings too). */
export class CommandError extends Error {}

/** Fails the running command with `message`. */
export const fail = (message: string): never => {
  throw new CommandError(message);
};

export type CoreContext = {
  store: Store;
  clock: Clock;
  deviceId: string;
  now: () => number;
  /** Tells the page, as Rust's events do. */
  emit: <K extends EventName>(name: K, payload: EventPayload<K>) => void;
  /** A device-local value (Rust's `local_meta`). */
  meta: <T>(key: string) => T | undefined;
  setMeta: (key: string, value: unknown) => void;
  /** Records the clock's latest stamp so it resumes past it next time. */
  persistClock: () => void;
};

/** A new row id, from the core's clock. */
export const uuidV7Of = (context: { now: () => number }) => uuidV7(context.now());

const DEVICE_ID = 'deviceId';
const HLC_LAST = 'hlcLast';

export const createContext = (store: Store, now: () => number = Date.now): CoreContext => {
  const meta = <T>(key: string) => store.get<T>('meta', key);
  const setMeta = (key: string, value: unknown) => store.put('meta', key, value);
  let deviceId = meta<string>(DEVICE_ID);
  if (!deviceId) {
    deviceId = uuidV7(now());
    setMeta(DEVICE_ID, deviceId);
  }
  const stored = meta<string>(HLC_LAST);
  const clock = new Clock(stored ? BigInt(stored) : 0n, now);
  return {
    store,
    clock,
    deviceId,
    now,
    emit: (name, payload) => {
      const event = events[name] as unknown as { emit: (payload: unknown) => Promise<void> };
      void event.emit(payload).catch(() => undefined);
    },
    meta,
    setMeta,
    persistClock: () => setMeta(HLC_LAST, clock.latest.toString()),
  };
};

// ---- Synced tables (raygun-db::sync::TABLES) ----

/** The synced tables and their key columns, as Rust's `TableSpec`s. */
export const SYNCED_TABLES = {
  setting: 'key',
  music_favorite: 'id',
  music_playlist: 'id',
  music_playlist_item: 'id',
  music_play_history: 'id',
} as const satisfies Partial<Record<TableName, string>>;

export type SyncedTable = keyof typeof SYNCED_TABLES;

/** The columns every synced row has besides its own. */
export type Stamp = { updated_at_hlc: bigint; origin_device: string; deleted: number };

export type SyncedRow = Record<string, unknown> & Stamp;

/** One change log entry: the latest change of a row (Rust's `local_sync_log`). */
export type LogEntry = { seq: number; tbl: SyncedTable; key: string; fromPeer: string | null };

const LOG_SEQ = 'syncSeq';

export const keyOf = (table: SyncedTable, row: Record<string, unknown>) =>
  String(row[SYNCED_TABLES[table]] ?? '');

const logKey = (table: SyncedTable, key: string) => `${table}\u0000${key}`;

/** Synced rows: local writes stamp them and log them for sync, as Rust's triggers do. */
export class SyncedRows {
  constructor(private readonly context: CoreContext) {}

  get<T extends SyncedRow>(table: SyncedTable, key: string): T | undefined {
    return this.context.store.get<T>(table, key);
  }

  /** Every row, tombstones too. */
  all<T extends SyncedRow>(table: SyncedTable): T[] {
    return this.context.store.all<T>(table);
  }

  live<T extends SyncedRow>(table: SyncedTable): T[] {
    return this.all<T>(table).filter((row) => row.deleted === 0);
  }

  /** A stamp for one local change (several rows written together share it). */
  stamp(): bigint {
    const stamp = this.context.clock.tick();
    this.context.persistClock();
    return stamp;
  }

  /** A local write, stamped here. */
  write<T extends Record<string, unknown>>(table: SyncedTable, row: T, stamp = this.stamp()) {
    const stamped = { ...row, updated_at_hlc: stamp, origin_device: this.context.deviceId };
    this.save(table, stamped as unknown as SyncedRow, null);
    return stamped;
  }

  /** Saves a row as it is; `fromPeer` names the device it came from, so it isn't sent back. */
  save(table: SyncedTable, row: SyncedRow, fromPeer: string | null) {
    const key = keyOf(table, row);
    this.context.store.put(table, key, row);
    const seq = (this.context.meta<number>(LOG_SEQ) ?? 0) + 1;
    this.context.setMeta(LOG_SEQ, seq);
    this.context.store.put('sync_log', logKey(table, key), { seq, tbl: table, key, fromPeer });
  }

  /** The change log after `afterSeq`, oldest first. */
  changesSince(afterSeq: number): LogEntry[] {
    return this.context.store
      .all<LogEntry>('sync_log')
      .filter((entry) => entry.seq > afterSeq)
      .sort((a, b) => a.seq - b.seq);
  }

  latestSeq(): number {
    return this.context.meta<number>(LOG_SEQ) ?? 0;
  }
}
