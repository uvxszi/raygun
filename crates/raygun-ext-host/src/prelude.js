// Builds the `raygun` host API (ARCHITECTURE §7.3) over the native functions in `__rg`, then
// removes `__rg` so extensions only see the documented surface. Runs before every extension.
'use strict';
(() => {
  const native = globalThis.__rg;
  delete globalThis.__rg;

  // Natives answer with a JSON envelope: { value } on success, { error } otherwise.
  const unwrap = (text) => {
    const envelope = JSON.parse(text);
    if (envelope.error !== undefined) {
      throw new Error(envelope.error);
    }
    return envelope.value;
  };

  const format = (args) =>
    args
      .map((arg) => {
        if (typeof arg === 'string') return arg;
        if (arg instanceof Error) return `${arg.name}: ${arg.message}`;
        try {
          return JSON.stringify(arg);
        } catch {
          return String(arg);
        }
      })
      .join(' ');

  const log = {};
  for (const level of ['debug', 'info', 'warn', 'error']) {
    log[level] = (...args) => native.log(level, format(args));
  }

  const toResponse = (raw) =>
    Object.freeze({
      status: raw.status,
      ok: raw.status >= 200 && raw.status < 300,
      url: raw.url,
      headers: raw.headers,
      text: async () => raw.body,
      json: async () => JSON.parse(raw.body),
      // The answer's bytes (a `Uint8Array`) when `binary` asked for them and they aren't text;
      // otherwise null, and the text is the answer.
      bytes: async () => raw.bytes ?? null,
    });

  const select = (html, selector, fragment) =>
    unwrap(native.htmlSelect(String(html), String(selector), fragment)).map((element) =>
      Object.freeze({
        ...element,
        attr: (name) => element.attrs[name] ?? null,
        select: (sub) => select(element.html, sub, true),
      }),
    );

  const providers = new Map();

  globalThis.raygun = Object.freeze({
    apiVersion: 1,
    log: Object.freeze(log),
    http: Object.freeze({
      // The native answers with an object ({ status, url, headers, body } or { error }), so a
      // big page is never escaped into JSON and parsed back.
      fetch: async (url, init = {}) => {
        const options = {
          method: init.method ?? 'GET',
          headers: init.headers ?? {},
          body: init.body ?? null,
        };
        // A body of bytes goes as base64; `binary` keeps an answer's bytes when they aren't text.
        if (init.bodyBase64 != null) options.bodyBase64 = String(init.bodyBase64);
        if (init.binary) options.binary = true;
        // `redirect: 'manual'`: a redirect comes back as it is (its status and `Location`).
        if (init.redirect === 'manual') options.redirect = 'manual';
        const raw = await native.fetch(String(url), JSON.stringify(options));
        if (raw.error !== undefined) {
          throw new Error(raw.error);
        }
        return toResponse(raw);
      },
    }),
    html: Object.freeze({
      select: (html, selector) => select(html, selector, false),
    }),
    // There are no timers in the runtime; this paces rate-limited APIs (at most 10 s per call).
    time: Object.freeze({
      sleep: (ms) => native.sleep(Number(ms) || 0),
    }),
    storage: Object.freeze({
      get: (key) => unwrap(native.storageGet(String(key))),
      set: (key, value) => {
        unwrap(native.storageSet(String(key), JSON.stringify(value ?? null)));
      },
      remove: (key) => {
        unwrap(native.storageRemove(String(key)));
      },
      keys: () => unwrap(native.storageKeys()),
    }),
    settings: Object.freeze({
      get: (key) => unwrap(native.settingsGet(String(key))),
    }),
    // The app-wide NSFW switch (off by default): filter adult results out while it is off.
    content: Object.freeze({
      showNsfw: () => native.showNsfw() === true,
    }),
    // YouTube through its own web API, answered in Rust (`youtube`); the extension's network
    // permissions apply. Converted Nuclear plugins reach it as `api.Ytdlp`.
    youtube: Object.freeze({
      search: async (query, limit) =>
        unwrap(await native.youtubeSearch(String(query ?? ''), Number(limit) || 0)),
      stream: async (video, options) =>
        unwrap(
          await native.youtubeStream(
            String(video ?? ''),
            JSON.stringify({
              targetKbps: typeof options?.targetKbps === 'number' ? options.targetKbps : null,
              decodable: options?.decodable === true,
            }),
          ),
        ),
      playlist: async (playlist) => unwrap(await native.youtubePlaylist(String(playlist ?? ''))),
      liveSearch: async (query, limit) =>
        unwrap(await native.youtubeLiveSearch(String(query ?? ''), Number(limit) || 0)),
      channelLive: async (channel) =>
        unwrap(await native.youtubeChannelLive(String(channel ?? ''))),
      live: async (video) => unwrap(await native.youtubeLive(String(video ?? ''))),
    }),
    providers: Object.freeze({
      register: (provider) => {
        const { kind, id, name, searchPill } = provider ?? {};
        unwrap(native.registerProvider(JSON.stringify({ kind, id, name, searchPill })));
        providers.set(id, provider);
      },
    }),
    // The extension can't work until someone sets it up: its key or sign-in is missing or was
    // refused. Throws; Raygun then asks it nothing until its settings change (the mark below is
    // what the host reads, `failure::NEEDS_SETUP_MARK`).
    needsSetup: (reason) => {
      const said = reason === undefined || reason === null ? '' : String(reason);
      throw new Error(`[raygun:needs-setup] ${said}`.trim());
    },
  });

  globalThis.console = Object.freeze({
    log: log.info,
    info: log.info,
    debug: log.debug,
    warn: log.warn,
    error: log.error,
  });

  // Rust calls providers through this: JSON in, a promise of JSON out.
  Object.defineProperty(globalThis, '__rg_call', {
    value: async (id, method, argsJson) => {
      const provider = providers.get(id);
      if (provider === undefined) {
        throw new Error(`no provider ${id}`);
      }
      if (typeof provider[method] !== 'function') {
        throw new Error(`provider ${id} has no method ${method}`);
      }
      const result = await provider[method](JSON.parse(argsJson));
      return JSON.stringify(result ?? null);
    },
  });
})();
