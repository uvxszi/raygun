// Runs a converted plugin: a small CommonJS module table with `require`, then the plugin's
// lifecycle (onLoad, onEnable). Mirrors how Nuclear's PluginLoader evaluates plugins.
import { createApi } from './api';
import type { NuclearPlugin, PluginInfo } from './nuclear-types';
// React and Nuclear's UI kit don't run headless; any use of them is inert.
import { inert } from './polyfills';

type ModuleFunction = (
  exports: object,
  require: (id: string) => unknown,
  module: { exports: unknown },
  document: unknown,
  window: unknown,
) => void;

const EXTENSIONS = ['', '.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '.json'];

/** The folder a module is in: '' for one at the plugin's root. */
const dirname = (path: string) => (path.includes('/') ? path.replace(/\/[^/]*$/, '') : '');

const join = (base: string, relative: string) => {
  const parts: string[] = [];
  for (const segment of `${base}/${relative}`.split('/')) {
    if (segment === '..') parts.pop();
    else if (segment && segment !== '.') parts.push(segment);
  }
  return parts.join('/');
};

export type Definitions = Record<string, ModuleFunction>;

export const createRequire = (definitions: Definitions, bare: Record<string, unknown>) => {
  const cache = new Map<string, { exports: unknown }>();

  const load = (path: string): unknown => {
    const cached = cache.get(path);
    if (cached) return cached.exports;
    const module = { exports: {} as unknown };
    cache.set(path, module);
    const define = definitions[path] as ModuleFunction;
    // A plugin's page globals are the shim's (an inert page, M46.9.1), wherever it runs.
    const scope = globalThis as Record<string, unknown>;
    define(
      module.exports as object,
      (id) => requireFrom(path, id),
      module,
      scope.document,
      scope.window ?? scope,
    );
    return module.exports;
  };

  const resolve = (from: string, id: string) => {
    const base = join(dirname(from), id);
    for (const candidate of [
      ...EXTENSIONS.map((extension) => `${base}${extension}`),
      ...EXTENSIONS.slice(1).map((extension) => `${base}/index${extension}`),
    ]) {
      if (candidate in definitions) return candidate;
    }
    throw new Error(`Cannot find module '${id}' from '${from}'`);
  };

  const requireFrom = (from: string, id: string): unknown => {
    if (id.startsWith('.')) return load(resolve(from, id));
    if (id in bare) return bare[id];
    if (id === 'react' || id.startsWith('react/') || id.startsWith('@nuclearplayer/ui'))
      return inert;
    throw new Error(`Nuclear plugin needs '${id}', which Raygun doesn't provide`);
  };

  return { load };
};

/**
 * Loads the plugin and runs `onLoad` then `onEnable`. The returned promise is published as
 * `globalThis.__rg_ready`, which the host awaits so providers registered asynchronously count.
 */
export const run = (options: { entry: string; definitions: Definitions; plugin: PluginInfo }) => {
  const api = createApi(options.plugin);
  const { load } = createRequire(options.definitions, {
    '@nuclearplayer/plugin-sdk': { NuclearPluginAPI: class NuclearPluginAPI {} },
    '@nuclearplayer/model': {},
  });
  const exports = load(options.entry) as { default?: NuclearPlugin } & NuclearPlugin;
  const plugin = exports?.default ?? exports;
  if (!plugin || typeof plugin !== 'object') {
    throw new Error('Nuclear plugin must export a default object');
  }
  const ready = (async () => {
    await plugin.onLoad?.(api);
    await plugin.onEnable?.(api);
  })();
  (globalThis as { __rg_ready?: Promise<void> }).__rg_ready = ready;
  return ready;
};
