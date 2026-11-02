// Web APIs Nuclear plugins use that QuickJS lacks: URL, URLSearchParams, Headers, base64,
// TextEncoder/TextDecoder, timers, and the Blob → DecompressionStream → Response pipeline some
// plugins use to inflate data. Small and pragmatic: enough for real plugins, not full specs.
import { gunzipSync, inflateSync, unzlibSync } from 'fflate';

// ---- Text ----

export class TextEncoderPolyfill {
  readonly encoding = 'utf-8';

  encode(text = ''): Uint8Array {
    const bytes: number[] = [];
    for (const character of text) {
      let code = character.codePointAt(0) ?? 0;
      if (code < 0x80) {
        bytes.push(code);
      } else if (code < 0x800) {
        bytes.push(0xc0 | (code >> 6), 0x80 | (code & 0x3f));
      } else if (code < 0x10000) {
        bytes.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 0x3f), 0x80 | (code & 0x3f));
      } else {
        code = Math.min(code, 0x10ffff);
        bytes.push(
          0xf0 | (code >> 18),
          0x80 | ((code >> 12) & 0x3f),
          0x80 | ((code >> 6) & 0x3f),
          0x80 | (code & 0x3f),
        );
      }
    }
    return Uint8Array.from(bytes);
  }
}

export class TextDecoderPolyfill {
  readonly encoding = 'utf-8';

  decode(input?: ArrayBuffer | ArrayBufferView): string {
    if (!input) return '';
    const bytes =
      input instanceof ArrayBuffer
        ? new Uint8Array(input)
        : new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    let text = '';
    for (let index = 0; index < bytes.length;) {
      const first = bytes[index++] ?? 0;
      let code = first;
      const extra = first >= 0xf0 ? 3 : first >= 0xe0 ? 2 : first >= 0xc0 ? 1 : 0;
      if (extra > 0) {
        code = first & (0x3f >> extra);
        for (let part = 0; part < extra; part += 1) {
          code = (code << 6) | ((bytes[index++] ?? 0) & 0x3f);
        }
      }
      text += String.fromCodePoint(Math.min(code, 0x10ffff));
    }
    return text;
  }
}

// ---- Base64 ----

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';

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

export class ResponsePolyfill {
  readonly status: number;
  readonly statusText: string;
  readonly headers: HeadersPolyfill;
  readonly url: string;
  readonly #bytes: Uint8Array;
  bodyUsed = false;

  constructor(
    body?: BodyInit,
    init: { status?: number; statusText?: string; headers?: HeadersInitLike; url?: string } = {},
  ) {
    this.#bytes =
      body === null || body === undefined
        ? new Uint8Array()
        : typeof body === 'object' && 'pipeThrough' in body
          ? body.bytes
          : toBytes(body);
    this.status = init.status ?? 200;
    this.statusText = init.statusText ?? '';
    this.headers = new HeadersPolyfill(init.headers);
    this.url = init.url ?? '';
  }

  get ok() {
    return this.status >= 200 && this.status < 300;
  }

  async text() {
    this.bodyUsed = true;
    return new TextDecoderPolyfill().decode(this.#bytes);
  }

  async json() {
    return JSON.parse(await this.text());
  }

  async arrayBuffer() {
    this.bodyUsed = true;
    return this.#bytes.slice().buffer;
  }

  async blob() {
    return new BlobPolyfill([this.#bytes]);
  }

  clone() {
    return new ResponsePolyfill(this.#bytes, this);
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

/** An element that takes every call and shows nothing (a plugin's own panel, a style tag). */
const inertElement = (): Record<string, unknown> => {
  const element: Record<string, unknown> = {
    style: {},
    dataset: {},
    children: [],
    childNodes: [],
    innerHTML: '',
    textContent: '',
    classList: { add() {}, remove() {}, toggle: () => false, contains: () => false },
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
    querySelector: () => null,
    querySelectorAll: () => [],
    getBoundingClientRect: () => ({ top: 0, left: 0, width: 0, height: 0, right: 0, bottom: 0 }),
  };
  return element;
};

/**
 * `document` for plugins that also draw in Nuclear's window (a lyrics panel, a mini player): their
 * drawing goes nowhere, and what Raygun runs (their providers) still loads (M46.9.1).
 */
export const inertDocument = () => ({
  head: inertElement(),
  body: inertElement(),
  documentElement: inertElement(),
  createElement: () => inertElement(),
  createElementNS: () => inertElement(),
  createTextNode: () => inertElement(),
  getElementById: () => null,
  getElementsByTagName: () => [],
  getElementsByClassName: () => [],
  querySelector: () => null,
  querySelectorAll: () => [],
  addEventListener() {},
  removeEventListener() {},
});

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
  provide('document', inertDocument());
  provide('window', target);
};
