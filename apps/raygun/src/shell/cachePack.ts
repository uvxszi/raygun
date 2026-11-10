import type { DehydratedState } from '@tanstack/react-query';

/**
 * The saved query cache as gzipped JSON (shell/queryPersist). IndexedDB keeps values in the
 * browser's own format, which Chromium only compresses lightly; the answers are mostly text and
 * take a fifth of that as gzip.
 *
 * JSON alone would lose some of what answers hold (a Map of tags by song, a Date), so such values
 * are tagged as they're written and put back as they were when read. Only the queries that hold
 * tags are walked when read, so a start doesn't pay for a full revival of every answer.
 */

/** The key of an object that stands for a value JSON has no form for. */
const TAG = '~raygun';

type TagKind = 'map' | 'set' | 'date' | 'number' | 'undefined';

/** A query that can't be written as JSON (an object of a class, binary data, a function). */
class Unpackable extends Error {}

const plainObject = (value: object) => {
  const prototype: unknown = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
};

const hasTag = (value: object) => Object.prototype.hasOwnProperty.call(value, TAG);

/** JSON for `value`, with Maps, Sets, Dates, non-finite numbers and holes in arrays tagged. */
const stringify = (value: unknown): { json: string; tagged: boolean } => {
  let tagged = false;
  const tag = (kind: TagKind, inner?: unknown) => {
    tagged = true;
    return inner === undefined ? { [TAG]: kind } : { [TAG]: kind, v: inner };
  };
  function replace(this: unknown, key: string, written: unknown): unknown {
    // The value before its own `toJSON` (a Date's) ran.
    const original = (this as Record<string, unknown>)[key];
    if (original instanceof Date) return tag('date', original.getTime());
    switch (typeof original) {
      case 'undefined':
        // Kept in arrays, where JSON would write null; left out of objects, as JSON does.
        return Array.isArray(this) ? tag('undefined') : undefined;
      case 'number':
        return Number.isFinite(original) ? original : tag('number', String(original));
      case 'bigint':
      case 'function':
      case 'symbol':
        throw new Unpackable();
      case 'object': {
        if (original === null || Array.isArray(original)) return written;
        if (original instanceof Map) return tag('map', [...original]);
        if (original instanceof Set) return tag('set', [...original]);
        if (!plainObject(original) || hasTag(original)) throw new Unpackable();
        return written;
      }
      default:
        return written;
    }
  }
  const json = JSON.stringify(value, replace);
  return { json, tagged };
};

/** Puts back what [stringify] tagged, in place. */
const revive = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index++) value[index] = revive(value[index]);
    return value;
  }
  if (value === null || typeof value !== 'object') return value;
  const record = value as Record<string, unknown>;
  if (hasTag(record)) {
    const inner = record.v;
    switch (record[TAG] as TagKind) {
      case 'map':
        return new Map(
          (inner as [unknown, unknown][]).map(([key, item]) => [revive(key), revive(item)]),
        );
      case 'set':
        return new Set((inner as unknown[]).map(revive));
      case 'date':
        return new Date(inner as number);
      case 'number':
        return Number(inner);
      case 'undefined':
        return undefined;
    }
  }
  for (const key of Object.keys(record)) record[key] = revive(record[key]);
  return record;
};

/**
 * `state` as JSON, and the hashes of the queries in it that hold tagged values. Queries that
 * can't be written as JSON are left out (they're asked again).
 */
export const packQueries = (state: DehydratedState): { json: string; tagged: string[] } => {
  const queries: string[] = [];
  const tagged: string[] = [];
  for (const query of state.queries) {
    try {
      const packed = stringify(query);
      queries.push(packed.json);
      if (packed.tagged) tagged.push(query.queryHash);
    } catch {
      // Left out.
    }
  }
  let mutations = '[]';
  try {
    mutations = stringify(state.mutations).json;
  } catch {
    // None kept.
  }
  return { json: `{"mutations":${mutations},"queries":[${queries.join(',')}]}`, tagged };
};

/** The state [packQueries] wrote, with the tagged queries' values put back. */
export const unpackQueries = (json: string, tagged: readonly string[]): DehydratedState => {
  const state = JSON.parse(json) as DehydratedState;
  if (tagged.length > 0) {
    const marked = new Set(tagged);
    for (const query of state.queries) {
      if (marked.has(query.queryHash)) revive(query);
    }
  }
  return state;
};

/** Whether this browser compresses (CompressionStream; Chromium 80 and later). */
export const canCompress = () =>
  typeof CompressionStream === 'function' && typeof DecompressionStream === 'function';

/** Runs `bytes` through `transform`, whole. */
const transformed = async (
  bytes: Uint8Array<ArrayBuffer>,
  transform: CompressionStream | DecompressionStream,
): Promise<Uint8Array<ArrayBuffer>> => {
  const writer = transform.writable.getWriter();
  const written = writer.write(bytes).then(() => writer.close());
  // A failure shows when reading; this one mustn't go unhandled meanwhile.
  written.catch(() => {});
  const reader = transform.readable.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    length += value.byteLength;
  }
  await written;
  const out = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return out;
};

/** `text` gzipped. */
export const gzipText = (text: string) =>
  transformed(new TextEncoder().encode(text), new CompressionStream('gzip'));

/** The text in gzipped `bytes`; fails on anything else. */
export const gunzipText = async (bytes: Uint8Array<ArrayBuffer>) =>
  new TextDecoder().decode(await transformed(bytes, new DecompressionStream('gzip')));
