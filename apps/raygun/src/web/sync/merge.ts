// What a sync sends and how rows from another device merge (raygun-db sync.rs): last writer wins
// per row, ordered by `(updated_at_hlc, origin_device)`; two devices that favorite one song settle
// on the smaller id.
import type { SyncKind } from '@raygun/bindings';

import { SYNCED_TABLES, keyOf } from '../core/context';
import type { CoreContext, SyncedRow, SyncedRows, SyncedTable } from '../core/context';
import type { WireRow } from './protocol';

/** Setting keys each device keeps for itself (`DEVICE_ONLY_SETTINGS`). */
export const DEVICE_ONLY_SETTINGS = new Set([
  'manualBrowserChecks',
  'runSourceCode',
  'fonts',
  'checkUpdates',
  'network',
  'torrents',
  'readAloud',
]);

const STAMP = ['updated_at_hlc', 'origin_device', 'deleted'];
const TRACK = ['track_key', 'title', 'artist', 'album', 'duration_ms', 'artwork_url', 'source'];

/** Every column that travels, per table (`TABLES` in Rust). */
export const COLUMNS: Record<SyncedTable, string[]> = {
  setting: ['key', 'value', ...STAMP],
  music_favorite: ['id', ...TRACK, 'added_at', 'position', ...STAMP],
  music_playlist: ['id', 'name', 'created_at', ...STAMP],
  music_playlist_item: ['id', 'playlist_id', 'position', ...TRACK, 'added_at', ...STAMP],
  music_play_history: ['id', ...TRACK, 'played_at', ...STAMP],
};

const KIND: Record<SyncedTable, SyncKind> = {
  setting: 'settings',
  music_favorite: 'library',
  music_playlist: 'library',
  music_playlist_item: 'library',
  music_play_history: 'history',
};

/** Columns SQLite keeps as INTEGER in STRICT tables: a fraction there would be refused. */
const INTEGERS = new Set(['duration_ms', 'added_at', 'played_at', 'created_at', 'deleted']);

const isSynced = (table: string): table is SyncedTable => table in SYNCED_TABLES;

const toBigInt = (value: unknown): bigint => {
  if (typeof value === 'bigint') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return BigInt(Math.trunc(value));
  if (typeof value === 'string' && /^-?\d+$/.test(value)) return BigInt(value);
  return 0n;
};

/** A row in its table's columns only, integers whole, as both sides store it. */
export const projectRow = (table: SyncedTable, row: Record<string, unknown>): SyncedRow => {
  const out: Record<string, unknown> = {};
  for (const column of COLUMNS[table]) {
    let value = row[column] ?? null;
    if (column === 'updated_at_hlc') value = toBigInt(value);
    else if (INTEGERS.has(column) && typeof value === 'number') value = Math.round(value);
    out[column] = value;
  }
  out.deleted = Number(out.deleted ?? 0) === 0 ? 0 : 1;
  return out as SyncedRow;
};

const stampOf = (row: Record<string, unknown>): [bigint, string] => [
  toBigInt(row.updated_at_hlc),
  typeof row.origin_device === 'string' ? row.origin_device : '',
];

/** Whether `incoming` was written after `existing`. */
export const isNewer = (incoming: Record<string, unknown>, existing: Record<string, unknown>) => {
  const [incomingHlc, incomingDevice] = stampOf(incoming);
  const [existingHlc, existingDevice] = stampOf(existing);
  if (incomingHlc !== existingHlc) return incomingHlc > existingHlc;
  return incomingDevice > existingDevice;
};

const sameRow = (a: Record<string, unknown>, b: Record<string, unknown>) =>
  Object.keys(a).length === Object.keys(b).length &&
  Object.entries(a).every(([key, value]) => b[key] === value);

/**
 * Rows changed after `afterSeq` for a peer, oldest first, and the last log seq looked at: rows of
 * kinds not shared, rows that came from that peer and device-only settings stay out.
 */
export const changesFor = (
  rows: SyncedRows,
  afterSeq: number,
  kinds: SyncKind[],
  peerId: string,
): { rows: WireRow[]; last: number } => {
  let last = afterSeq;
  const out: WireRow[] = [];
  for (const entry of rows.changesSince(afterSeq)) {
    last = entry.seq;
    if (entry.fromPeer !== null && entry.fromPeer === peerId) continue;
    if (!isSynced(entry.tbl) || !kinds.includes(KIND[entry.tbl])) continue;
    if (entry.tbl === 'setting' && DEVICE_ONLY_SETTINGS.has(entry.key)) continue;
    const row = rows.get(entry.tbl, entry.key);
    if (row) out.push({ table: entry.tbl, row: projectRow(entry.tbl, row) });
  }
  return { rows: out, last };
};

export type Applied = { received: number; changed: number };

/** Merges rows from `fromPeer`; a row replaces ours only when it was written later. */
export const applyRows = (
  context: CoreContext,
  rows: SyncedRows,
  incoming: WireRow[],
  kinds: SyncKind[],
  fromPeer: string,
): Applied => {
  const applied: Applied = { received: 0, changed: 0 };
  let newest = 0n;
  for (const wire of incoming) {
    if (!isSynced(wire.table) || !kinds.includes(KIND[wire.table])) continue;
    const table = wire.table;
    const arrived = projectRow(table, wire.row);
    const key = keyOf(table, arrived);
    if (!key) continue;
    if (table === 'setting' && DEVICE_ONLY_SETTINGS.has(key)) continue;
    applied.received += 1;
    const [hlc] = stampOf(arrived);
    if (hlc > newest) newest = hlc;
    const existing = rows.get(table, key);
    if (existing && !isNewer(arrived, existing)) continue;
    let row = arrived;
    if (table === 'music_favorite' && row.deleted === 0)
      row = settleFavorite(context, rows, row, key);
    // Taken as it came: the sender has it already. A row changed here goes back.
    rows.save(table, row, sameRow(row, arrived) ? fromPeer : null);
    applied.changed += 1;
  }
  if (newest > 0n) {
    context.clock.observe(newest);
    context.persistClock();
  }
  return applied;
};

/** Two live favorites of one song: the smaller id stays, the other becomes a tombstone here. */
const settleFavorite = (
  context: CoreContext,
  rows: SyncedRows,
  row: SyncedRow,
  key: string,
): SyncedRow => {
  const trackKey = row.track_key;
  const other = rows
    .live('music_favorite')
    .find((candidate) => candidate.track_key === trackKey && candidate.id !== key);
  if (!other) return row;
  const stamp = rows.stamp();
  const otherId = String(other.id);
  if (key < otherId) {
    rows.write('music_favorite', { ...other, deleted: 1 }, stamp);
    return row;
  }
  return { ...row, deleted: 1, updated_at_hlc: stamp, origin_device: context.deviceId };
};
