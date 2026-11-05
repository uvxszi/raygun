// Miru's extension runtime over Raygun's host API. Adapted from miru-project/miru-app
// (lib/data/services/extension_service.dart, AGPL-3.0): the `Extension` base class scripts
// extend, `Element` and `XPathNode`, `this.request` with Miru's (Dio's) semantics, settings,
// `console`, `setTimeout`, a `fetch`, `atob`/`btoa` and `md5`. Then the script's methods become a
// Raygun video, manga or novel source. Evaluated after Raygun's prelude, before the script.
'use strict';
(() => {
  const native = globalThis.__rgMiru;
  delete globalThis.__rgMiru;
  const host = globalThis.raygun;

  const unwrap = (text) => {
    const envelope = JSON.parse(text);
    if (envelope.error !== undefined) throw new Error(envelope.error);
    return envelope.value;
  };
  const str = (value) => (value === undefined || value === null ? '' : String(value));
  const message = (error) => (error instanceof Error ? error.message : str(error));
  const info = JSON.parse(native.info());

  // Globals scripts may replace: configurable, like Miru's own definitions.
  const define = (name, value) =>
    Object.defineProperty(globalThis, name, {
      value,
      writable: true,
      configurable: true,
      enumerable: false,
    });
  define('window', globalThis);
  define('global', globalThis);

  // Scripts log whole pages; keep the log readable.
  const MAX_LOG_CHARS = 2000;
  const logger =
    (level) =>
    (first, ...rest) => {
      const text = [first, ...rest]
        .map((value) => {
          if (typeof value === 'string') return value;
          if (value instanceof Error) return `${value.name}: ${value.message}`;
          try {
            return JSON.stringify(value);
          } catch {
            return String(value);
          }
        })
        .join(' ');
      host.log[level](text.length > MAX_LOG_CHARS ? `${text.slice(0, MAX_LOG_CHARS)}…` : text);
    };
  define(
    'console',
    Object.freeze({
      log: logger('debug'),
      info: logger('debug'),
      debug: logger('debug'),
      warn: logger('warn'),
      error: logger('warn'),
    }),
  );

  // ---- Timers: host sleeps of at most 10 s, chained ----

  const wait = async (ms) => {
    let left = Math.min(Number(ms) || 0, 3_600_000);
    do {
      const step = Math.min(Math.max(left, 0), 10000);
      await host.time.sleep(step);
      left -= step;
    } while (left > 0);
  };
  let nextTimer = 0;
  const timers = new Set();
  define('setTimeout', (callback, ms, ...args) => {
    nextTimer += 1;
    const id = nextTimer;
    timers.add(id);
    wait(ms).then(() => {
      if (!timers.delete(id) || typeof callback !== 'function') return;
      try {
        callback(...args);
      } catch (error) {
        console.warn(`a timer failed: ${message(error)}`);
      }
    });
    return id;
  });
  define('clearTimeout', (id) => {
    timers.delete(id);
  });

  // ---- base64, as browsers have it ----

  const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  define('btoa', (input) => {
    const text = str(input);
    let out = '';
    for (let index = 0; index < text.length; index += 3) {
      const bytes = [
        text.charCodeAt(index),
        text.charCodeAt(index + 1),
        text.charCodeAt(index + 2),
      ];
      if (bytes.some((byte) => byte > 255))
        throw new Error('btoa: the text has characters past Latin-1');
      const [a, b = 0, c = 0] = bytes.map((byte) => (Number.isNaN(byte) ? undefined : byte));
      const chunk = (a << 16) | (b << 8) | c;
      out += ALPHABET[(chunk >> 18) & 63] + ALPHABET[(chunk >> 12) & 63];
      out += index + 1 < text.length ? ALPHABET[(chunk >> 6) & 63] : '=';
      out += index + 2 < text.length ? ALPHABET[chunk & 63] : '=';
    }
    return out;
  });
  define('atob', (input) => {
    const text = str(input)
      .replace(/[\t\n\f\r ]+/g, '')
      .replace(/=+$/, '');
    if (/[^A-Za-z0-9+/]/.test(text) || text.length % 4 === 1) {
      throw new Error('atob: the text is not base64');
    }
    let out = '';
    let bits = 0;
    let buffer = 0;
    for (const char of text) {
      buffer = (buffer << 6) | ALPHABET.indexOf(char);
      bits += 6;
      if (bits >= 8) {
        bits -= 8;
        out += String.fromCharCode((buffer >> bits) & 255);
      }
    }
    return out;
  });

  // ---- md5: the npm `md5` package's interface, over CryptoJS ----

  define('md5', (input, options) => {
    if (input === undefined || input === null) throw new Error(`Illegal argument ${input}`);
    if (typeof CryptoJS === 'undefined') throw new Error('md5 needs CryptoJS, which is missing');
    let words;
    if (typeof input === 'string') {
      words =
        options?.encoding === 'binary'
          ? CryptoJS.enc.Latin1.parse(input)
          : CryptoJS.enc.Utf8.parse(input);
    } else if (Array.isArray(input) || input instanceof Uint8Array) {
      let binary = '';
      for (const byte of input) binary += String.fromCharCode(byte & 255);
      words = CryptoJS.enc.Latin1.parse(binary);
    } else {
      words = CryptoJS.enc.Utf8.parse(String(input));
    }
    const digest = CryptoJS.MD5(words);
    if (options?.asBytes) {
      const hex = digest.toString();
      const bytes = [];
      for (let index = 0; index < hex.length; index += 2) {
        bytes.push(parseInt(hex.slice(index, index + 2), 16));
      }
      return bytes;
    }
    if (options?.asString) return digest.toString(CryptoJS.enc.Latin1);
    return digest.toString();
  });

  // ---- fetch, as flutter_js polyfills it, through the extension's client ----

  class Headers {
    constructor(init) {
      this.map = new Map();
      const entries =
        init instanceof Headers
          ? [...init.map.entries()]
          : Array.isArray(init)
            ? init
            : Object.entries(init && typeof init === 'object' ? init : {});
      for (const [name, value] of entries) this.map.set(str(name).toLowerCase(), str(value));
    }
    get(name) {
      return this.map.get(str(name).toLowerCase()) ?? null;
    }
    has(name) {
      return this.map.has(str(name).toLowerCase());
    }
    set(name, value) {
      this.map.set(str(name).toLowerCase(), str(value));
    }
    append(name, value) {
      const key = str(name).toLowerCase();
      this.map.set(key, this.map.has(key) ? `${this.map.get(key)}, ${str(value)}` : str(value));
    }
    forEach(callback) {
      for (const [name, value] of this.map) callback(value, name, this);
    }
    entries() {
      return this.map.entries();
    }
    keys() {
      return this.map.keys();
    }
    values() {
      return this.map.values();
    }
    [Symbol.iterator]() {
      return this.map.entries();
    }
  }
  define('Headers', Headers);
  const respond = (status, url, headers, text) => ({
    ok: status >= 200 && status < 300,
    status,
    statusText: '',
    url,
    redirected: false,
    headers: new Headers(headers),
    text: async () => text,
    json: async () => JSON.parse(text),
    clone: () => respond(status, url, headers, text),
  });
  define('fetch', async (input, init = {}) => {
    const url = input && typeof input === 'object' && 'url' in input ? str(input.url) : str(input);
    const headers = {};
    new Headers(init?.headers).forEach((value, name) => {
      headers[name] = value;
    });
    const body = init?.body === undefined || init?.body === null ? null : String(init.body);
    const response = await host.http.fetch(url, {
      method: str(init?.method || 'GET').toUpperCase(),
      headers,
      body,
    });
    return respond(response.status, response.url, response.headers, await response.text());
  });

  // ---- this.request: Dio's request, as Miru makes it ----

  let lastRequestUrl = '';
  const hostOf = (url) => /^[a-z][a-z0-9+.-]*:\/\/([^/?#]+)/i.exec(url)?.[1] ?? url;
  // Dart's Uri.encodeQueryComponent: spaces as `+`, and `!'()` escaped too.
  const component = (value) =>
    encodeURIComponent(value)
      .replace(/%20/g, '+')
      .replace(/[!'()]/g, (char) => `%${char.charCodeAt(0).toString(16).toUpperCase()}`);
  const formEncode = (data, prefix) =>
    Object.entries(data).flatMap(([key, value]) => {
      const name = prefix ? `${prefix}[${key}]` : key;
      if (value === undefined) return [];
      if (value === null) return [component(name)];
      if (Array.isArray(value))
        return value.map((item) => `${component(name)}=${component(str(item))}`);
      if (typeof value === 'object') return formEncode(value, name);
      return [`${component(name)}=${component(String(value))}`];
    });
  const isJson = (type) => {
    const mime = str(type).split(';')[0].trim().toLowerCase();
    return mime === 'application/json' || mime === 'text/json' || mime.endsWith('+json');
  };
  const send = async (address, options) => {
    const headers = {};
    for (const [name, value] of Object.entries(options.headers ?? {})) {
      if (value !== undefined && value !== null) headers[name] = str(value);
    }
    let url = str(address);
    const query = options.queryParameters;
    if (query && typeof query === 'object') {
      const encoded = formEncode(query).join('&');
      if (encoded) url += (url.includes('?') ? '&' : '?') + encoded;
    }
    let body = null;
    const data = options.data;
    if (data !== undefined && data !== null) {
      // Dio sends maps, lists and strings as JSON unless told otherwise.
      let typeName = Object.keys(headers).find((name) => name.toLowerCase() === 'content-type');
      if (typeName === undefined) {
        typeName = 'Content-Type';
        headers[typeName] = 'application/json; charset=utf-8';
      }
      if (typeof data !== 'string' && isJson(headers[typeName])) body = JSON.stringify(data);
      else if (typeof data === 'object' && !Array.isArray(data)) body = formEncode(data).join('&');
      else body = str(data);
    }
    const response = await host.http.fetch(url, {
      method: str(options.method || 'get').toUpperCase(),
      headers,
      body,
    });
    const text = await response.text();
    if (!response.ok)
      throw new Error(`${hostOf(response.url || url)} answered HTTP ${response.status}`);
    try {
      return JSON.parse(text);
    } catch {
      return text;
    }
  };

  /** Miru's headers for covers, pages and videos a script gives none for. */
  const defaultHeaders = () => {
    const headers = { 'User-Agent': native.userAgent(info.webSite || '') };
    if (lastRequestUrl) headers.Referer = lastRequestUrl;
    const cookie = info.webSite ? native.cookies(info.webSite) : '';
    if (cookie) headers.Cookie = cookie;
    return headers;
  };
  const headersOf = (given, fallback) => {
    if (!given || typeof given !== 'object') return fallback();
    const headers = {};
    for (const [name, value] of Object.entries(given)) {
      if (value !== undefined && value !== null) headers[name] = str(value);
    }
    return headers;
  };

  // ---- HTML: Miru's Element and XPathNode, answered by Rust ----

  class Element {
    constructor(content, selector) {
      this.content = content;
      this.selector = selector || '';
    }
    async querySelector(selector) {
      return new Element(await this.excute(), selector);
    }
    async excute(fun) {
      return unwrap(native.select(str(this.content), str(this.selector), str(fun)));
    }
    execute(fun) {
      return this.excute(fun);
    }
    async removeSelector(selector) {
      this.content = unwrap(native.remove(await this.outerHTML, str(selector)));
      return this;
    }
    async getAttributeText(attr) {
      return unwrap(native.attr(await this.outerHTML, str(this.selector), str(attr)));
    }
    get text() {
      return this.excute('text');
    }
    get outerHTML() {
      return this.excute('outerHTML');
    }
    get innerHTML() {
      return this.excute('innerHTML');
    }
  }

  /** Dart's List.toString, which Miru hands scripts for `allHTML`; they index it as a list. */
  const dartList = (items) =>
    Object.defineProperty(items, 'toString', { value: () => `[${items.join(', ')}]` });

  class XPathNode {
    constructor(content, selector) {
      this.content = content;
      this.selector = selector;
    }
    async excute(fun) {
      const value = unwrap(native.xpath(str(this.content), str(this.selector), str(fun)));
      return fun === 'allHTML' ? dartList(value) : value;
    }
    execute(fun) {
      return this.excute(fun);
    }
    get attr() {
      return this.excute('attr');
    }
    get attrs() {
      return this.excute('attrs');
    }
    get text() {
      return this.excute('text');
    }
    get allHTML() {
      return this.excute('allHTML');
    }
    get outerHTML() {
      return this.excute('outerHTML');
    }
  }

  // ---- Settings: registered while loading, shown with the extension's other settings ----

  const MAX_LABEL_CHARS = 80;
  const settings = new Map();
  const label = (text) => str(text).trim().replace(/\s+/g, ' ').slice(0, MAX_LABEL_CHARS);
  const radioOptions = (options) => {
    const seen = new Set();
    const pairs = [];
    for (const [name, value] of Object.entries(
      options && typeof options === 'object' ? options : {},
    )) {
      const shown = label(name) || label(value) || '—';
      if (seen.has(shown)) continue;
      seen.add(shown);
      pairs.push([shown, str(value)]);
    }
    return pairs;
  };

  const WEB_VIEW =
    "This source wants its site opened in a web view to get past a bot check, which Raygun doesn't do";

  class Extension {
    package = info.package;
    name = info.name;
    settingKeys = [];

    querySelector(content, selector) {
      return new Element(content, selector);
    }
    async request(url, options) {
      options = options || {};
      options.headers = options.headers || {};
      const miruUrl = options.headers['Miru-Url'] || info.webSite;
      options.method = options.method || 'get';
      // Miru joins the base and the address as text; a full address joined that way can never
      // be reached (`https://site/basehttps://…`), so it is taken as it is.
      const address = /^https?:\/\//i.test(str(url)) ? str(url) : miruUrl + url;
      lastRequestUrl = address;
      return send(address, options);
    }
    queryXPath(content, selector) {
      return new XPathNode(content, selector);
    }
    async querySelectorAll(content, selector) {
      return unwrap(native.selectAll(str(content), str(selector))).map(
        (html) => new Element(html, selector),
      );
    }
    async getAttributeText(content, selector, attr) {
      return unwrap(native.attr(str(content), str(selector), str(attr)));
    }
    popular() {
      throw new Error('not implement popular');
    }
    latest() {
      throw new Error('not implement latest');
    }
    search() {
      throw new Error('not implement search');
    }
    createFilter() {
      throw new Error('not implement createFilter');
    }
    detail() {
      throw new Error('not implement detail');
    }
    watch() {
      throw new Error('not implement watch');
    }
    checkUpdate() {
      throw new Error('not implement checkUpdate');
    }
    async getSetting(key) {
      const setting = settings.get(str(key));
      if (!setting) return null;
      const stored = host.settings.get(setting.key);
      switch (setting.type) {
        case 'toggle':
          return typeof stored === 'boolean' ? String(stored) : setting.defaultValue;
        case 'radio': {
          const chosen = setting.options.find(([shown]) => shown === stored);
          return chosen ? chosen[1] : setting.defaultValue;
        }
        default:
          return typeof stored === 'string' ? stored : setting.defaultValue;
      }
    }
    async registerSetting(setting) {
      const key = str(setting?.key).trim();
      if (!key) throw new Error('a setting needs a key');
      const type = ['input', 'radio', 'toggle'].includes(setting.type) ? setting.type : 'input';
      const registered = {
        key,
        title: label(setting.title) || key,
        type,
        defaultValue: str(setting.defaultValue),
        options: type === 'radio' ? radioOptions(setting.options) : [],
      };
      unwrap(native.registerSetting(JSON.stringify(registered)));
      settings.set(key, registered);
      this.settingKeys.push(key);
    }
    async load() {}
    async openWebView() {
      throw new Error(WEB_VIEW);
    }
  }
  define('Extension', Extension);
  define('Element', Element);
  define('XPathNode', XPathNode);

  // ---- The script as a Raygun source ----

  const KINDS = { bangumi: 'video.source', manga: 'manga.source', fikushon: 'novel.source' };
  /** Calls a script can't answer without, in words. */
  const MISSING = {
    latest: 'This source has no list of titles: search it instead',
    popular: 'This source has no list of titles: search it instead',
    search: "This source can't search",
    detail: "This source can't open titles",
    watch: "This source can't open episodes or chapters",
  };
  /** How long loading waits for the script's `load()`; later calls wait for the rest. */
  const LOAD_WAIT_MS = 250;
  const DETAIL_REUSE_MS = 60_000;
  const FILTER_REUSE_MS = 600_000;
  const SEASON = /season|staffel|saison|temporada|stagione|сезон|第.+季|シーズン|^s\d+$/i;
  const IMAGE = /^https?:\/\/\S+\.(?:jpe?g|png|webp|gif|avif)(?:\?\S*)?$/i;
  /** A select's value standing for an option whose key is empty, or a default it doesn't list. */
  const DEFAULT_VALUE = '__miru_default__';

  const escapeHtml = (text) =>
    str(text)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  /** Keeps a map to its newest `limit` entries. */
  const trim = (map, limit) => {
    while (map.size > limit) map.delete(map.keys().next().value);
  };
  const originOf = (url) => /^(https?:\/\/[^/?#]+)/i.exec(url)?.[1];

  /** The number in an episode's or chapter's name: "Episode 12", "第12集", "Ch. 3.5", "07". */
  const NUMBERS = [
    /s\d+\s*e(\d+(?:\.\d+)?)/i,
    /第\s*(\d+(?:\.\d+)?)\s*[集话話章回]/,
    /(?:^|[^\p{L}])(?:episodes?|eps?|chapters?|chap|ch|cap[íi]tulo|episodio|épisode|folge|chương|tập|bab|глава|серия)\.?\s*(\d+(?:\.\d+)?)/iu,
    /(\d+(?:\.\d+)?)\s*[集话話章回]/,
    // A short name around one number: "01", "EP 07", "Part 2".
    /^\D{0,8}(\d+(?:\.\d+)?)\D{0,8}$/,
  ];
  const numberIn = (name) => {
    for (const pattern of NUMBERS) {
      const found = pattern.exec(name);
      if (found) return Number(found[1]);
    }
    return null;
  };
  /**
   * Numbers for a list in Miru's order (its reading order): from the names when every name has
   * one, else the place in the list.
   */
  const numbered = (units) => {
    const parsed = units.map((unit) => (unit.name ? numberIn(unit.name) : null));
    const named = parsed.every((number) => number !== null && Number.isFinite(number));
    units.forEach((unit, index) => {
      unit.number = named ? parsed[index] : index + 1;
    });
    return units;
  };

  Object.defineProperty(globalThis, '__miruStart', {
    configurable: true,
    value: (instance) => {
      delete globalThis.__miruStart;
      const kind = KINDS[info.type];
      const overrides = (name) =>
        typeof instance[name] === 'function' && instance[name] !== Extension.prototype[name];

      // Miru starts `load()` without waiting; calls here wait for it, failed or not.
      const loading = (async () => {
        try {
          await instance.load();
        } catch (error) {
          console.warn(`${info.name} failed to set up: ${message(error)}`);
        }
      })();

      const call = async (run) => {
        await loading;
        try {
          return await run();
        } catch (error) {
          const missing = /^not implement (\w+)/.exec(message(error));
          if (missing) throw new Error(MISSING[missing[1]] ?? message(error));
          throw error instanceof Error ? error : new Error(message(error));
        }
      };

      // Sources that ignore the page number send the same titles again: that's the end.
      const listed = new Map();
      const listing = async (key, page, run) => {
        const fallback = (() => {
          let headers;
          return () => (headers ??= defaultHeaders());
        })();
        const found = await call(run);
        const items = (Array.isArray(found) ? found : [])
          .filter((item) => item && item.url !== undefined && item.url !== null)
          .map((item) => ({
            id: str(item.url),
            title: str(item.title).trim(),
            coverUrl: item.cover ? str(item.cover) : null,
            coverHeaders: headersOf(item.headers, fallback),
          }));
        const pageNumber = Number(page) || 1;
        if (pageNumber <= 1 || !listed.has(key)) {
          listed.delete(key);
          listed.set(key, new Set(items.map((item) => item.id)));
          trim(listed, 20);
        } else {
          const seen = listed.get(key);
          if (items.length > 0 && items.every((item) => seen.has(item.id))) {
            return { items: [], hasNextPage: false };
          }
          for (const item of items) seen.add(item.id);
        }
        return { items, hasNextPage: items.length > 0 };
      };

      const details = new Map();
      const detailOf = (id) => {
        const cached = details.get(id);
        if (cached && Date.now() - cached.at < DETAIL_REUSE_MS) return cached.promise;
        const promise = call(() => instance.detail(id));
        details.set(id, { at: Date.now(), promise });
        trim(details, 20);
        promise.catch(() => details.delete(id));
        return promise;
      };
      const groupsOf = (detail) =>
        (Array.isArray(detail?.episodes) ? detail.episodes : []).map((group, index) => ({
          title: str(group?.title).trim() || String(index + 1),
          urls: (Array.isArray(group?.urls) ? group.urls : [])
            .filter((unit) => unit && unit.url !== undefined && unit.url !== null)
            .map((unit) => ({ name: str(unit.name).trim(), url: str(unit.url) })),
        }));

      // createFilter's answer, kept a while: the Filters sheet and every search use it.
      let filterCache = null;
      const encode = (value) => (value === '' || value.trim() !== value ? null : value);
      const filterDefs = async () => {
        if (!overrides('createFilter')) return [];
        if (filterCache && Date.now() - filterCache.at < FILTER_REUSE_MS) return filterCache.defs;
        const answer = await call(() => instance.createFilter());
        const defs = Object.entries(answer && typeof answer === 'object' ? answer : {})
          .slice(0, 20)
          .flatMap(([key, filter], index) => {
            if (!filter || typeof filter !== 'object') return [];
            const values = new Map();
            const options = [];
            for (const [value, shown] of Object.entries(
              filter.options && typeof filter.options === 'object' ? filter.options : {},
            )) {
              const raygunValue = encode(value) ?? `${DEFAULT_VALUE}${index}_${options.length}`;
              values.set(raygunValue, value);
              options.push({ value: raygunValue, label: label(shown) || value || '—' });
            }
            const fallback = str(filter.default);
            let defaultValue = [...values.entries()].find(([, value]) => value === fallback)?.[0];
            if (defaultValue === undefined) {
              defaultValue = `${DEFAULT_VALUE}${index}`;
              values.set(defaultValue, fallback);
              options.unshift({ value: defaultValue, label: label(fallback) || 'All' });
            }
            const max = Number(filter.max ?? 1);
            const raygunKey = /^[A-Za-z0-9_-]{1,40}$/.test(key) ? key : `f${index}`;
            return [
              {
                key,
                max: Number.isFinite(max) && max > 0 ? max : 1,
                fallback,
                values,
                raygun:
                  max > 1
                    ? { kind: 'tags', key: raygunKey, label: label(filter.title) || key, options }
                    : {
                        kind: 'select',
                        key: raygunKey,
                        label: label(filter.title) || key,
                        options,
                        default: defaultValue,
                      },
              },
            ];
          });
        filterCache = { at: Date.now(), defs };
        return defs;
      };
      /** Raygun's chosen filters as Miru's `{ key: [option, …] }`, defaults where none is chosen. */
      const miruFilter = async (chosen) => {
        let defs;
        try {
          defs = await filterDefs();
        } catch (error) {
          console.warn(`createFilter failed: ${message(error)}`);
          return null;
        }
        if (defs.length === 0) return null;
        const values = chosen && typeof chosen === 'object' ? chosen : {};
        const filter = {};
        for (const def of defs) {
          const value = values[def.raygun.key];
          const decode = (option) => def.values.get(option) ?? option;
          if (def.raygun.kind === 'select') {
            filter[def.key] = [typeof value === 'string' ? decode(value) : def.fallback];
          } else {
            const include = Array.isArray(value?.include) ? value.include.map(decode) : [];
            filter[def.key] = include.length > 0 ? include.slice(-def.max) : [def.fallback];
          }
        }
        return filter;
      };

      /** Names from the first of `keys` a detail gives: a string, a list, or `{ name }`s. */
      const names = (detail, keys) => {
        for (const key of keys) {
          const value = detail[key];
          const list = (Array.isArray(value) ? value : [value])
            .map((entry) => str(entry && typeof entry === 'object' ? entry.name : entry).trim())
            .filter((name) => name && name !== info.name);
          if (list.length > 0) return list;
        }
        return [];
      };
      /**
       * Who made it, when the detail says: authors and artists for comics and books; for videos a
       * studio or director (Raygun's directors) and the cast.
       */
      const people = (detail) =>
        kind === 'video.source'
          ? {
              cast: names(detail, ['cast', 'actors', 'actor']),
              directors: names(detail, [
                'studio',
                'studios',
                'director',
                'directors',
                'author',
                'authors',
              ]),
            }
          : {
              authors: names(detail, ['author', 'authors', 'writer']),
              artists: names(detail, ['artist', 'artists', 'illustrator']),
            };

      const provider = {
        kind,
        id: 'source',
        name: info.name,
        popular: ({ page = 1 } = {}) =>
          listing('popular', page, () =>
            overrides('popular') ? instance.popular(page) : instance.latest(page),
          ),
        latest: ({ page = 1 } = {}) => listing('latest', page, () => instance.latest(page)),
        search: async ({ query = '', page = 1, filters } = {}) => {
          await loading;
          const filter = await miruFilter(filters);
          return listing(`search\u0000${query}\u0000${JSON.stringify(filter)}`, page, () =>
            instance.search(str(query), page, filter),
          );
        },
        filters: async () => (await filterDefs()).map((def) => def.raygun),
        details: async ({ id }) => {
          const detail = (await detailOf(str(id))) ?? {};
          const origin = originOf(info.webSite);
          const page = /^https?:\/\//i.test(str(id))
            ? str(id)
            : origin && str(id).startsWith('/')
              ? origin + str(id)
              : null;
          return {
            id: str(id),
            title: str(detail.title).trim(),
            coverUrl: detail.cover ? str(detail.cover) : null,
            coverHeaders: headersOf(detail.headers, defaultHeaders),
            description: detail.desc ? str(detail.desc) : null,
            url: page,
            // Miru's detail has no people of its own; scripts that add them name them like this.
            // The source's name never stands in for them.
            ...people(detail),
          };
        },
        // Which of Miru's methods the script has (for reports and surveys).
        implemented: async () =>
          [
            'latest',
            'popular',
            'search',
            'createFilter',
            'detail',
            'watch',
            'load',
            'checkUpdate',
          ].filter(overrides),
      };

      if (kind === 'video.source') {
        // An episode's servers: the episode list's groups that have it (lines, qualities).
        const servers = new Map();
        provider.episodes = async ({ id }) => {
          const groups = groupsOf(await detailOf(str(id)));
          const seasons = groups.length > 1 && groups.every((group) => SEASON.test(group.title));
          const episodes = [];
          const byName = new Map();
          const ids = new Set();
          groups.forEach((group, season) => {
            for (const unit of group.urls) {
              const known = !seasons && unit.name ? byName.get(unit.name) : undefined;
              if (known && !known.groups.has(season)) {
                known.groups.add(season);
                known.servers.push({ id: unit.url, name: group.title });
                continue;
              }
              if (ids.has(unit.url)) continue;
              ids.add(unit.url);
              const episode = {
                id: unit.url,
                name: unit.name,
                season: seasons ? season + 1 : undefined,
                servers: [{ id: unit.url, name: group.title }],
                groups: new Set([season]),
              };
              episodes.push(episode);
              if (unit.name && !known) byName.set(unit.name, episode);
            }
          });
          for (const episode of episodes) {
            servers.delete(episode.id);
            servers.set(episode.id, episode.servers);
          }
          trim(servers, 5000);
          return {
            episodes: numbered(episodes).map((episode) => ({
              id: episode.id,
              name: episode.name || null,
              number: episode.number,
              season: episode.season,
            })),
          };
        };
        provider.servers = async ({ episodeId }) => ({
          servers: (servers.get(str(episodeId)) ?? [{ id: str(episodeId), name: info.name }]).map(
            (server) => ({ id: server.id, name: server.name }),
          ),
        });
        provider.videos = async ({ serverId }) => {
          const watch = await call(() => instance.watch(str(serverId)));
          if (!watch || !watch.url) throw new Error('The source found no video for this episode');
          const url = str(watch.url);
          const type = str(watch.type).toLowerCase();
          const torrent = type === 'torrent' || /^magnet:/i.test(url);
          const format = torrent
            ? 'torrent'
            : type === 'hls' || /\.m3u8(?:[?#]|$)/i.test(url)
              ? 'hls'
              : /\.mpd(?:[?#]|$)/i.test(url)
                ? 'dash'
                : null;
          if (watch.audioTrack) {
            console.warn('this video has a separate audio track, which Raygun plays without');
          }
          return {
            videos: [
              {
                url,
                headers: headersOf(watch.headers, defaultHeaders),
                format,
                subtitles: (Array.isArray(watch.subtitles) ? watch.subtitles : [])
                  .filter((track) => track && track.url)
                  .map((track) => ({
                    url: str(track.url),
                    label: track.title ? str(track.title) : null,
                    lang: track.language ? str(track.language) : null,
                  })),
              },
            ],
          };
        };
      } else {
        provider.chapters = async ({ id }) => {
          const groups = groupsOf(await detailOf(str(id)));
          const several = groups.length > 1;
          const ids = new Set();
          const chapters = [];
          for (const group of groups) {
            for (const unit of group.urls) {
              if (ids.has(unit.url)) continue;
              ids.add(unit.url);
              chapters.push({
                id: unit.url,
                name: unit.name || null,
                scanlator: several ? group.title : null,
              });
            }
          }
          return { chapters: numbered(chapters) };
        };
      }
      if (kind === 'manga.source') {
        provider.pages = async ({ chapterId }) => {
          const watch = await call(() => instance.watch(str(chapterId)));
          const headers = headersOf(watch?.headers, defaultHeaders);
          return {
            pages: (Array.isArray(watch?.urls) ? watch.urls : [])
              .filter((url) => url !== undefined && url !== null && url !== '')
              .map((url) => ({ url: str(url), headers })),
          };
        };
      }
      if (kind === 'novel.source') {
        provider.passage = async ({ chapterId }) => {
          const watch = await call(() => instance.watch(str(chapterId)));
          const parts = [];
          if (watch?.subtitle) parts.push(`<h3>${escapeHtml(watch.subtitle)}</h3>`);
          for (const paragraph of Array.isArray(watch?.content) ? watch.content : []) {
            const text = str(paragraph);
            if (IMAGE.test(text.trim())) {
              parts.push(`<img src="${escapeHtml(text.trim())}">`);
            } else {
              parts.push(`<p>${text.split(/\r?\n/).map(escapeHtml).join('<br>')}</p>`);
            }
          }
          return { html: parts.join('') };
        };
      }

      host.providers.register(provider);
      return Promise.race([loading, wait(LOAD_WAIT_MS)]);
    },
  });
})();
