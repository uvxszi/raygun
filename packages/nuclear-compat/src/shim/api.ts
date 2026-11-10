// A NuclearPluginAPI built on Raygun's host API (shape from Nuclear's plugin-sdk, AGPL-3.0).
// Http, Logger, Settings, Providers and Ytdlp (YouTube, over `raygun.youtube`) work; every other
// domain is a stub that says so.
import { HeadersPolyfill, ResponsePolyfill } from './polyfills';
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

const unsupported = (domain: string) =>
  new Proxy(
    {},
    {
      get: (_target, property) => {
        if (property === Symbol.toPrimitive || property === 'then') return undefined;
        return () => {
          throw new Error(`${domain} isn't available to Nuclear plugins in Raygun yet`);
        };
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

/** `fetch` for plugins, over `raygun.http.fetch` (so the manifest's host permissions apply). */
export const pluginFetch = async (input: unknown, init: FetchInit = {}) => {
  const url = String(
    typeof input === 'object' && input !== null && 'url' in input
      ? (input as { url: string }).url
      : input,
  );
  const response = await raygun.http.fetch(url, {
    method: init.method ?? 'GET',
    headers: headersRecord(init.headers),
    body: typeof init.body === 'string' ? init.body : null,
  });
  return new ResponsePolyfill(await response.text(), {
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
