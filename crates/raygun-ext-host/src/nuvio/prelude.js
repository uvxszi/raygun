// Nuvio's provider runtime over Raygun's host API. Nuvio (tapframe/NuvioStreaming, GPL-3.0:
// composeApp/src/fullCommonMain/kotlin/com/nuvio/app/features/plugins/runtime) evaluates each
// provider as a CommonJS module with globals of its own (`JsBindings.kt`): `fetch`, cheerio and
// crypto-js through `require`, `CryptoJS`, `crypto.subtle`, `atob`/`btoa`, `URL`, `TextEncoder`,
// timers, `AbortController`, `SCRAPER_ID`, `SCRAPER_SETTINGS` and `TMDB_API_KEY`, then calls the
// module's `getStreams(tmdbId, mediaType, season, episode)`. This prelude builds those globals
// over Raygun's natives (the LNReader prelude, evaluated before it, gives the timers, base64,
// `URL`, `Headers`, `FormData` and cheerio) and presents the provider as a `video.streams`
// provider. Evaluated after Raygun's prelude, before crypto-js and the provider.
'use strict';
(() => {
  const native = globalThis.__rgNuvio;
  delete globalThis.__rgNuvio;
  const host = globalThis.raygun;
  // What the LNReader prelude hands out (cheerio); the rest of it isn't a Nuvio provider's.
  const libraries = globalThis.__lnrRequire;
  delete globalThis.__lnrRequire;
  delete globalThis.__lnrStart;

  const unwrap = (text) => {
    const envelope = JSON.parse(text);
    if (envelope.error !== undefined) throw new Error(envelope.error);
    return envelope.value;
  };
  const str = (value) => (value === undefined || value === null ? '' : String(value));
  const define = (name, value) =>
    Object.defineProperty(globalThis, name, {
      value,
      writable: true,
      configurable: true,
      enumerable: false,
    });

  // ---- Bytes and text ----

  const utf8Encode = (text) => {
    const input = str(text);
    const out = [];
    for (let i = 0; i < input.length; i++) {
      let code = input.charCodeAt(i);
      if (code >= 0xd800 && code <= 0xdbff && i + 1 < input.length) {
        const next = input.charCodeAt(i + 1);
        if (next >= 0xdc00 && next <= 0xdfff) {
          code = 0x10000 + ((code - 0xd800) << 10) + (next - 0xdc00);
          i++;
        } else code = 0xfffd;
      } else if (code >= 0xd800 && code <= 0xdfff) code = 0xfffd;
      if (code < 0x80) out.push(code);
      else if (code < 0x800) out.push(0xc0 | (code >> 6), 0x80 | (code & 63));
      else if (code < 0x10000)
        out.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63));
      else
        out.push(
          0xf0 | (code >> 18),
          0x80 | ((code >> 12) & 63),
          0x80 | ((code >> 6) & 63),
          0x80 | (code & 63),
        );
    }
    return Uint8Array.from(out);
  };

  /** UTF-8 to text; a sequence that isn't UTF-8 reads as U+FFFD. */
  const utf8Decode = (bytes) => {
    const parts = [];
    let codes = [];
    const flush = () => {
      if (codes.length) parts.push(String.fromCodePoint(...codes));
      codes = [];
    };
    for (let i = 0; i < bytes.length;) {
      const byte = bytes[i];
      let code;
      let size;
      if (byte < 0x80) [code, size] = [byte, 1];
      else if (byte >= 0xc2 && byte < 0xe0) [code, size] = [byte & 0x1f, 2];
      else if (byte >= 0xe0 && byte < 0xf0) [code, size] = [byte & 0x0f, 3];
      else if (byte >= 0xf0 && byte < 0xf5) [code, size] = [byte & 0x07, 4];
      else [code, size] = [-1, 1];
      let whole = code >= 0 && i + size <= bytes.length;
      for (let k = 1; whole && k < size; k++) {
        const next = bytes[i + k];
        if ((next & 0xc0) !== 0x80) whole = false;
        else code = (code << 6) | (next & 0x3f);
      }
      const odd =
        (size === 3 && code < 0x800) ||
        (size === 4 && (code < 0x10000 || code > 0x10ffff)) ||
        (code >= 0xd800 && code <= 0xdfff);
      if (!whole || odd) {
        codes.push(0xfffd);
        i++;
      } else {
        codes.push(code);
        i += size;
      }
      if (codes.length >= 4096) flush();
    }
    flush();
    return parts.join('');
  };

  const toBytes = (data) => {
    if (data === undefined || data === null) return new Uint8Array(0);
    if (data instanceof Uint8Array) return data;
    if (data instanceof ArrayBuffer) return new Uint8Array(data);
    if (ArrayBuffer.isView(data))
      return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    if (Array.isArray(data)) return Uint8Array.from(data);
    if (typeof data === 'string') return utf8Encode(data);
    return new Uint8Array(0);
  };
  const toBase64 = (bytes) => {
    let binary = '';
    for (let i = 0; i < bytes.length; i += 0x2000) {
      binary += String.fromCharCode(...bytes.subarray(i, i + 0x2000));
    }
    return btoa(binary);
  };
  const fromBase64 = (text) => {
    const binary = atob(str(text));
    const out = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i);
    return out;
  };
  const bufferOf = (bytes) => {
    const copy = new Uint8Array(bytes.length);
    copy.set(bytes);
    return copy.buffer;
  };

  if (typeof globalThis.TextEncoder !== 'function') {
    class TextEncoder {
      get encoding() {
        return 'utf-8';
      }
      encode(text = '') {
        return utf8Encode(text);
      }
    }
    define('TextEncoder', TextEncoder);
  }
  if (typeof globalThis.TextDecoder !== 'function') {
    const SINGLE_BYTE = ['latin1', 'iso-8859-1', 'ascii', 'us-ascii', 'windows-1252'];
    class TextDecoder {
      constructor(label = 'utf-8') {
        this._single = SINGLE_BYTE.includes(str(label).trim().toLowerCase());
      }
      get encoding() {
        return this._single ? 'windows-1252' : 'utf-8';
      }
      decode(data) {
        const bytes = toBytes(data);
        if (!this._single) return utf8Decode(bytes);
        let out = '';
        for (let i = 0; i < bytes.length; i += 0x2000) {
          out += String.fromCharCode(...bytes.subarray(i, i + 0x2000));
        }
        return out;
      }
    }
    define('TextDecoder', TextDecoder);
  }

  // ---- Timers and aborting ----

  /** Longest timer, as Nuvio's: a provider's call has a minute there. */
  const LONGEST_TIMER_MS = 60000;
  /** Raygun's sleep is at most 10 s a call; longer waits sleep in turns. */
  const sleepFor = async (ms) => {
    let left = Math.min(Math.max(Number(ms) || 0, 0), LONGEST_TIMER_MS);
    do {
      const turn = Math.min(left, 10000);
      await host.time.sleep(turn);
      left -= turn;
    } while (left > 0);
  };
  let timerId = 1000000;
  const cleared = new Set();
  const report = (error) => host.log.warn(`a timer's callback failed: ${error?.message ?? error}`);
  define('setTimeout', (callback, ms, ...args) => {
    const id = ++timerId;
    void sleepFor(ms).then(() => {
      if (cleared.delete(id)) return;
      try {
        if (typeof callback === 'function') callback(...args);
      } catch (error) {
        report(error);
      }
    });
    return id;
  });
  define('setInterval', (callback, ms, ...args) => {
    const id = ++timerId;
    const every = Math.max(1, Number(ms) || 0);
    // An interval runs at most a minute, as long as a provider's call may.
    const until = Date.now() + LONGEST_TIMER_MS;
    const tick = () => {
      void sleepFor(every).then(() => {
        if (cleared.delete(id) || Date.now() > until) return;
        try {
          if (typeof callback === 'function') callback(...args);
        } catch (error) {
          report(error);
        }
        tick();
      });
    };
    tick();
    return id;
  });
  const clear = (id) => {
    if (id !== undefined && id !== null) cleared.add(id);
  };
  define('clearTimeout', clear);
  define('clearInterval', clear);

  const abortError = (reason) => {
    if (reason instanceof Error) return reason;
    const error = new Error(reason === undefined ? 'The operation was aborted' : str(reason));
    error.name = 'AbortError';
    return error;
  };
  class AbortSignal {
    constructor() {
      this.aborted = false;
      this.reason = undefined;
      this._listeners = [];
      this.onabort = null;
    }
    addEventListener(type, listener) {
      if (type === 'abort' && typeof listener === 'function') this._listeners.push(listener);
    }
    removeEventListener(type, listener) {
      this._listeners = this._listeners.filter((known) => known !== listener);
    }
    throwIfAborted() {
      if (this.aborted) throw abortError(this.reason);
    }
    _abort(reason) {
      if (this.aborted) return;
      this.aborted = true;
      this.reason = reason === undefined ? abortError() : reason;
      const event = { type: 'abort', target: this };
      for (const listener of [this.onabort, ...this._listeners]) {
        try {
          if (typeof listener === 'function') listener.call(this, event);
        } catch {}
      }
    }
    static timeout(ms) {
      const signal = new AbortSignal();
      setTimeout(
        () => {
          const error = new Error('The operation timed out');
          error.name = 'TimeoutError';
          signal._abort(error);
        },
        Number(ms) || 0,
      );
      return signal;
    }
    static abort(reason) {
      const signal = new AbortSignal();
      signal._abort(reason);
      return signal;
    }
  }
  class AbortController {
    constructor() {
      this.signal = new AbortSignal();
    }
    abort(reason) {
      this.signal._abort(reason);
    }
  }
  define('AbortSignal', AbortSignal);
  define('AbortController', AbortController);

  // ---- fetch, as Nuvio's ----

  // Requests that failed lately. Providers often swallow a refusal and answer nothing; a call
  // that comes back empty then fails with what its requests met, so the app says why.
  const failures = [];
  const noteFailure = (url, status, message) => {
    failures.push({ at: Date.now(), url, status, message });
    if (failures.length > 20) failures.shift();
  };
  const failIfEmpty = (start, empty) => {
    const failure = failures.find((failed) => failed.at >= start);
    if (!empty || !failure) return;
    let where = failure.url;
    try {
      where = new URL(failure.url).hostname;
    } catch {}
    throw new Error(
      failure.status ? `${where} answered HTTP ${failure.status}` : `${where}: ${failure.message}`,
    );
  };

  const bodyOf = (body, headers) => {
    if (body === undefined || body === null) return {};
    if (typeof body === 'string') return { body };
    if (body instanceof URLSearchParams || body instanceof FormData) {
      if (!headers.has('content-type')) {
        headers.set('content-type', 'application/x-www-form-urlencoded;charset=UTF-8');
      }
      return { body: new URLSearchParams(body._list).toString() };
    }
    if (body instanceof ArrayBuffer || ArrayBuffer.isView(body)) {
      return { bodyBase64: toBase64(toBytes(body)) };
    }
    return { body: String(body) };
  };

  const answer = (head, text, bytes) => {
    const ok = head.status >= 200 && head.status < 300;
    const raw = () => bytes ?? utf8Encode(text);
    return {
      ok,
      status: head.status,
      statusText: head.statusText ?? '',
      url: head.url,
      redirected: head.redirected === true,
      type: 'basic',
      headers: new Headers(head.headers ?? {}),
      bodyUsed: false,
      text: async () => text,
      // As Nuvio's: an answer that isn't JSON (or is empty) reads as null.
      json: async () => {
        if (!text) return null;
        try {
          return JSON.parse(text);
        } catch {
          return null;
        }
      },
      arrayBuffer: async () => bufferOf(raw()),
      bytes: async () => Uint8Array.from(raw()),
      blob: async () => {
        const data = raw();
        return {
          size: data.length,
          type: head.headers?.['content-type'] ?? '',
          text: async () => text,
          arrayBuffer: async () => bufferOf(data),
        };
      },
      clone: () => answer(head, text, bytes),
    };
  };

  const fetchApi = async (input, init) => {
    const request = input && typeof input === 'object' && 'url' in input ? input : null;
    const url = str(request ? request.url : input);
    const options = init ?? request ?? {};
    const signal = options.signal;
    if (signal?.aborted) throw abortError(signal.reason);
    const given = options.headers;
    const headers = new Headers(
      Array.isArray(given)
        ? Object.fromEntries(given.filter((pair) => Array.isArray(pair) && pair.length >= 2))
        : (given ?? {}),
    );
    const sent = {
      method: str(options.method || 'GET').toUpperCase(),
      ...bodyOf(options.body, headers),
      // Bytes that aren't text come too, for `arrayBuffer`.
      binary: true,
    };
    sent.headers = Object.fromEntries(headers.entries());
    if (options.redirect === 'manual') sent.redirect = 'manual';
    const asked = host.http.fetch(url, sent).then(
      async (response) => ({
        response,
        text: await response.text(),
        bytes: await response.bytes(),
      }),
      (error) => ({ error }),
    );
    const settled = signal
      ? await Promise.race([
          asked,
          new Promise((_, reject) => {
            signal.addEventListener('abort', () => reject(abortError(signal.reason)));
          }),
        ])
      : await asked;
    if (settled.error) {
      // As Nuvio's: a request that failed answers status 0 with the reason, rather than throwing.
      const message = str(settled.error?.message ?? settled.error);
      noteFailure(url, 0, message);
      return answer({ status: 0, url, statusText: message, headers: {} }, '', null);
    }
    const { response, text, bytes } = settled;
    if (!response.ok && !(response.status >= 300 && response.status < 400)) {
      noteFailure(response.url, response.status, '');
    }
    return answer(
      {
        status: response.status,
        url: response.url,
        headers: response.headers,
        redirected: response.url !== url,
      },
      text,
      bytes,
    );
  };
  define('fetch', fetchApi);

  // ---- crypto ----

  const crypt = (args) => fromBase64(unwrap(native.crypto(JSON.stringify(args))));
  const base64Of = (data) => toBase64(toBytes(data));
  const hashName = (hash) =>
    str(hash?.name ?? hash ?? 'SHA-256')
      .toUpperCase()
      .replace(/[^A-Z0-9]/g, '')
      .toLowerCase();
  const algorithmName = (algorithm) => str(algorithm?.name ?? algorithm).toUpperCase();
  const digest = (hash, data) =>
    crypt({ op: 'digest', algorithm: hashName(hash), data: base64Of(data) });
  const hmac = (hash, key, data) =>
    crypt({ op: 'hmac', algorithm: hashName(hash), key: base64Of(key), data: base64Of(data) });
  const randomBytes = (count) => fromBase64(unwrap(native.random(Number(count) || 0)));

  class CryptoKey {
    constructor(type, algorithm, extractable, usages, raw) {
      this.type = type;
      this.algorithm = algorithm;
      this.extractable = extractable;
      this.usages = usages;
      Object.defineProperty(this, '_raw', { value: raw });
    }
  }
  const keyOf = (raw, algorithm, extractable, usages) => {
    const name = algorithmName(algorithm);
    const described = { name };
    if (algorithm?.hash) described.hash = { name: str(algorithm.hash?.name ?? algorithm.hash) };
    if (name.startsWith('AES')) described.length = raw.length * 8;
    return new CryptoKey('secret', described, Boolean(extractable), usages ?? [], raw);
  };
  const aes = (encrypt, algorithm, key, data) => {
    const name = algorithmName(algorithm);
    const args = { op: 'aes', key: base64Of(key._raw), data: base64Of(data), encrypt };
    if (name === 'AES-CBC') Object.assign(args, { mode: 'cbc', iv: base64Of(algorithm.iv) });
    else if (name === 'AES-CTR')
      Object.assign(args, { mode: 'ctr', iv: base64Of(algorithm.counter) });
    else if (name === 'AES-GCM') {
      if (algorithm.tagLength && Number(algorithm.tagLength) !== 128) {
        throw new Error('Only 128-bit AES-GCM tags are supported');
      }
      Object.assign(args, { mode: 'gcm', iv: base64Of(algorithm.iv) });
      if (algorithm.additionalData) args.aad = base64Of(algorithm.additionalData);
    } else throw new Error(`${name} isn't a cipher Raygun's Nuvio runtime has`);
    return bufferOf(crypt(args));
  };
  const deriveBits = (algorithm, key, length) => {
    if (algorithmName(algorithm) !== 'PBKDF2') throw new Error('Only PBKDF2 derives bits here');
    return crypt({
      op: 'pbkdf2',
      algorithm: hashName(algorithm.hash),
      password: base64Of(key._raw),
      salt: base64Of(algorithm.salt),
      iterations: Number(algorithm.iterations) || 1,
      length: Math.ceil((Number(length) || 256) / 8),
    });
  };
  const subtle = {
    digest: async (algorithm, data) => bufferOf(digest(algorithm, data)),
    importKey: async (format, keyData, algorithm, extractable, usages) => {
      if (str(format).toLowerCase() !== 'raw') {
        throw new Error(`Raygun's Nuvio runtime imports raw keys, not ${format}`);
      }
      return keyOf(Uint8Array.from(toBytes(keyData)), algorithm, extractable, usages);
    },
    exportKey: async (format, key) => bufferOf(key._raw),
    generateKey: async (algorithm, extractable, usages) => {
      const name = algorithmName(algorithm);
      const bits = Number(algorithm?.length) || (name === 'HMAC' ? 256 : 256);
      return keyOf(randomBytes(bits / 8), algorithm, extractable, usages);
    },
    sign: async (algorithm, key, data) => {
      if (algorithmName(algorithm) !== 'HMAC') throw new Error('Only HMAC signs here');
      return bufferOf(hmac(key.algorithm.hash ?? 'SHA-256', key._raw, data));
    },
    verify: async (algorithm, key, signature, data) => {
      const expected = hmac(key.algorithm.hash ?? 'SHA-256', key._raw, data);
      const given = toBytes(signature);
      if (expected.length !== given.length) return false;
      let difference = 0;
      for (let i = 0; i < expected.length; i++) difference |= expected[i] ^ given[i];
      return difference === 0;
    },
    encrypt: async (algorithm, key, data) => aes(true, algorithm, key, data),
    decrypt: async (algorithm, key, data) => aes(false, algorithm, key, data),
    deriveBits: async (algorithm, key, length) => bufferOf(deriveBits(algorithm, key, length)),
    deriveKey: async (algorithm, key, derived, extractable, usages) =>
      keyOf(
        deriveBits(algorithm, key, Number(derived?.length) || 256),
        derived,
        extractable,
        usages,
      ),
  };
  const getRandomValues = (array) => {
    const view = toBytes(array);
    view.set(randomBytes(view.length));
    return array;
  };
  define('crypto', {
    subtle,
    getRandomValues,
    randomUUID: () => {
      const bytes = randomBytes(16);
      bytes[6] = (bytes[6] & 0x0f) | 0x40;
      bytes[8] = (bytes[8] & 0x3f) | 0x80;
      const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
      return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    },
  });
  define('CryptoKey', CryptoKey);

  // ---- require and cheerio ----

  const cheerio = libraries('cheerio');
  define('cheerio', cheerio);
  const modules = {
    cheerio: () => cheerio,
    'cheerio-without-node-native': () => cheerio,
    'react-native-cheerio': () => cheerio,
    'crypto-js': () => {
      if (!globalThis.CryptoJS) throw new Error("crypto-js isn't loaded");
      return globalThis.CryptoJS;
    },
  };
  define('require', (name) => {
    const make = modules[str(name)];
    if (!make) throw new Error(`Module '${name}' is not available`);
    return make();
  });
  define('__nuvioModule', { exports: {} });

  // ---- Settings ----

  // The fields `onSettings` declared, kept so the next start knows them before the provider runs:
  // `[{ key, type, default, choices: [[label, value]] }]`.
  const FIELDS_KEY = 'nuvio.settings';
  const fields = () => {
    try {
      const kept = host.storage.get(FIELDS_KEY);
      return Array.isArray(kept) ? kept : [];
    } catch {
      return [];
    }
  };
  const valueOf = (field) => {
    let value;
    try {
      value = host.settings.get(field.key);
    } catch {
      value = null;
    }
    if (value === null || value === undefined) return field.default;
    const choice = (field.choices ?? []).find(([label]) => label === value);
    return choice ? choice[1] : value;
  };
  const scraperSettings = {};
  const fillSettings = () => {
    for (const field of fields()) scraperSettings[field.key] = valueOf(field);
  };
  fillSettings();
  define('SCRAPER_SETTINGS', scraperSettings);
  if (typeof globalThis.TMDB_API_KEY === 'undefined') {
    let key = '';
    try {
      key = str(host.settings.get('tmdbApiKey')).trim();
    } catch {}
    define('TMDB_API_KEY', key);
  }

  /** `onSettings`'s layout (text, select and toggle fields) as the extension's settings. */
  const declareSettings = (layout) => {
    const declared = [];
    for (const item of Array.isArray(layout) ? layout : []) {
      if (!item || typeof item !== 'object') continue;
      const type = str(item.type).toLowerCase();
      const key = str(item.key).trim();
      if (!key || !['text', 'select', 'toggle'].includes(type)) continue;
      const choices =
        type === 'select' && Array.isArray(item.options)
          ? item.options
              .map((option) => [str(option?.label ?? option).trim(), str(option?.value ?? option)])
              .filter(([label]) => label)
          : [];
      const given = item.defaultValue ?? item.default;
      let fallback;
      if (type === 'toggle') fallback = given === true || given === 'true';
      else if (given !== undefined && given !== null) fallback = str(given);
      else fallback = type === 'select' ? (choices[0]?.[1] ?? '') : '';
      try {
        unwrap(
          native.registerSetting(
            JSON.stringify({
              key,
              label: str(item.label).trim() || key,
              type: type === 'toggle' ? 'Switch' : choices.length ? 'Select' : 'Text',
              value: fallback,
              options: choices,
            }),
          ),
        );
        declared.push({ key, type, default: fallback, ...(choices.length ? { choices } : {}) });
      } catch (error) {
        host.log.warn(`setting ${key}: ${error.message}`);
      }
    }
    host.storage.set(FIELDS_KEY, declared);
    fillSettings();
  };

  // ---- Streams ----

  const HASH = /^[0-9a-f]{40}$/i;
  const plainHeaders = (given) => {
    const out = {};
    if (!given || typeof given !== 'object') return out;
    const entries =
      typeof given.entries === 'function' ? [...given.entries()] : Object.entries(given);
    for (const [name, value] of entries) {
      if (value !== undefined && value !== null && typeof value !== 'object')
        out[name] = str(value);
    }
    return out;
  };
  const magnetOf = (item, hash) => {
    let link = `magnet:?xt=urn:btih:${hash.toLowerCase()}`;
    const name = str(item.title || item.name)
      .split('\n')[0]
      .trim();
    if (name) link += `&dn=${encodeURIComponent(name)}`;
    for (const tracker of Array.isArray(item.sources) ? item.sources : []) {
      const address = str(tracker).replace(/^tracker:/, '');
      if (address && !address.startsWith('dht:')) link += `&tr=${encodeURIComponent(address)}`;
    }
    if (Number.isInteger(item.fileIdx)) link += `&so=${item.fileIdx}`;
    return link;
  };

  /** Nuvio's stream objects as Raygun's `streams` answer. */
  const toStreams = (result) => {
    const items = Array.isArray(result)
      ? result
      : Array.isArray(result?.streams)
        ? result.streams
        : [];
    const out = [];
    for (const item of items) {
      if (!item || typeof item !== 'object') continue;
      let url = str(item.url && typeof item.url === 'object' ? item.url.url : item.url).trim();
      const hash = str(item.infoHash).trim();
      if (!url && HASH.test(hash)) url = magnetOf(item, hash);
      if (!url) continue;
      const kind = str(item.type).toLowerCase();
      const path = url.split(/[?#]/)[0].toLowerCase();
      let format;
      if (url.startsWith('magnet:') || kind === 'torrent') format = 'torrent';
      else if (kind === 'hls' || kind === 'm3u8' || path.endsWith('.m3u8')) format = 'hls';
      else if (kind === 'dash' || path.endsWith('.mpd')) format = 'dash';
      const title = str(item.title).trim();
      const quality = str(item.quality).trim();
      const named = str(item.name).trim() || str(item.provider).trim();
      const name =
        quality && !named.toLowerCase().includes(quality.toLowerCase())
          ? `${named} ${quality}`.trim()
          : named;
      const size = str(item.size).trim();
      const language = str(item.language ?? item.lang).trim();
      const seeds = Number(item.seeders);
      const extra = [
        size,
        language,
        item.seeders !== undefined && item.seeders !== null && Number.isFinite(seeds)
          ? `Seeders: ${seeds}`
          : '',
      ].filter((part) => part && !title.includes(part));
      const description = [title, extra.join(' · ')].filter(Boolean).join('\n');
      const subtitles = (Array.isArray(item.subtitles) ? item.subtitles : [])
        .map((track) => ({
          url: str(track?.url).trim(),
          lang: str(track?.language ?? track?.lang).trim() || undefined,
          label: str(track?.name ?? track?.label).trim() || undefined,
        }))
        .filter((track) => /^https?:\/\//i.test(track.url));
      out.push({
        url,
        format,
        quality: item.quality,
        name: name || undefined,
        description: description || undefined,
        size: item.size,
        headers: plainHeaders(
          item.headers ?? item.behaviorHints?.proxyHeaders?.request ?? item.behaviorHints?.headers,
        ),
        subtitles,
      });
    }
    return out;
  };

  let types = ['movie', 'tv'];
  Object.defineProperty(globalThis, '__nuvioSetup', {
    configurable: true,
    value: (meta) => {
      delete globalThis.__nuvioSetup;
      define('SCRAPER_ID', str(meta?.scraper));
      if (Array.isArray(meta?.types) && meta.types.length) types = meta.types.map(str);
    },
  });

  const register = (getStreams) => {
    host.providers.register({
      kind: 'video.streams',
      id: 'streams',
      streams: async (query) => {
        const mediaType = query?.type === 'movie' ? 'movie' : 'tv';
        if (!types.includes(mediaType)) return { streams: [] };
        const id = str(query?.tmdbId).trim() || str(query?.imdbId).trim();
        if (!id) return { streams: [] };
        const season = mediaType === 'tv' ? (query?.season ?? undefined) : undefined;
        const episode = mediaType === 'tv' ? (query?.episode ?? undefined) : undefined;
        if (mediaType === 'tv' && (season === undefined || episode === undefined)) {
          return { streams: [] };
        }
        const start = Date.now();
        const streams = toStreams(await getStreams(id, mediaType, season, episode));
        failIfEmpty(start, streams.length === 0);
        return { streams };
      },
    });
  };

  Object.defineProperty(globalThis, '__nuvioStart', {
    configurable: true,
    value: (module) => {
      delete globalThis.__nuvioStart;
      const exported = module && typeof module === 'object' ? module.exports : undefined;
      const pick = (name) => {
        if (exported && typeof exported[name] === 'function') return exported[name];
        return typeof globalThis[name] === 'function' ? globalThis[name] : null;
      };
      const getStreams = pick('getStreams');
      // Without one, it registers nothing and Raygun says it isn't for Raygun.
      if (getStreams) register(getStreams);
      const onSettings = pick('onSettings');
      if (!onSettings) return undefined;
      return Promise.resolve()
        .then(() => onSettings())
        .then(declareSettings, (error) =>
          host.log.warn(`its settings couldn't be read: ${error?.message ?? error}`),
        );
    },
  });
})();
