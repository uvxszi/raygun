// Web APIs Nuclear plugins use that QuickJS lacks: URL, URLSearchParams, Headers, base64,
// TextEncoder/TextDecoder, timers, and the Blob → DecompressionStream → Response pipeline some
// plugins use to inflate data. Small and pragmatic: enough for real plugins, not full specs.
//
// Memory matters here: a plugin's runtime has a fixed heap, and a scraped page is megabytes. Text
// stays a string (never an array of numbers, which costs 16 bytes a byte in QuickJS), bytes stay
// typed arrays, and either becomes the other only when a plugin asks.
import { gunzipSync, inflateSync, unzlibSync } from 'fflate';

// ---- Text ----

const REPLACEMENT = 0xfffd;

export class TextEncoderPolyfill {
  readonly encoding = 'utf-8';

  encode(text = ''): Uint8Array {
    const input = String(text);
    // At most three bytes per UTF-16 unit (a pair's four bytes cover its two units).
    const out = new Uint8Array(input.length * 3);
    let length = 0;
    for (let index = 0; index < input.length; index += 1) {
      let code = input.charCodeAt(index);
      if (code >= 0xd800 && code <= 0xdbff && index + 1 < input.length) {
        const next = input.charCodeAt(index + 1);
        if (next >= 0xdc00 && next <= 0xdfff) {
          code = 0x10000 + ((code - 0xd800) << 10) + (next - 0xdc00);
          index += 1;
        }
      }
      if (code >= 0xd800 && code <= 0xdfff) code = REPLACEMENT;
      if (code < 0x80) {
        out[length++] = code;
      } else if (code < 0x800) {
        out[length++] = 0xc0 | (code >> 6);
        out[length++] = 0x80 | (code & 0x3f);
      } else if (code < 0x10000) {
        out[length++] = 0xe0 | (code >> 12);
        out[length++] = 0x80 | ((code >> 6) & 0x3f);
        out[length++] = 0x80 | (code & 0x3f);
      } else {
        out[length++] = 0xf0 | (code >> 18);
        out[length++] = 0x80 | ((code >> 12) & 0x3f);
        out[length++] = 0x80 | ((code >> 6) & 0x3f);
        out[length++] = 0x80 | (code & 0x3f);
      }
    }
    return out.slice(0, length);
  }
}

/** UTF-16 units gathered before they become a piece of the decoded text. */
const DECODE_CHUNK = 8192;

/** A UTF-8 continuation byte (10xxxxxx). */
const continues = (byte: number | undefined) => byte !== undefined && (byte & 0xc0) === 0x80;

export class TextDecoderPolyfill {
  readonly encoding = 'utf-8';
  readonly fatal: boolean;

  constructor(label = 'utf-8', options: { fatal?: boolean } = {}) {
    // Other encodings are refused as browsers refuse unknown ones, not read as UTF-8 by mistake.
    if (!['utf-8', 'utf8', 'unicode-1-1-utf-8'].includes(String(label).trim().toLowerCase())) {
      throw new RangeError(`Raygun's TextDecoder reads UTF-8, not ${String(label)}`);
    }
    this.fatal = options.fatal === true;
  }

