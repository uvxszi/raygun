// Runs one extension in the browser (docs/WEB.md). Each gets an iframe sandboxed without
// `allow-same-origin`: an opaque origin with no access to Raygun's storage, cookies or page, and
// whose requests go out as a page's do (CORS applies). Inside, the `raygun` host API is built as
// the app's QuickJS prelude builds it (crates/raygun-ext-host/src/prelude.js), over the browser's
// fetch, DOMParser and timers; the page talks to it by postMessage.

/** What the page tells a sandbox when it loads an extension. */
export type LoadRequest = {
  id: string;
  source: string;
  /** The manifest's `permissions.net`. */
  net: string[];
  settings: Record<string, unknown>;
  storage: Record<string, unknown>;
  showNsfw: boolean;
};

export type RegisteredProvider = {
  kind: string;
  id: string;
  name?: string | null;
  searchPill?: string | null;
};

/** The sandbox's side, as one document. Kept to plain ES2020 that every browser runs. */
export const SANDBOX_PAGE = `<!doctype html><meta charset="utf-8"><script>
'use strict';
(() => {
  const send = (message) => parent.postMessage(Object.assign({ raygunSandbox: true }, message), '*');
  const providers = new Map();
  let state = null;

  const allowed = (url) => {
    let host;
    try {
      const parsed = new URL(url);
      if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') return false;
      host = parsed.hostname.toLowerCase();
    } catch (error) {
      return false;
    }
    return state.net.some((pattern) => {
      const rule = String(pattern).toLowerCase();
      if (rule === '*') return true;
      if (rule.startsWith('*.')) return host === rule.slice(2) || host.endsWith(rule.slice(1));
      return host === rule;
    });
  };

  const looksTextual = (type) => /^text\\/|json|xml|javascript|html|x-www-form-urlencoded/i.test(type || '');

  const fromBase64 = (text) => Uint8Array.from(atob(text), (char) => char.charCodeAt(0));

  const toResponse = (raw) => Object.freeze({
    status: raw.status,
    ok: raw.status >= 200 && raw.status < 300,
    url: raw.url,
    headers: raw.headers,
    text: async () => raw.body,
    json: async () => JSON.parse(raw.body),
    bytes: async () => raw.bytes || null,
  });

  // MediaWiki's APIs answer web pages only when asked with origin=* (anonymous requests).
  const WIKI = /(^|\\.)(wikipedia|wikimedia|wikibooks|wikisource|wikidata|wiktionary|wikiquote|wikiversity|wikivoyage|mediawiki)\\.org$/;
  const forBrowsers = (address) => {
    try {
      const parsed = new URL(address);
      if (WIKI.test(parsed.hostname) && /\\/api\\.php$/.test(parsed.pathname) && !parsed.searchParams.has('origin')) {
        parsed.searchParams.set('origin', '*');
        return parsed.href;
      }
    } catch (error) {
      return address;
    }
    return address;
  };

  const httpFetch = async (url, init) => {
    init = init || {};
    const address = forBrowsers(String(url));
    if (!allowed(address)) {
      throw new Error('this extension may not reach ' + address + ' (its permissions)');
    }
    // The browser names itself: a User-Agent of the source's own would only make the browser ask
    // the site first (a preflight), which many sites never answer.
    const sent = {};
    for (const [name, value] of Object.entries(init.headers || {})) {
      if (String(name).toLowerCase() !== 'user-agent') sent[name] = value;
    }
    const options = {
      method: init.method || 'GET',
      headers: sent,
      credentials: 'omit',
      redirect: init.redirect === 'manual' ? 'manual' : 'follow',
    };
    if (init.bodyBase64 != null) options.body = fromBase64(String(init.bodyBase64));
    else if (init.body != null) options.body = init.body;
    let response;
    try {
      response = await fetch(address, options);
    } catch (error) {
      const host = new URL(address).hostname;
      throw new Error(host + " doesn't let web pages read it, so this source can't work in a browser");
    }
    const headers = {};
    response.headers.forEach((value, key) => { headers[key] = value; });
    const type = response.headers.get('content-type') || '';
    const raw = { status: response.status, url: response.url || address, headers, body: '', bytes: null };
    if (init.binary && !looksTextual(type)) raw.bytes = new Uint8Array(await response.arrayBuffer());
    else raw.body = await response.text();
    return toResponse(raw);
  };

  const elementOf = (element) => {
    const attrs = {};
    for (const attribute of Array.from(element.attributes)) attrs[attribute.name] = attribute.value;
    const html = element.outerHTML;
    return Object.freeze({
      text: element.textContent || '',
      html,
      inner: element.innerHTML,
      attrs,
      attr: (name) => (attrs[name] === undefined ? null : attrs[name]),
      select: (sub) => select(html, sub, true),
    });
  };

  const select = (html, selector, fragment) => {
    let root;
    if (fragment) {
      const template = document.createElement('template');
      template.innerHTML = String(html);
      root = template.content;
    } else {
      root = new DOMParser().parseFromString(String(html), 'text/html');
    }
    return Array.from(root.querySelectorAll(String(selector))).map(elementOf);
  };

  const log = {};
  for (const level of ['debug', 'info', 'warn', 'error']) {
    log[level] = (...args) => send({ type: 'log', level, text: args.map((arg) => {
      if (typeof arg === 'string') return arg;
      if (arg instanceof Error) return arg.name + ': ' + arg.message;
      try { return JSON.stringify(arg); } catch (error) { return String(arg); }
    }).join(' ') });
  }

  const noYouTube = async () => {
    throw new Error("YouTube's own API isn't available to sources in a browser");
  };

  const install = () => {
    globalThis.raygun = Object.freeze({
      apiVersion: 1,
      log: Object.freeze(log),
      http: Object.freeze({ fetch: httpFetch }),
      html: Object.freeze({ select: (html, selector) => select(html, selector, false) }),
      time: Object.freeze({
        sleep: (ms) => new Promise((resolve) => setTimeout(resolve, Math.min(10000, Math.max(0, Number(ms) || 0)))),
      }),
      storage: Object.freeze({
        get: (key) => (Object.prototype.hasOwnProperty.call(state.storage, key) ? state.storage[key] : null),
        set: (key, value) => {
          const kept = JSON.parse(JSON.stringify(value === undefined ? null : value));
          state.storage[String(key)] = kept;
          send({ type: 'storage', key: String(key), value: kept });
        },
        remove: (key) => {
          delete state.storage[String(key)];
          send({ type: 'storage', key: String(key), value: null, removed: true });
        },
        keys: () => Object.keys(state.storage),
      }),
      settings: Object.freeze({
        get: (key) => (Object.prototype.hasOwnProperty.call(state.settings, key) ? state.settings[key] : null),
      }),
      content: Object.freeze({ showNsfw: () => state.showNsfw === true }),
      youtube: Object.freeze({
        search: noYouTube, stream: noYouTube, playlist: noYouTube,
        liveSearch: noYouTube, channelLive: noYouTube, live: noYouTube,
      }),
      providers: Object.freeze({
        register: (provider) => {
          const { kind, id, name, searchPill } = provider || {};
          if (typeof kind !== 'string' || typeof id !== 'string') throw new Error('a provider needs a kind and an id');
          providers.set(id, provider);
          send({ type: 'provider', provider: { kind, id, name: name || null, searchPill: searchPill || null } });
        },
      }),
      needsSetup: (reason) => {
        const said = reason === undefined || reason === null ? '' : String(reason);
        throw new Error(('[raygun:needs-setup] ' + said).trim());
      },
    });
    globalThis.console = Object.freeze({ log: log.info, info: log.info, debug: log.debug, warn: log.warn, error: log.error });
  };

  window.addEventListener('message', async (event) => {
    if (event.source !== parent) return;
    const message = event.data || {};
    if (message.type === 'load' && state === null) {
      state = message.request;
      try {
        install();
        (0, eval)(state.source);
        send({ type: 'loaded' });
      } catch (error) {
        send({ type: 'loaded', error: String((error && error.message) || error) });
      }
      return;
    }
    if (message.type === 'call') {
      const provider = providers.get(message.provider);
      try {
        if (!provider) throw new Error('no provider ' + message.provider);
        if (typeof provider[message.method] !== 'function') {
          throw new Error('provider ' + message.provider + ' has no method ' + message.method);
        }
        const result = await provider[message.method](message.args);
        send({ type: 'answer', id: message.id, result: JSON.parse(JSON.stringify(result === undefined ? null : result)) });
      } catch (error) {
        send({ type: 'answer', id: message.id, error: String((error && error.message) || error) });
      }
    }
  });
  send({ type: 'ready' });
})();
</script>`;

