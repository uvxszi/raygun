// What `raygun-ext validate` checks in a built extension: it runs the bundle in a sandbox like
// Raygun's (a bare JavaScript context with a `raygun` global that makes no requests) to see the
// providers it registers and their calls, and reads the code for requests to hosts the manifest
// doesn't allow and for browser or Node APIs QuickJS doesn't have.

import vm from 'node:vm';

import { KINDS, allowsHost, isProviderId } from './manifest.mjs';

/** @typedef {import('../src/types').Manifest} Manifest */
/**
 * @typedef {{ id: string, kind: string, name: string | null, methods: string[] }} FoundProvider
 * @typedef {{ errors: string[], warnings: string[], providers: FoundProvider[] }} BundleCheck
 */

/** What a provider has besides its calls. */
const META = new Set(['kind', 'id', 'name', 'searchPill']);
/** How long the script may take to set itself up. */
const LOAD_TIMEOUT_MS = 5000;

/**
 * Globals browsers or Node have and QuickJS doesn't, as calls or `new` in the bundle.
 * @type {[name: string, use: RegExp, instead: string][]}
 */
const MISSING_GLOBALS = [
  ['fetch', /(?<![\w$.])fetch\s*\(/, 'raygun.http.fetch (or fetchJson/fetchText from @raygun/sdk)'],
  ['URL', /\bnew\s+URL\s*\(/, 'resolveUrl or plain strings'],
  ['URLSearchParams', /(?<![\w$.])URLSearchParams\b/, 'query from @raygun/sdk'],
  ['setTimeout', /(?<![\w$.])setTimeout\s*\(/, 'raygun.time.sleep'],
  ['setInterval', /(?<![\w$.])setInterval\s*\(/, 'raygun.time.sleep in a loop'],
  ['TextDecoder', /(?<![\w$.])TextDecoder\b/, 'response.text()'],
  ['TextEncoder', /(?<![\w$.])TextEncoder\b/, 'plain strings'],
  ['Intl', /(?<![\w$.])Intl\.\w/, 'formatting of your own'],
  ['structuredClone', /(?<![\w$.])structuredClone\s*\(/, 'JSON.parse(JSON.stringify(…))'],
  ['require', /(?<![\w$.])require\s*\(/, 'imports, which raygun-ext bundles'],
  ['document', /(?<![\w$.])document\.\w/, 'raygun.html.select'],
  ['window', /(?<![\w$.])window\.\w/, 'the raygun global'],
  ['process', /(?<![\w$.])process\.(env|argv|exit)\b/, 'raygun.settings'],
];

/**
 * A word's distance from another, for "did you mean".
 * @param {string} a
 * @param {string} b
 */
const distance = (a, b) => {
  let previous = Array.from({ length: b.length + 1 }, (_, index) => index);
  for (let row = 1; row <= a.length; row += 1) {
    const current = [row];
    for (let column = 1; column <= b.length; column += 1) {
      const cost = a[row - 1] === b[column - 1] ? 0 : 1;
      current.push(
        Math.min(
          (previous[column] ?? 0) + 1,
          (current[column - 1] ?? 0) + 1,
          (previous[column - 1] ?? 0) + cost,
        ),
      );
    }
    previous = current;
  }
  return previous[b.length] ?? 0;
};

/**
 * Hosts the code fetches from: the first argument of each `fetch`-like call when it is a string,
 * a template that starts with one, or a constant holding one. Addresses built at run time aren't
 * seen.
 * @param {string} code
 * @returns {Set<string>}
 */
export const fetchedHosts = (code) => {
  /** @type {Map<string, string>} */
  const constants = new Map();
  const declaration = /\b(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*(["'`])([^"'`]*)\2/g;
  for (const match of code.matchAll(declaration)) {
    const [, name = '', , text = ''] = match;
    constants.set(name, text);
  }
  /** @param {string} text @param {number} depth @returns {string | null} */
  const expand = (text, depth) => {
    const head = /^\$\{\s*([A-Za-z_$][\w$]*)\s*\}/.exec(text);
    if (!head) return text;
    const value = constants.get(head[1] ?? '');
    return value === undefined || depth > 5
      ? null
      : expand(value + text.slice(head[0].length), depth + 1);
  };
  const hosts = new Set();
  // `raygun.http.fetch`, `fetchJson`, `fetchText` and the like (esbuild may number their names).
  // Only the start of the address matters, so a quote inside a `${}` cuts it short harmlessly.
  const call = /\bfetch\w*\s*(?:<[^>()]*>)?\(\s*(?:(["'`])([^"'`]*)|([A-Za-z_$][\w$]*)\s*[,)])/g;
  for (const match of code.matchAll(call)) {
    const literal = match[2];
    const name = match[3];
    const text = literal !== undefined ? expand(literal, 0) : constants.get(name ?? '');
    const host = text ? /^https?:\/\/([^/?#:]+)/i.exec(text)?.[1] : undefined;
    if (host && !host.includes('$')) hosts.add(host.toLowerCase());
  }
  return hosts;
};

/**
 * A `raygun` global that answers like the host's without reaching anything, recording what the
 * script registers.
 * @param {Manifest} manifest
 * @param {unknown[]} registered
 */
const sandboxApi = (manifest, registered) => {
  const quiet = () => {};
  const log = { debug: quiet, info: quiet, warn: quiet, error: quiet };
  return {
    apiVersion: 1,
    log,
    http: {
      fetch: () => Promise.reject(new Error('raygun-ext validate makes no requests')),
    },
    html: { select: () => [] },
    time: { sleep: () => Promise.resolve() },
    storage: { get: () => null, set: quiet, remove: quiet, keys: () => [] },
    settings: {
      get: (/** @type {string} */ key) =>
        manifest.settings?.find((setting) => setting.key === key)?.default ?? null,
    },
    content: { showNsfw: () => false },
    providers: {
      register: (/** @type {unknown} */ provider) => {
        registered.push(provider);
      },
    },
  };
};

/**
 * Checks a built bundle against its manifest the way Raygun loads it.
 * @param {Manifest} manifest
 * @param {string} code
 * @returns {Promise<BundleCheck>}
 */
export const checkBundle = async (manifest, code) => {
  /** @type {string[]} */
  const errors = [];
  /** @type {string[]} */
  const warnings = [];
  /** @type {unknown[]} */
  const registered = [];
  const raygun = sandboxApi(manifest, registered);
  const context = vm.createContext({ raygun, console: { ...raygun.log, log: raygun.log.info } });
  try {
    vm.runInContext(code, context, { filename: manifest.entry, timeout: LOAD_TIMEOUT_MS });
    // Scripts that set up asynchronously publish a promise; their providers count once it settles.
    const ready = context.__rg_ready;
    if (ready && typeof ready.then === 'function') {
      await Promise.race([
        ready,
        new Promise((_, reject) => {
          setTimeout(
            () => reject(new Error('setup took longer than 5 s')),
            LOAD_TIMEOUT_MS,
          ).unref();
        }),
      ]);
    }
  } catch (error) {
    // Errors thrown in the sandbox come from its own `Error`, so `instanceof` can't tell them.
    const message = /** @type {{ message?: unknown }} */ (error)?.message;
    errors.push(
      `the script fails while it loads: ${typeof message === 'string' ? message : String(error)}`,
    );
  }

  /** @type {FoundProvider[]} */
  const providers = [];
  const seen = new Set();
  /** @type {Set<string>} */
  const provides = new Set(manifest.provides);
  for (const value of registered) {
    if (typeof value !== 'object' || value === null) {
      errors.push('raygun.providers.register needs a provider object');
      continue;
    }
    const provider = /** @type {Record<string, unknown>} */ (value);
    const { kind, id, name, searchPill } = provider;
    const label = `provider ${typeof id === 'string' ? `"${id}"` : '(no id)'}`;
    if (typeof kind !== 'string' || kind === '') {
      errors.push(`${label} has no kind`);
      continue;
    }
    if (!provides.has(kind)) {
      errors.push(
        `${label} is a ${kind}, which the manifest's provides doesn't list: Raygun refuses it`,
      );
    }
    if (!isProviderId(id)) {
      errors.push(`${label}: its id must be [a-z0-9][a-z0-9_-]*, at most 64 characters`);
    } else if (seen.has(id)) {
      errors.push(`${label} is registered twice`);
    }
    seen.add(id);
    if (name !== undefined && name !== null && typeof name !== 'string') {
      errors.push(`${label}: its name must be text`);
    }
    if (searchPill !== undefined && kind !== 'video.source') {
      warnings.push(`${label}: only video sources get a searchPill; Raygun ignores it`);
    } else if (typeof searchPill === 'string' && searchPill.trim().length > 24) {
      warnings.push(`${label}: Raygun cuts its searchPill to 24 characters`);
    }
    const info = KINDS[kind];
    const methods = Object.keys(provider).filter((key) => !META.has(key));
    if (info) {
      const known = [...info.required, ...info.optional];
      for (const method of info.required) {
        if (typeof provider[method] !== 'function') {
          errors.push(`${label} (${kind}) has no ${method}: Raygun calls it`);
        }
      }
      for (const method of methods) {
        if (known.includes(method)) {
          if (typeof provider[method] !== 'function') {
            errors.push(`${label}: ${method} must be a function`);
          }
          continue;
        }
        if (typeof provider[method] !== 'function') continue;
        const near = known.find((candidate) => distance(candidate, method) <= 2);
        warnings.push(
          `${label}: Raygun never calls ${method} on a ${kind} provider${near ? ` (did you mean ${near}?)` : ''}`,
        );
      }
    }
    providers.push({
      id: typeof id === 'string' ? id : '',
      kind,
      name: typeof name === 'string' ? name : null,
      methods: methods.filter((method) => typeof provider[method] === 'function'),
    });
  }
  if (errors.length === 0 && providers.length === 0) {
    errors.push('the script registers no providers: call registerProvider at its top level');
  }
  for (const kind of provides) {
    if (registered.length > 0 && !providers.some((provider) => provider.kind === kind)) {
      warnings.push(`provides lists ${kind}, but no provider of that kind is registered`);
    }
  }

  const refused = [...fetchedHosts(code)].filter(
    (host) => !allowsHost(manifest.permissions?.net, host),
  );
  if (refused.length > 0) {
    warnings.push(
      `the code fetches from ${refused.join(', ')}, which permissions.net doesn't list: Raygun refuses those requests`,
    );
  }
  for (const [global, pattern, instead] of MISSING_GLOBALS) {
    if (pattern.test(code)) {
      warnings.push(`the code uses ${global}, which Raygun's QuickJS doesn't have: use ${instead}`);
    }
  }
  return { errors, warnings, providers };
};