  decode(input?: ArrayBuffer | ArrayBufferView): string {
    if (!input) return '';
    const bytes =
      input instanceof ArrayBuffer
        ? new Uint8Array(input)
        : new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    const pieces: string[] = [];
    const units: number[] = [];
    const flush = () => {
      pieces.push(String.fromCharCode.apply(null, units));
      units.length = 0;
    };
    let index = 0;
    // A byte order mark isn't part of the text.
    if (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) index = 3;
    while (index < bytes.length) {
      const first = bytes[index] as number;
      // A byte that starts no valid sequence stands for itself as U+FFFD.
      let code = -1;
      let size = 1;
      if (first < 0x80) {
        code = first;
      } else if (first >= 0xc2 && first <= 0xdf && continues(bytes[index + 1])) {
        code = ((first & 0x1f) << 6) | ((bytes[index + 1] as number) & 0x3f);
        size = 2;
      } else if (
        first >= 0xe0 &&
        first <= 0xef &&
        continues(bytes[index + 1]) &&
        continues(bytes[index + 2])
      ) {
        code =
          ((first & 0x0f) << 12) |
          (((bytes[index + 1] as number) & 0x3f) << 6) |
          ((bytes[index + 2] as number) & 0x3f);
        size = 3;
        if (code < 0x800 || (code >= 0xd800 && code <= 0xdfff)) code = -1;
      } else if (
        first >= 0xf0 &&
        first <= 0xf4 &&
        continues(bytes[index + 1]) &&
        continues(bytes[index + 2]) &&
        continues(bytes[index + 3])
      ) {
        code =
          ((first & 0x07) << 18) |
          (((bytes[index + 1] as number) & 0x3f) << 12) |
          (((bytes[index + 2] as number) & 0x3f) << 6) |
          ((bytes[index + 3] as number) & 0x3f);
        size = 4;
        if (code < 0x10000 || code > 0x10ffff) code = -1;
      }
      if (code < 0) {
        if (this.fatal) throw new TypeError('The encoded data was not valid UTF-8');
        code = REPLACEMENT;
        size = 1;
      }
      index += size;
      if (code >= 0x10000) {
        code -= 0x10000;
        units.push(0xd800 + (code >> 10), 0xdc00 + (code & 0x3ff));
      } else {
        units.push(code);
      }
      if (units.length >= DECODE_CHUNK) flush();
    }
    flush();
    return pieces.length === 1 ? (pieces[0] as string) : pieces.join('');
  }
}

// ---- Base64 ----

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

/** Bytes as base64 (what `raygun.http.fetch` takes as `bodyBase64`). */
export const bytesToBase64 = (bytes: Uint8Array): string => {
  const pieces: string[] = [];
  let piece = '';
  for (let index = 0; index < bytes.length; index += 3) {
    const a = bytes[index] as number;
    const b = bytes[index + 1];
    const c = bytes[index + 2];
    const triple = (a << 16) | ((b ?? 0) << 8) | (c ?? 0);
    piece += BASE64[(triple >> 18) & 63];
    piece += BASE64[(triple >> 12) & 63];
    piece += b === undefined ? '=' : BASE64[(triple >> 6) & 63];
    piece += c === undefined ? '=' : BASE64[triple & 63];
    if (piece.length >= DECODE_CHUNK) {
      pieces.push(piece);
      piece = '';
    }
  }
  pieces.push(piece);
  return pieces.join('');
};

/** Base64 as bytes. */
export const base64ToBytes = (base64: string): Uint8Array => {
  const clean = base64.replace(/[^A-Za-z0-9+/]/g, '');
  const out = new Uint8Array(Math.floor((clean.length * 3) / 4));
  let length = 0;
  for (let index = 0; index < clean.length; index += 4) {
    const values = [0, 1, 2, 3].map((offset) => BASE64.indexOf(clean[index + offset] ?? 'A'));
    const triple =
      ((values[0] ?? 0) << 18) |
      ((values[1] ?? 0) << 12) |
      ((values[2] ?? 0) << 6) |
      (values[3] ?? 0);
    out[length++] = (triple >> 16) & 255;
    if (index + 2 < clean.length) out[length++] = (triple >> 8) & 255;
    if (index + 3 < clean.length) out[length++] = triple & 255;
  }
  return out.slice(0, length);
};

export const btoaPolyfill = (binary: string): string => {
  let out = '';
  for (let index = 0; index < binary.length; index += 3) {
    const [a, b, c] = [0, 1, 2].map((offset) => binary.charCodeAt(index + offset));
    const triple = ((a ?? 0) << 16) | ((b || 0) << 8) | (c || 0);
    out += BASE64[(triple >> 18) & 63];
    out += BASE64[(triple >> 12) & 63];
    out += index + 1 < binary.length ? BASE64[(triple >> 6) & 63] : '=';
    out += index + 2 < binary.length ? BASE64[triple & 63] : '=';
  }
  return out;
};