type SandboxMessage =
  | { raygunSandbox: true; type: 'ready' }
  | { raygunSandbox: true; type: 'loaded'; error?: string }
  | { raygunSandbox: true; type: 'provider'; provider: RegisteredProvider }
  | { raygunSandbox: true; type: 'storage'; key: string; value: unknown; removed?: boolean }
  | { raygunSandbox: true; type: 'log'; level: string; text: string }
  | { raygunSandbox: true; type: 'answer'; id: number; result?: unknown; error?: string };

/** How long one provider call may take. */
const CALL_TIMEOUT_MS = 30_000;
const LOAD_TIMEOUT_MS = 15_000;

/** One extension running in its own sandboxed iframe. */
export class Sandbox {
  readonly providers: RegisteredProvider[] = [];
  private readonly frame: HTMLIFrameElement;
  private readonly calls = new Map<
    number,
    { resolve: (value: unknown) => void; reject: (error: Error) => void }
  >();
  private nextCall = 1;
  private readonly listener: (event: MessageEvent) => void;
  private loaded: Promise<void>;

  constructor(
    request: LoadRequest,
    /** A storage change the extension made (kept by the page). */
    onStorage: (key: string, value: unknown, removed: boolean) => void,
  ) {
    this.frame = document.createElement('iframe');
    this.frame.setAttribute('sandbox', 'allow-scripts');
    this.frame.setAttribute('aria-hidden', 'true');
    this.frame.title = `Raygun source ${request.id}`;
    this.frame.style.display = 'none';
    let ready: () => void = () => undefined;
    let done: (error?: string) => void = () => undefined;
    const readyNow = new Promise<void>((resolve) => {
      ready = resolve;
    });
    this.loaded = new Promise<void>((resolve, reject) => {
      done = (error) => (error ? reject(new Error(error)) : resolve());
      setTimeout(() => reject(new Error(`${request.id} didn't load in time`)), LOAD_TIMEOUT_MS);
    });
    this.listener = (event: MessageEvent) => {
      if (event.source !== this.frame.contentWindow) return;
      const message = event.data as SandboxMessage;
      if (!message || message.raygunSandbox !== true) return;
      switch (message.type) {
        case 'ready':
          ready();
          break;
        case 'loaded':
          done(message.error);
          break;
        case 'provider':
          this.providers.push(message.provider);
          break;
        case 'storage':
          onStorage(message.key, message.value, message.removed === true);
          break;
        case 'log':
          console.debug(`[${request.id}] ${message.text}`);
          break;
        case 'answer': {
          const call = this.calls.get(message.id);
          this.calls.delete(message.id);
          if (!call) break;
          if (message.error !== undefined) call.reject(new Error(message.error));
          else call.resolve(message.result);
          break;
        }
      }
    };
    window.addEventListener('message', this.listener);
    this.frame.srcdoc = SANDBOX_PAGE;
    document.body.appendChild(this.frame);
    void readyNow.then(() => this.frame.contentWindow?.postMessage({ type: 'load', request }, '*'));
  }

  /** Resolves once the extension ran and registered its providers. */
  ready(): Promise<void> {
    return this.loaded;
  }

  async call<T>(provider: string, method: string, args: unknown): Promise<T> {
    await this.loaded;
    const id = this.nextCall++;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.calls.delete(id);
        reject(new Error(`${provider} didn't answer in time`));
      }, CALL_TIMEOUT_MS);
      this.calls.set(id, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value as T);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      this.frame.contentWindow?.postMessage({ type: 'call', id, provider, method, args }, '*');
    });
  }

  dispose() {
    window.removeEventListener('message', this.listener);
    for (const call of this.calls.values()) call.reject(new Error('the source was turned off'));
    this.calls.clear();
    this.frame.remove();
  }
}
