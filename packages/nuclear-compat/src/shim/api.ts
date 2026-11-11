// A NuclearPluginAPI built on Raygun's host API (shape from Nuclear's plugin-sdk, AGPL-3.0).
// Http, Logger, Settings, Providers and Ytdlp (YouTube, over `raygun.youtube`) work; every other
// domain is a stub that says so.
import {
  BlobPolyfill,
  HeadersPolyfill,
  ResponsePolyfill,
  URLSearchParamsPolyfill,
  bytesToBase64,
} from './polyfills';
import type { NuclearProvider, PluginInfo } from './nuclear-types';
import { KIND_MAP, wrapProvider } from './providers';
import { createYtdlp } from './ytdlp';

/** Domains Raygun doesn't offer to Nuclear plugins yet; calling into them throws. */
export const STUBBED_DOMAINS = [
  'Queue',
  'Playback',
  'Favorites',
  'Playlists',
  'Dashboard',
  'Discovery',
  'Events',
  'Shell',
  'Navigation',
  'Metadata',
  'Streaming',
  'Lyrics',
] as const;

/**
 * `domain` with a no-op for anything Raygun's copy lacks (newer plugin SDK calls), logged once,
 * instead of "not a function" stopping the whole plugin (M46.9.1).
 */
const lenient = <T extends object>(domain: string, object: T): T => {
  const warned = new Set<string>();
  return new Proxy(object, {
    get: (target, property, receiver) => {
      if (property in target || typeof property === 'symbol' || property === 'then') {
        return Reflect.get(target, property, receiver) as unknown;
      }
      return () => {
        if (!warned.has(property)) {
          warned.add(property);
          raygun.log.warn(`${domain}.${property} isn't in Raygun's Nuclear API; skipped`);
        }
        return undefined;
      };
    },
  });
};

/**
 * Calls that only listen or set up (`Events.on`, `Queue.subscribeToCurrentItem`,
 * `Playback.subscribe`): a stubbed domain takes them and never calls back, so a plugin that
 * listens while it starts still starts (Last.fm listens for played songs to scrobble them).
 */
const LISTENS = /^(on|once|off|subscribe|unsubscribe|add\w*Listener|remove\w*Listener|listen)/;

/**
 * A domain Raygun doesn't offer. Nuclear's API calls are asynchronous, so a call is refused the
 * same way: a rejected promise, which fails the provider call that awaits it and nothing else (a
 * plugin that only asks for the playing song while it starts still starts).
 */
const unsupported = (domain: string) =>
  new Proxy(
    {},
    {
      get: (_target, property) => {
        if (property === Symbol.toPrimitive || property === 'then') return undefined;
        if (typeof property === 'string' && LISTENS.test(property)) {
          return () => () => {};
        }
        return () =>
          Promise.reject(new Error(`${domain} isn't available to Nuclear plugins in Raygun yet`));
      },
    },
  );

type FetchInit = { method?: string; headers?: unknown; body?: unknown };

const headersRecord = (headers: unknown): Record<string, string> => {
  if (!headers) return {};
  if (headers instanceof HeadersPolyfill) return headers.toRecord();
  if (Array.isArray(headers)) return Object.fromEntries(headers as [string, string][]);
  if (typeof (headers as { forEach?: unknown }).forEach === 'function') {
    const record: Record<string, string> = {};
    (headers as { forEach(cb: (value: string, key: string) => void): void }).forEach(
      (value, key) => (record[key] = value),
    );
    return record;
  }
  return Object.fromEntries(
    Object.entries(headers as Record<string, unknown>).map(([key, value]) => [key, String(value)]),
  );
};

/**
 * `raygun.http.fetch` with what the host also takes beyond the SDK's types (as converted Mihon
 * sources use it): a body of bytes in base64, and an answer's bytes when they aren't text.
 */
type HostFetch = (
  url: string,
  init: {
    method: string;
    headers: Record<string, string>;
    body: string | null;
    bodyBase64?: string;
    binary?: boolean;
  },
) => Promise<{
  status: number;
  url: string;
  headers: Record<string, string>;
  text(): Promise<string>;
  bytes?: () => Promise<Uint8Array | null>;
}>;