export const atobPolyfill = (base64: string): string => {
  const clean = base64.replace(/[^A-Za-z0-9+/]/g, '');
  let out = '';
  for (let index = 0; index < clean.length; index += 4) {
    const values = [0, 1, 2, 3].map((offset) => BASE64.indexOf(clean[index + offset] ?? 'A'));
    const triple =
      ((values[0] ?? 0) << 18) |
      ((values[1] ?? 0) << 12) |
      ((values[2] ?? 0) << 6) |
      (values[3] ?? 0);
    out += String.fromCharCode((triple >> 16) & 255);
    if (index + 2 < clean.length) out += String.fromCharCode((triple >> 8) & 255);
    if (index + 3 < clean.length) out += String.fromCharCode(triple & 255);
  }
  return out;
};

// ---- URLSearchParams and URL ----

const encodeForm = (text: string) =>
  encodeURIComponent(text)
    .replace(/[!'()~]/g, (character) => `%${character.charCodeAt(0).toString(16).toUpperCase()}`)
    .replace(/%20/g, '+');

const decodeForm = (text: string) => {
  try {
    return decodeURIComponent(text.replace(/\+/g, ' '));
  } catch {
    return text;
  }
};

type ParamsInit = string | URLSearchParamsPolyfill | Record<string, string> | [string, string][];

export class URLSearchParamsPolyfill {
  #pairs: [string, string][] = [];
  #onChange?: () => void;

  constructor(init?: ParamsInit, onChange?: () => void) {
    if (typeof init === 'string') {
      this.#parse(init);
    } else if (init instanceof URLSearchParamsPolyfill) {
      this.#pairs = [...init.entries()];
    } else if (Array.isArray(init)) {
      this.#pairs = init.map(([key, value]) => [String(key), String(value)]);
    } else if (init) {
      this.#pairs = Object.entries(init).map(([key, value]) => [key, String(value)]);
    }
    this.#onChange = onChange;
  }

  #parse(text: string) {
    this.#pairs = text
      .replace(/^\?/, '')
      .split('&')
      .filter(Boolean)
      .map((pair) => {
        const [key = '', ...rest] = pair.split('=');
        return [decodeForm(key), decodeForm(rest.join('='))];
      });
  }

  /** Used by URL when its `search` changes. */
  replaceWith(text: string) {
    this.#parse(text);
  }

  #changed() {
    this.#onChange?.();
  }

  append(key: string, value: string) {
    this.#pairs.push([String(key), String(value)]);
    this.#changed();
  }

  /** Replaces the first `key` in place and drops any others. */
  set(key: string, value: string) {
    const index = this.#pairs.findIndex(([name]) => name === key);
    if (index === -1) {
      this.#pairs.push([String(key), String(value)]);
    } else {
      this.#pairs[index] = [String(key), String(value)];
      this.#pairs = this.#pairs.filter(([name], position) => name !== key || position === index);
    }
    this.#changed();
  }

  get(key: string): string | null {
    return this.#pairs.find(([name]) => name === key)?.[1] ?? null;
  }

  getAll(key: string): string[] {
    return this.#pairs.filter(([name]) => name === key).map(([, value]) => value);
  }

  has(key: string): boolean {
    return this.#pairs.some(([name]) => name === key);
  }

  delete(key: string) {
    this.#pairs = this.#pairs.filter(([name]) => name !== key);
    this.#changed();
  }

  forEach(callback: (value: string, key: string) => void) {
    for (const [key, value] of this.#pairs) callback(value, key);
  }

  entries() {
    return this.#pairs.map(([key, value]) => [key, value] as [string, string])[Symbol.iterator]();
  }

  keys() {
    return this.#pairs.map(([key]) => key)[Symbol.iterator]();
  }

  values() {
    return this.#pairs.map(([, value]) => value)[Symbol.iterator]();
  }

  [Symbol.iterator]() {
    return this.entries();
  }

  get size() {
    return this.#pairs.length;
  }

  toString() {
    return this.#pairs.map(([key, value]) => `${encodeForm(key)}=${encodeForm(value)}`).join('&');
  }
}

const ABSOLUTE = /^([a-zA-Z][a-zA-Z0-9+.-]*:)(\/\/([^/?#]*))?([^?#]*)(\?[^#]*)?(#.*)?$/;

const normalizePath = (path: string) => {
  const output: string[] = [];
  for (const segment of path.split('/')) {
    if (segment === '..') output.pop();
    else if (segment !== '.') output.push(segment);
  }
  const joined = output.join('/');
  const trailing = /\/\.\.?$/.test(path) ? '/' : '';
  return (joined.startsWith('/') ? joined : `/${joined}`) + trailing;
};

export class URLPolyfill {
  protocol = '';
  username = '';
  password = '';
  hostname = '';
  port = '';
  pathname = '/';
  hash = '';
  #search = '';
  readonly searchParams: URLSearchParamsPolyfill;

  constructor(input: string | URLPolyfill, base?: string | URLPolyfill) {
    const text = String(input).trim();
    const match = ABSOLUTE.exec(text);
    if (match) {
      this.#assign(match);
    } else {
      if (base === undefined) throw new TypeError(`Invalid URL: ${text}`);
      const root = new URLPolyfill(String(base));
      this.protocol = root.protocol;
      this.username = root.username;
      this.password = root.password;
      this.hostname = root.hostname;
      this.port = root.port;
      if (text.startsWith('//')) {
        this.#assign(ABSOLUTE.exec(`${root.protocol}${text}`) as RegExpExecArray);
      } else if (text.startsWith('/')) {
        this.#setPathQueryHash(text);
      } else if (text.startsWith('?') || text === '') {
        this.pathname = root.pathname;
        this.#setPathQueryHash(`${root.pathname}${text || root.search}`);
      } else if (text.startsWith('#')) {
        this.pathname = root.pathname;
        this.#search = root.search;
        this.hash = text;
      } else {
        const directory = root.pathname.replace(/[^/]*$/, '');
        this.#setPathQueryHash(`${directory}${text}`);
      }
    }
    this.searchParams = new URLSearchParamsPolyfill(this.#search, () => {
      const query = this.searchParams.toString();
      this.#search = query ? `?${query}` : '';
    });
  }

  #assign(match: RegExpExecArray) {
    const [, protocol = '', , authority = '', path = '', search = '', hash = ''] = match;
    this.protocol = protocol.toLowerCase();
    const at = authority.lastIndexOf('@');
    const credentials = at >= 0 ? authority.slice(0, at) : '';
    const hostPort = at >= 0 ? authority.slice(at + 1) : authority;
    [this.username = '', this.password = ''] = credentials ? credentials.split(':') : [];
    const portMatch = /^(.*?)(?::(\d+))?$/.exec(hostPort);
    this.hostname = (portMatch?.[1] ?? '').toLowerCase();
    this.port = portMatch?.[2] ?? '';
    this.pathname = path ? normalizePath(path) : authority ? '/' : '';
    this.#search = search === '?' ? '' : search;
    this.hash = hash === '#' ? '' : hash;
  }

  #setPathQueryHash(text: string) {
    const [, path = '', search = '', hash = ''] = /^([^?#]*)(\?[^#]*)?(#.*)?$/.exec(text) ?? [];
    this.pathname = normalizePath(path);
    this.#search = search === '?' ? '' : search;
    this.hash = hash === '#' ? '' : hash;
  }

  get search() {
    return this.#search;
  }

  set search(value: string) {
    const text = String(value);
    this.#search = text && !text.startsWith('?') ? `?${text}` : text === '?' ? '' : text;
    this.searchParams.replaceWith(this.#search);
  }

  get host() {
    return this.port ? `${this.hostname}:${this.port}` : this.hostname;
  }

  get origin() {
    return `${this.protocol}//${this.host}`;
  }

  get href() {
    const credentials = this.username
      ? `${this.username}${this.password ? `:${this.password}` : ''}@`
      : '';
    return `${this.protocol}//${credentials}${this.host}${this.pathname}${this.#search}${this.hash}`;
  }

  toString() {
    return this.href;
  }

  toJSON() {
    return this.href;
  }
}

// ---- Headers ----

type HeadersInitLike = HeadersPolyfill | Record<string, string> | [string, string][];

export class HeadersPolyfill {
  #map = new Map<string, string>();

  constructor(init?: HeadersInitLike) {
    if (init instanceof HeadersPolyfill) init.forEach((value, key) => this.set(key, value));
    else if (Array.isArray(init)) for (const [key, value] of init) this.append(key, value);
    else if (init) for (const [key, value] of Object.entries(init)) this.set(key, String(value));
  }

  get(name: string) {
    return this.#map.get(name.toLowerCase()) ?? null;
  }

  set(name: string, value: string) {
    this.#map.set(name.toLowerCase(), String(value));
  }

  append(name: string, value: string) {
    const key = name.toLowerCase();
    const existing = this.#map.get(key);
    this.#map.set(key, existing ? `${existing}, ${value}` : String(value));
  }

  has(name: string) {
    return this.#map.has(name.toLowerCase());
  }

  delete(name: string) {
    this.#map.delete(name.toLowerCase());
  }

  forEach(callback: (value: string, key: string) => void) {
    for (const [key, value] of this.#map) callback(value, key);
  }

  entries() {
    return this.#map.entries();
  }

  [Symbol.iterator]() {
    return this.#map.entries();
  }

  toRecord(): Record<string, string> {
    return Object.fromEntries(this.#map);
  }
}

// ---- Bytes: Blob, DecompressionStream, Response ----

/** A finished "stream": every polyfilled stream here is already fully in memory. */
type ByteStream = { readonly bytes: Uint8Array; pipeThrough(transform: ByteTransform): ByteStream };
type ByteTransform = { transform(bytes: Uint8Array): Uint8Array };

const streamOf = (bytes: Uint8Array): ByteStream => ({
  bytes,
  pipeThrough: (transform) => streamOf(transform.transform(bytes)),
});

const toBytes = (part: unknown): Uint8Array => {
  if (typeof part === 'string') return new TextEncoderPolyfill().encode(part);
  if (part instanceof Uint8Array) return part;
  if (part instanceof ArrayBuffer) return new Uint8Array(part);
  if (ArrayBuffer.isView(part))
    return new Uint8Array(part.buffer, part.byteOffset, part.byteLength);
  if (part instanceof BlobPolyfill) return part.bytes;
  return new TextEncoderPolyfill().encode(String(part));
};

const concat = (chunks: Uint8Array[]) => {
  const out = new Uint8Array(chunks.reduce((total, chunk) => total + chunk.length, 0));
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
};

export class BlobPolyfill {
  readonly bytes: Uint8Array;
  readonly type: string;

  constructor(parts: unknown[] = [], options: { type?: string } = {}) {
    this.bytes = concat(parts.map(toBytes));
    this.type = options.type ?? '';
  }

  get size() {
    return this.bytes.length;
  }

  stream(): ByteStream {
    return streamOf(this.bytes);
  }

  async text() {
    return new TextDecoderPolyfill().decode(this.bytes);
  }

  async arrayBuffer() {
    return this.bytes.slice().buffer;
  }
}

export class DecompressionStreamPolyfill implements ByteTransform {
  readonly #format: string;

  constructor(format: 'deflate' | 'deflate-raw' | 'gzip') {
    this.#format = format;
  }

  transform(bytes: Uint8Array) {
    if (this.#format === 'gzip') return gunzipSync(bytes);
    if (this.#format === 'deflate-raw') return inflateSync(bytes);
    return unzlibSync(bytes);
  }
}

type BodyInit = string | Uint8Array | ArrayBuffer | BlobPolyfill | ByteStream | null | undefined;

/** A copy of `bytes` in an `ArrayBuffer` of its own. */
const bufferOf = (bytes: Uint8Array): ArrayBuffer =>
  bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;

/**
 * A fetch answer. Text stays the string it came as: a page is never turned into bytes and back
 * (which made a 1 MB page cost tens of MB). Bytes are made from it only when asked for.
 */
export class ResponsePolyfill {
  readonly status: number;
  readonly statusText: string;
  readonly headers: HeadersPolyfill;
  readonly url: string;
  readonly #text: string | null;
  #bytes: Uint8Array | null;
  bodyUsed = false;

  constructor(
    body?: BodyInit,
    init: { status?: number; statusText?: string; headers?: HeadersInitLike; url?: string } = {},
  ) {
    if (typeof body === 'string') {
      this.#text = body;
      this.#bytes = null;
    } else {
      this.#text = null;
      this.#bytes =
        body === null || body === undefined
          ? new Uint8Array()
          : typeof body === 'object' && 'pipeThrough' in body
            ? body.bytes
            : toBytes(body);
    }
    this.status = init.status ?? 200;
    this.statusText = init.statusText ?? '';
    this.headers = new HeadersPolyfill(init.headers);
    this.url = init.url ?? '';
  }

  get ok() {
    return this.status >= 200 && this.status < 300;
  }

  /** The body's bytes, made from its text the first time they're asked for. */
  #binary(): Uint8Array {
    if (this.#bytes === null) this.#bytes = new TextEncoderPolyfill().encode(this.#text ?? '');
    return this.#bytes;
  }

  async text() {
    this.bodyUsed = true;
    return this.#text ?? new TextDecoderPolyfill().decode(this.#binary());
  }

  async json() {
    return JSON.parse(await this.text());
  }

  async arrayBuffer() {
    this.bodyUsed = true;
    return bufferOf(this.#binary());
  }

  async bytes() {
    this.bodyUsed = true;
    return this.#binary().slice();
  }

  async blob() {
    this.bodyUsed = true;
    return new BlobPolyfill([this.#binary()], { type: this.headers.get('content-type') ?? '' });
  }

  clone() {
    return new ResponsePolyfill(this.#text ?? this.#bytes, this);
  }
}

// ---- Timers ----

let nextTimer = 1;
const cancelled = new Set<number>();

const setTimeoutPolyfill = (callback: (...args: unknown[]) => void, ms = 0, ...args: unknown[]) => {
  const id = nextTimer++;
  void raygun.time.sleep(Number(ms) || 0).then(() => {
    if (cancelled.delete(id)) return;
    callback(...args);
  });
  return id;
};

const clearTimeoutPolyfill = (id?: number) => {
  if (id !== undefined) cancelled.add(id);
};

const setIntervalPolyfill = (
  callback: (...args: unknown[]) => void,
  ms = 0,
  ...args: unknown[]
) => {
  const id = nextTimer++;
  const tick = () => {
    void raygun.time.sleep(Math.max(Number(ms) || 0, 10)).then(() => {
      if (cancelled.delete(id)) return;
      callback(...args);
      tick();
    });
  };
  tick();
  return id;
};

class AbortControllerPolyfill {
  readonly signal = {
    aborted: false,
    reason: undefined as unknown,
    addEventListener() {},
    removeEventListener() {},
  };

  abort(reason?: unknown) {
    this.signal.aborted = true;
    this.signal.reason = reason;
  }
}

/** Installs whichever of these the runtime lacks. */
// ---- A page that isn't there ----

/**
 * Takes every read, call and construction and shows nothing: what a plugin's drawing reaches
 * (React, Nuclear's UI kit, an element's `style.setProperty`). It reads as '' and iterates as
 * empty, and isn't a promise, so awaiting it doesn't hang.
 */
export const inert: unknown = new Proxy(function inert() {}, {
  get: (_target, property) => {
    if (property === Symbol.toPrimitive) return () => '';
    if (property === Symbol.iterator) return function* empty() {};
    if (property === 'then') return undefined;
    return inert;
  },
  set: () => true,
  apply: () => inert,
  construct: () => inert as object,
});

/** An element that takes every call and shows nothing (a plugin's own panel, a style tag). */
const inertElement = (): Record<string, unknown> => {
  const element: Record<string, unknown> = {
    // Styles are set and read back as nothing.
    style: inert,
    dataset: {},
    children: [],
    childNodes: [],
    innerHTML: '',
    textContent: '',
    classList: { add() {}, remove() {}, toggle: () => false, contains: () => false, replace() {} },
    appendChild: (child: unknown) => child,
    removeChild: (child: unknown) => child,
    insertBefore: (child: unknown) => child,
    append() {},
    prepend() {},
    remove() {},
    setAttribute() {},
    getAttribute: () => null,
    removeAttribute() {},
    addEventListener() {},
    removeEventListener() {},
    querySelector: () => inertElement(),
    querySelectorAll: () => [],
    getBoundingClientRect: () => ({ top: 0, left: 0, width: 0, height: 0, right: 0, bottom: 0 }),
  };
  // Anything else an element has (`style.setProperty`, `animate`, `focus`...) is inert.
  return new Proxy(element, {
    get: (target, property) =>
      property in target ? (target as Record<PropertyKey, unknown>)[property] : inert,
  });
};

/**
 * `document` for plugins that also draw in Nuclear's window (a lyrics panel, a mini player): their
 * drawing goes nowhere, and what Raygun runs (their providers) still loads (M46.9.1).
 */
export const inertDocument = () =>
  new Proxy(
    {
      head: inertElement(),
      body: inertElement(),
      documentElement: inertElement(),
      createElement: () => inertElement(),
      createElementNS: () => inertElement(),
      createTextNode: () => inertElement(),
      // A plugin looking up the elements it drew finds them (inert), as it would in Nuclear.
      getElementById: () => inertElement(),
      querySelector: () => inertElement(),
      getElementsByTagName: () => [],
      getElementsByClassName: () => [],
      querySelectorAll: () => [],
      addEventListener() {},
      removeEventListener() {},
    } as Record<PropertyKey, unknown>,
    { get: (target, property) => (property in target ? target[property] : inert) },
  );

/** `navigator` for plugins that read the browser's (Media Session, language, connection). */
const inertNavigator = () =>
  new Proxy(
    { userAgent: 'Mozilla/5.0', language: 'en-US', languages: ['en-US'], onLine: true },
    {
      get: (target, property) =>
        property in target ? (target as Record<PropertyKey, unknown>)[property] : inert,
    },
  );

export const installPolyfills = (
  target: Record<string, unknown> = globalThis as Record<string, unknown>,
) => {
  const provide = (name: string, value: unknown) => {
    if (target[name] === undefined) target[name] = value;
  };
  provide('TextEncoder', TextEncoderPolyfill);
  provide('TextDecoder', TextDecoderPolyfill);
  provide('atob', atobPolyfill);
  provide('btoa', btoaPolyfill);
  provide('URL', URLPolyfill);
  provide('URLSearchParams', URLSearchParamsPolyfill);
  provide('Headers', HeadersPolyfill);
  provide('Blob', BlobPolyfill);
  provide('DecompressionStream', DecompressionStreamPolyfill);
  provide('Response', ResponsePolyfill);
  provide('setTimeout', setTimeoutPolyfill);
  provide('clearTimeout', clearTimeoutPolyfill);
  provide('setInterval', setIntervalPolyfill);
  provide('clearInterval', clearTimeoutPolyfill);
  provide('AbortController', AbortControllerPolyfill);
  provide('queueMicrotask', (callback: () => void) => void Promise.resolve().then(callback));
  const started = Date.now();
  provide('performance', { now: () => Date.now() - started, timeOrigin: started });
  provide('document', inertDocument());
  provide('navigator', inertNavigator());
  provide('window', target);
  // The page's own events, frames and layout never happen here: listening is fine, and nothing
  // is drawn, so no frame ever comes (an animation loop doesn't spin).
  provide('addEventListener', () => {});
  provide('removeEventListener', () => {});
  provide('dispatchEvent', () => true);
  provide('requestAnimationFrame', () => 0);
  provide('cancelAnimationFrame', () => {});
  provide('matchMedia', (query: string) => ({
    matches: false,
    media: String(query),
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
  }));
  provide('getComputedStyle', () => ({ getPropertyValue: () => '' }));
  for (const name of ['MutationObserver', 'ResizeObserver', 'IntersectionObserver']) {
    provide(name, InertObserver);
  }
};

/** An observer of a page that never changes. */
class InertObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}
