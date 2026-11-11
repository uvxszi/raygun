import './global';
import type {
  FetchInit,
  FilterValues,
  Provider,
  SelectFilter,
  TagsFilter,
  ToggleFilter,
} from './types';

export type * from './types';

/** Type-checks a provider against its kind's contract; returns it unchanged. */
export const defineProvider = <P extends Provider>(provider: P): P => provider;

/**
 * Registers a provider with the app. Call it at the top level of the extension, while the script
 * first runs: providers registered later aren't seen.
 */
export const registerProvider = (provider: Provider): void => {
  raygun.providers.register(provider);
};

/**
 * Says the extension can't work until someone sets it up (its key or sign-in is missing, or its
 * own API refused it with a 401 or 403) and throws. Raygun then asks it nothing until its
 * settings change, and counts it with the sources that need a key rather than the ones that
 * failed. In a Raygun without `raygun.needsSetup`, it throws `reason` as a plain error.
 */
export const needsSetup = (reason: string): never => {
  const host = raygun as Partial<typeof raygun>;
  if (typeof host.needsSetup === 'function') host.needsSetup(reason);
  throw new Error(reason);
};

/**
 * Fetches a page's text; throws `HTTP <status> from <url>` on a non-2xx status. Raygun reads that
 * message (a rate limit, a missing page, a server error) to tell the user what happened.
 */
export const fetchText = async (url: string, init?: FetchInit): Promise<string> => {
  const response = await raygun.http.fetch(url, init);
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} from ${url}`);
  }
  return response.text();
};

/** Fetches and parses JSON; throws `HTTP <status> from <url>` on a non-2xx status. */
export const fetchJson = async <T = unknown>(url: string, init?: FetchInit): Promise<T> => {
  const response = await raygun.http.fetch(url, init);
  if (!response.ok) {
    throw new Error(`HTTP ${response.status} from ${url}`);
  }
  return response.json<T>();
};

type QueryValue = string | number | boolean;

/**
 * A query string, each name and value encoded: `query({ q: 'a b', page: 2 })` is
 * `q=a%20b&page=2`. `undefined` and `null` values are left out; a list repeats its name.
 */
export const query = (
  params: Record<string, QueryValue | readonly QueryValue[] | null | undefined>,
): string =>
  Object.entries(params)
    .flatMap(([name, value]) => {
      if (value === undefined || value === null) return [];
      const values: readonly QueryValue[] = Array.isArray(value) ? value : [value as QueryValue];
      return values.map((item) => `${encodeURIComponent(name)}=${encodeURIComponent(item)}`);
    })
    .join('&');

const SCHEME = /^[a-z][a-z0-9+.-]*:/i;
const PARTS = /^([a-z][a-z0-9+.-]*:)\/\/([^/?#]*)([^?#]*)(\?[^#]*)?(#.*)?$/i;

/** A path with its `.` and `..` segments worked out, as browsers do. */
const normalizePath = (path: string) => {
  const segments: string[] = [];
  const parts = path.split('/');
  parts.forEach((part, index) => {
    const last = index === parts.length - 1;
    if (part === '..') {
      if (segments.length > 1) segments.pop();
      if (last) segments.push('');
    } else if (part === '.') {
      if (last) segments.push('');
    } else {
      segments.push(part);
    }
  });
  const joined = segments.join('/');
  return joined.startsWith('/') ? joined : `/${joined}`;
};

/**
 * An address resolved against the page it was found on, as a browser resolves a link (there is
 * no `URL` in extensions): `resolveUrl('https://example.org/a/b.html', '../c.jpg')` is
 * `https://example.org/c.jpg`. Addresses with a scheme come back as they are.
 */
export const resolveUrl = (base: string, href: string): string => {
  const link = href.trim();
  if (SCHEME.test(link)) return link;
  const parts = PARTS.exec(base.trim());
  if (!parts) throw new Error(`resolveUrl needs an absolute base address, not ${base}`);
  const [, scheme = '', host = '', path = '', search = ''] = parts;
  if (link.startsWith('//')) return `${scheme}${link}`;
  const origin = `${scheme}//${host}`;
  if (link === '') return `${origin}${path || '/'}${search}`;
  if (link.startsWith('#')) return `${origin}${path || '/'}${search}${link}`;
  if (link.startsWith('?')) return `${origin}${path || '/'}${link}`;
  const cut = link.search(/[?#]/);
  const [linkPath, rest] = cut < 0 ? [link, ''] : [link.slice(0, cut), link.slice(cut)];
  const directory = path.slice(0, path.lastIndexOf('/') + 1) || '/';
  const joined = linkPath.startsWith('/') ? linkPath : `${directory}${linkPath}`;
  return `${origin}${normalizePath(joined)}${rest}`;
};

/**
 * A select filter's value in a search: the user's choice when it's one of the filter's options,
 * else the filter's default, else its first option.
 */
export const selectValue = (values: FilterValues | undefined, filter: SelectFilter): string => {
  const chosen = values?.[filter.key];
  const known = (value: unknown): value is string =>
    typeof value === 'string' && filter.options.some((option) => option.value === value);
  if (known(chosen)) return chosen;
  if (known(filter.default)) return filter.default;
  return filter.options[0]?.value ?? '';
};

/** A toggle filter's state in a search: the user's, else the filter's default, else off. */
export const toggleValue = (values: FilterValues | undefined, filter: ToggleFilter): boolean => {
  const chosen = values?.[filter.key];
  return typeof chosen === 'boolean' ? chosen : (filter.default ?? false);
};

/** The tags included and excluded in a search, only ones among the filter's options. */
export const tagsValue = (
  values: FilterValues | undefined,
  filter: TagsFilter,
): { include: string[]; exclude: string[] } => {
  const chosen = values?.[filter.key];
  const known = new Set(filter.options.map((option) => option.value));
  const pick = (list: unknown) =>
    Array.isArray(list) ? list.filter((tag): tag is string => known.has(tag)) : [];
  if (typeof chosen !== 'object' || chosen === null) return { include: [], exclude: [] };
  return { include: pick(chosen.include), exclude: pick(chosen.exclude) };
};