/** A request body as `raygun.http.fetch` takes it, and the type it implies. */
const requestBody = (body: unknown): { body?: string; bodyBase64?: string; type?: string } => {
  if (body === null || body === undefined) return {};
  if (typeof body === 'string') return { body };
  if (body instanceof URLSearchParamsPolyfill) {
    return { body: body.toString(), type: 'application/x-www-form-urlencoded;charset=UTF-8' };
  }
  if (body instanceof BlobPolyfill) {
    return { bodyBase64: bytesToBase64(body.bytes), type: body.type || undefined };
  }
  if (body instanceof ArrayBuffer) return { bodyBase64: bytesToBase64(new Uint8Array(body)) };
  if (ArrayBuffer.isView(body)) {
    return {
      bodyBase64: bytesToBase64(new Uint8Array(body.buffer, body.byteOffset, body.byteLength)),
    };
  }
  return { body: String(body) };
};

/** `fetch` for plugins, over `raygun.http.fetch` (so the manifest's host permissions apply). */
export const pluginFetch = async (input: unknown, init: FetchInit = {}) => {
  const request =
    typeof input === 'object' && input !== null && 'url' in input
      ? (input as FetchInit & { url: string })
      : null;
  const url = String(request ? request.url : input);
  const headers = headersRecord(init.headers ?? request?.headers);
  const { body, bodyBase64, type } = requestBody(init.body ?? request?.body);
  if (type && !Object.keys(headers).some((name) => name.toLowerCase() === 'content-type')) {
    headers['Content-Type'] = type;
  }
  const response = await (raygun.http.fetch as HostFetch)(url, {
    method: init.method ?? request?.method ?? 'GET',
    headers,
    body: body ?? null,
    ...(bodyBase64 === undefined ? {} : { bodyBase64 }),
    // Bytes that aren't text (a picture, a zip) come as they are; text comes as text.
    binary: true,
  });
  const bytes = typeof response.bytes === 'function' ? await response.bytes() : null;
  return new ResponsePolyfill(bytes ?? (await response.text()), {
    status: response.status,
    headers: response.headers,
    url: response.url,
  });
};

const format = (args: unknown[]) =>
  args.map((arg) =>
    typeof arg === 'string' ? arg : arg instanceof Error ? arg.message : JSON.stringify(arg),
  );

type SettingDefinition = { id: string; default?: unknown };

export const createApi = (plugin: PluginInfo) => {
  const defaults = new Map<string, unknown>();
  const registered = new Set<string>();
  const key = (id: string) => `nuclear-setting:${id}`;

  const Logger = {
    debug: (...args: unknown[]) => raygun.log.debug(...format(args)),
    info: (...args: unknown[]) => raygun.log.info(...format(args)),
    log: (...args: unknown[]) => raygun.log.info(...format(args)),
    warn: (...args: unknown[]) => raygun.log.warn(...format(args)),
    error: (...args: unknown[]) => raygun.log.error(...format(args)),
  };

  const Settings = {
    async register(definitions: SettingDefinition[]) {
      for (const definition of definitions ?? []) defaults.set(definition.id, definition.default);
      return { registered: (definitions ?? []).map((definition) => definition.id) };
    },
    async get(id: string) {
      const stored = raygun.storage.get(key(id));
      return stored ?? defaults.get(id);
    },
    async set(id: string, value: unknown) {
      raygun.storage.set(key(id), (value ?? null) as never);
    },
    async getGlobal() {
      return undefined;
    },
    async setGlobal() {},
    subscribe: () => () => {},
    // A settings widget (Subsonic's connection test) is Nuclear UI; Raygun shows the fields only.
    registerWidget() {},
  };

  const Providers = {
    register(provider: NuclearProvider) {
      const wrapped = wrapProvider(provider);
      if (!wrapped) {
        Logger.warn(`${plugin.name}: "${provider.kind}" providers aren't supported in Raygun`);
        return provider.id;
      }
      if (!registered.has(wrapped.id)) {
        registered.add(wrapped.id);
        // A kind the conversion didn't find is skipped, not the whole plugin (M46.8).
        try {
          raygun.providers.register(wrapped);
        } catch (error) {
          Logger.warn(`${plugin.name}: skipped a "${provider.kind}" provider: ${String(error)}`);
        }
      }
      return provider.id;
    },
    unregister: () => false,
    list: () => [],
    get: () => undefined,
    getActive: () => undefined,
    setActive() {},
    subscribe: () => () => {},
    clear() {},
    supportedKinds: Object.keys(KIND_MAP),
  };

  const api: Record<string, unknown> = {
    Http: { fetch: pluginFetch },
    Logger,
    Settings: lenient('Settings', Settings),
    Providers: lenient('Providers', Providers),
    Ytdlp: lenient('Ytdlp', createYtdlp()),
  };
  for (const domain of STUBBED_DOMAINS) api[domain] = unsupported(domain);
  return api;
};
