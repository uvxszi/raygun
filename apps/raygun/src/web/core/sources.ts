// Sources → Add in the browser (docs/WEB.md): what an address is, read by the page itself, as
// raygun-app-sources' `source_detect.rs` reads it in the apps. The page fetches with CORS, so only
// sites that let web pages read them answer; it runs only Raygun's own repositories (its
// extensions in sandboxed iframes), so addons, other apps' repositories and servers are named and
// left to a linked app. While one is in use the app answers these commands itself (web/remote/).
import type {
  CatalogEntry,
  DetectedSource,
  ServerKind,
  SourceFormat,
  SourceQueueAdded,
  SourceQueueItem,
  SourceQueueStatus,
  StremioAddon,
  StremioProvides,
} from '@raygun/bindings';

import { CommandError, fail } from './context';
import type { CoreContext } from './context';

/** What only the apps can add, said on the card and when Add is pressed. */
export const NEEDS_APP = 'This needs the Raygun app: link a device in Devices.';

/** The page reads Raygun's own repositories; the extensions service keeps them. */
export type RepoKeeper = {
  repos: () => Array<{ url: string }>;
  addRepo: (url: string) => Promise<unknown>;
  preview: (url: string) => Promise<{ entries: CatalogEntry[] }>;
  rememberIndex: (url: string, index: unknown) => void;
};

/** One answer, as the page got it. */
type Answer = { url: string; status: number; contentType: string; bytes: Uint8Array; text: string };

/** A site that didn't let the page read it (or couldn't be reached). */
const blocked = (url: string) =>
  new CommandError(
    `${hostOf(url)} doesn't let web pages read it; link the Raygun app to add it, or try again from the app.`,
  );

const hostOf = (url: string) => {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
};

/** The addresses `typed` may be: as typed, `stremio://` as https, without a scheme https. */
export const addressesOf = (typed: string): string[] => {
  const notOne = `“${typed}” isn't an address`;
  if (typed.startsWith('stremio://')) return [`https://${typed.slice('stremio://'.length)}`];
  if (/^[a-z]:[\\/]/i.test(typed) || typed.startsWith('/') || typed.startsWith('file:')) {
    fail("A web page can't read files on this device. Add it from the Raygun app.");
  }
  if (typed.includes('://')) {
    let url: URL;
    try {
      url = new URL(typed);
    } catch {
      return fail(notOne);
    }
    if (url.protocol !== 'https:' && url.protocol !== 'http:') fail(`${typed} isn't a web address`);
    url.hash = '';
    return [url.href];
  }
  try {
    const url = new URL(`https://${typed}`);
    if (!url.hostname.includes('.') && url.hostname !== 'localhost' && !url.port) fail(notOne);
    url.hash = '';
    return [url.href];
  } catch {
    return fail(notOne);
  }
};

/** The raw files a GitHub project's page stands for, as `github_places` in the apps. */
export const githubPlaces = (address: string): string[] | null => {
  const url = new URL(address);
  if (url.hostname !== 'github.com' && url.hostname !== 'www.github.com') return null;
  const parts = url.pathname.split('/').filter(Boolean);
  const [owner, rawProject, ...rest] = parts;
  if (!owner || !rawProject) return null;
  const project = rawProject.replace(/\.git$/, '');
  const raw = (path: string) => `https://raw.githubusercontent.com/${owner}/${project}/${path}`;
  if (rest.length === 0) {
    return [
      'repo/index.min.json',
      'HEAD/index.json',
      'HEAD/index.min.json',
      'HEAD/manifest.json',
      'HEAD/plugins.json',
      'plugins/v3.0.0/.dist/plugins.min.json',
      'HEAD/addons.xml',
    ].map(raw);
  }
  const [mode, branch, ...inside] = rest;
  if (mode === 'tree' && branch) {
    const folder = inside.map((part) => `${part}/`).join('');
    return [...INDEX_NAMES, 'manifest.json'].map((name) => raw(`${branch}/${folder}${name}`));
  }
  if (mode === 'blob' && branch && inside.length > 0) return [raw(`${branch}/${inside.join('/')}`)];
  return null;
};

const INDEX_NAMES = [
  'index.json',
  'index.min.json',
  'plugins.min.json',
  'plugins.json',
  'addons.xml',
];

/** Servers' well-known endpoints under an address (`SERVER_PROBES`). */
const SERVER_PROBES = [
  'System/Info/Public',
  'emby/System/Info/Public',
  'rest/ping.view?v=1.16.1&c=Raygun&f=json',
  'status',
  'opds/v1.2/catalog',
  'opds',
];

const folderOf = (address: string) => {
  const url = new URL(address);
  url.search = '';
  url.hash = '';
  if (!url.pathname.endsWith('/')) url.pathname = `${url.pathname}/`;
  return url;
};

const under = (address: string, path: string) => new URL(path, folderOf(address)).href;

const namesAFile = (address: string) => {
  const last = new URL(address).pathname.split('/').filter(Boolean).pop() ?? '';
  return last.includes('.');
};

/** Where to look when an address answered with nothing to add (`beside`). */
const besidePlaces = (asked: string): Array<{ url: string; base: string }> => {
  const places: Array<{ url: string; base: string }> = [];
  const add = (url: string, base = url) => {
    if (url !== asked && !places.some((place) => place.url === url)) places.push({ url, base });
  };
  const file = namesAFile(asked);
  if (!file) {
    add(under(asked, 'manifest.json'));
    for (const name of ['index.json', 'index.min.json']) add(under(asked, name));
  }
  if (asked.toLowerCase().endsWith('/addons.xml')) add(`${asked}.gz`);
  const root = new URL(asked);
  root.pathname = '/';
  root.search = '';
  const bases = [root.href];
  if (!file && new URL(asked).pathname.replace(/\//g, '') !== '') {
    const base = new URL(asked);
    base.search = '';
    bases.unshift(base.href);
  }
  for (const base of bases) for (const probe of SERVER_PROBES) add(under(base, probe), base);
  return places;
};

/** What one answer holds. */
type Reading =
  | { kind: 'found'; source: DetectedSource }
  | { kind: 'refused'; message: string }
  | { kind: 'raygun'; url: string; name: string; description: string | null; index: unknown };

const detected = (
  fields: Partial<DetectedSource> & Pick<DetectedSource, 'kind' | 'url' | 'name'>,
) =>
  ({
    format: null,
    server: null,
    description: null,
    added: false,
    addon: null,
    entries: [],
    note: null,
    ...fields,
  }) satisfies DetectedSource;

const text = (value: unknown): string | null =>
  typeof value === 'string' && value.trim() ? value.trim() : null;

/** An addon's manifest address from where it answered (`normalize_addon_url`). */
const manifestAddress = (url: string) => {
  const address = new URL(url);
  address.search = '';
  address.hash = '';
  if (!address.pathname.endsWith('/manifest.json')) {
    address.pathname = `${address.pathname.replace(/\/+$/, '')}/manifest.json`;
  }
  return address.href;
};

/** A Stremio manifest as the addon list shows it (`protocol::parse_manifest`). */
export const addonOf = (manifest: Record<string, unknown>, url: string): StremioAddon => {
  const resources = Array.isArray(manifest.resources)
    ? manifest.resources.flatMap((resource) =>
        typeof resource === 'string'
          ? [resource]
          : resource && typeof resource === 'object' && typeof resource.name === 'string'
            ? [resource.name as string]
            : [],
      )
    : [];
  const types = Array.isArray(manifest.types)
    ? manifest.types.filter((type): type is string => typeof type === 'string')
    : [];
  const catalogs = Array.isArray(manifest.catalogs) ? manifest.catalogs : [];
  const hints = (manifest.behaviorHints ?? {}) as Record<string, unknown>;
  // A row per resource, by type for catalogs (as the app's addon rows show them).
  const provides: StremioProvides[] = resources.flatMap((resource): StremioProvides[] =>
    resource === 'catalog'
      ? [...new Set(catalogs.map((catalog) => String((catalog as { type?: unknown }).type ?? '')))]
          .filter(Boolean)
          .map((kind) => ({
            resource,
            kind,
            count: catalogs.filter((catalog) => (catalog as { type?: unknown }).type === kind)
              .length,
          }))
      : [{ resource, kind: null, count: 1 }],
  );
  const address = manifestAddress(url);
  return {
    id: String(manifest.id ?? ''),
    url: address,
    name: text(manifest.name) ?? hostOf(url),
    description: text(manifest.description),
    logo: text(manifest.logo),
    version: text(manifest.version) ?? '0.0.0',
    resources,
    types,
    catalogs: catalogs.length,
    adult: hints.adult === true,
    error: null,
    enabled: true,
    provides,
    p2p: hints.p2p === true,
    configureUrl:
      hints.configurable === true ? address.replace(/manifest\.json$/, 'configure') : null,
    needsSetup: hints.configurationRequired === true,
    failing: false,
  };
};

const listOf = (value: unknown) => (Array.isArray(value) ? (value as unknown[]) : null);

/** Another app's repository: named, counted, and left to the apps. */
const foreign = (url: string, format: SourceFormat, name: string, count: number) =>
  detected({
    kind: 'repository',
    format,
    url,
    name,
    description: count > 0 ? `${count} extensions` : null,
    note: NEEDS_APP,
  });

const firstElement = (body: string) =>
  body
    .replace(/<\?[\s\S]*?\?>/g, '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<!DOCTYPE[^>]*>/gi, '')
    .match(/<([A-Za-z_][\w.:-]*)/)?.[1]
    ?.split(':')
    .pop()
    ?.toLowerCase() ?? null;

const readXml = (answer: Answer, base: string): Reading | null => {
  switch (firstElement(answer.text)) {
    case 'addons': {
      const count = answer.text.match(/<addon[\s>]/g)?.length ?? 0;
      return {
        kind: 'found',
        source: foreign(answer.url, 'kodi', hostOf(answer.url), count),
      };
    }
    case 'addon':
      return {
        kind: 'refused',
        message: `A Kodi add-on on its own is added in the app. ${NEEDS_APP}`,
      };
    case 'subsonic-response': {
      const kind = answer.text.includes('type="navidrome"') ? 'navidrome' : 'subsonic';
      return { kind: 'found', source: server(kind, subsonicRoot(base), null) };
    }
    case 'feed':
      if (
        answer.contentType.includes('opds') ||
        answer.text.includes('opds-spec.org') ||
        answer.text.includes('profile=opds-catalog')
      ) {
        const title = answer.text.match(/<title[^>]*>([^<]*)<\/title>/)?.[1]?.trim() || null;
        return { kind: 'found', source: server('opds', answer.url, title) };
      }
      return null;
    default:
      return null;
  }
};

const server = (kind: ServerKind, url: string, name: string | null) =>
  detected({ kind: 'server', server: kind, url, name: name ?? hostOf(url), note: NEEDS_APP });

/** A Subsonic server's address before its API's `/rest/` part. */
const subsonicRoot = (base: string) => {
  const url = new URL(base);
  url.search = '';
  const lower = url.pathname.toLowerCase();
  const at = lower.indexOf('/rest/');
  let root = at >= 0 ? url.pathname.slice(0, at + 1) : url.pathname;
  if (at < 0 && lower.endsWith('/rest')) root = url.pathname.slice(0, -'rest'.length);
  url.pathname = root.endsWith('/') ? root : `${root}/`;
  return url.href;
};

const readServer = (
  value: Record<string, unknown>,
  answer: Answer,
  base: string,
): DetectedSource | null => {
  if ('subsonic-response' in value) {
    const response = value['subsonic-response'];
    const navidrome =
      typeof response === 'object' &&
      response !== null &&
      'type' in response &&
      text(response.type)?.toLowerCase() === 'navidrome';
    return server(navidrome ? 'navidrome' : 'subsonic', subsonicRoot(base), null);
  }
  if ('Version' in value && 'Id' in value && ('ServerName' in value || 'ProductName' in value)) {
    const emby = (text(value.ProductName) ?? '').toLowerCase().includes('emby');
    return server(emby ? 'emby' : 'jellyfin', base, text(value.ServerName));
  }
  if ((text(value.app) ?? '').toLowerCase() === 'audiobookshelf') {
    return server('audiobookshelf', base, null);
  }
  const metadata = value.metadata as Record<string, unknown> | undefined;
  if (
    metadata &&
    typeof metadata === 'object' &&
    (answer.contentType.includes('opds+json') ||
      ['navigation', 'publications', 'groups'].some((key) => key in value))
  ) {
    return server('opds', answer.url, text(metadata.title));
  }
  return null;
};

const readJson = (answer: Answer, base: string): Reading | null => {
  let value: unknown;
  try {
    value = JSON.parse(answer.text);
  } catch {
    return null;
  }
  const list = listOf(value);
  if (list) {
    const entries = list.filter(
      (entry): entry is Record<string, unknown> => !!entry && typeof entry === 'object',
    );
    if (entries.length === 0) return null;
    const sample = entries[0] ?? {};
    const name = hostOf(answer.url);
    if ('pkg' in sample && 'apk' in sample) {
      return { kind: 'found', source: foreign(answer.url, 'mihon', name, entries.length) };
    }
    if ('site' in sample && 'url' in sample) {
      return { kind: 'found', source: foreign(answer.url, 'lnreader', name, entries.length) };
    }
    if ('package' in sample) {
      return { kind: 'found', source: foreign(answer.url, 'miru', name, entries.length) };
    }
    return null;
  }
  if (!value || typeof value !== 'object') return null;
  const object = value as Record<string, unknown>;
  if ('id' in object && 'resources' in object) {
    const addon = addonOf(object, answer.url);
    const onlyLists =
      addon.resources.length > 0 && addon.resources.every((name) => name === 'addon_catalog');
    if (onlyLists) {
      return {
        kind: 'found',
        source: foreign(addon.url, 'stremio', addon.name, 0),
      };
    }
    return {
      kind: 'found',
      source: detected({
        kind: 'addon',
        url: addon.url,
        name: addon.name,
        description: addon.description,
        addon,
        note: NEEDS_APP,
      }),
    };
  }
  const found = readServer(object, answer, base);
  if (found) return { kind: 'found', source: found };
  const name = text(object.name) ?? hostOf(answer.url);
  const scripts = listOf(object.scripts);
  if (scripts)
    return { kind: 'found', source: foreign(answer.url, 'shosetsu', name, scripts.length) };
  const scrapers = listOf(object.scrapers);
  if (scrapers)
    return { kind: 'found', source: foreign(answer.url, 'nuvio', name, scrapers.length) };
  const plugins = listOf(object.plugins);
  if (plugins)
    return { kind: 'found', source: foreign(answer.url, 'nuclear', name, plugins.length) };
  if (listOf(object.extensions)) {
    return {
      kind: 'raygun',
      url: answer.url,
      name,
      description: text(object.description),
      index: object,
    };
  }
  return null;
};

const read = (answer: Answer, base: string): Reading | null => {
  const bytes = answer.bytes;
  if (bytes[0] === 0x50 && bytes[1] === 0x4b) {
    return {
      kind: 'refused',
      message: `One extension on its own is installed from the app. ${NEEDS_APP}`,
    };
  }
  if (bytes[0] === 0x1f && bytes[1] === 0x8b) {
    return { kind: 'found', source: foreign(answer.url, 'kodi', hostOf(answer.url), 0) };
  }
  const body = answer.text.replace(/^\uFEFF/, '').trimStart();
  const first = body[0];
  if (first === '<') return readXml({ ...answer, text: body }, base);
  if (first === '{' || first === '[') return readJson({ ...answer, text: body }, base);
  if (/\.(lua|js)$/i.test(new URL(answer.url).pathname)) {
    return {
      kind: 'refused',
      message: `A script on its own is installed from the app. ${NEEDS_APP}`,
    };
  }
  return null;
};

export class WebSources {
  private queue: SourceQueueStatus = emptyQueue();
  private nextId = 1;
  private working = false;

  constructor(
    private readonly context: CoreContext,
    private readonly repos: RepoKeeper,
    private readonly get: typeof fetch = (input, init) => fetch(input, init),
  ) {}

  private async fetchOne(url: string): Promise<Answer> {
    let response: Response;
    try {
      response = await this.get(url, { credentials: 'omit' });
    } catch {
      throw blocked(url);
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    return {
      url: response.url || url,
      status: response.status,
      contentType: response.headers.get('content-type') ?? '',
      bytes,
      text: new TextDecoder().decode(bytes),
    };
  }

  /** What `typed` is, read by the page. */
  async detect(typed: string): Promise<DetectedSource> {
    const wanted = typed.trim();
    if (!wanted) return fail('Type or paste an address');
    const [first] = addressesOf(wanted);
    if (!first) return fail(`“${wanted}” isn't an address`);
    const reading = await this.look(first);
    return this.shown(reading);
  }

  /** Adds what `typed` is: Raygun's own repositories here, the rest in a linked app. */
  async add(typed: string): Promise<DetectedSource> {
    const found = await this.detect(typed);
    if (found.kind !== 'repository' || found.format !== 'raygun') {
      return fail(found.note ?? NEEDS_APP);
    }
    if (!found.added) await this.repos.addRepo(found.url);
    return { ...found, added: true };
  }

  private async shown(reading: Reading): Promise<DetectedSource> {
    if (reading.kind === 'refused') return fail(reading.message);
    if (reading.kind === 'found') return reading.source;
    this.repos.rememberIndex(reading.url, reading.index);
    const listing = await this.repos.preview(reading.url);
    return detected({
      kind: 'repository',
      format: 'raygun',
      url: reading.url,
      name: reading.name,
      description: reading.description,
      added: this.repos.repos().some((repo) => sameRepo(repo.url, reading.url)),
      entries: listing.entries,
    });
  }

  private async look(first: string): Promise<Reading> {
    const github = githubPlaces(first);
    if (github) {
      const project = new URL(first).pathname.replace(/^\/|\/$/g, '');
      const found = await this.firstFound(github.map((url) => ({ url, base: url })));
      if (found) return found;
      return fail(`${project} publishes no repository, addon or extension Raygun knows`);
    }
    const answer = await this.fetchOne(first);
    let refused: Reading | null = null;
    if (answer.status >= 200 && answer.status < 300) {
      const reading = read(answer, first);
      if (reading?.kind === 'refused') refused = reading;
      else if (reading) return reading;
    }
    if (refused) return refused;
    const beside = await this.firstFound(besidePlaces(first));
    if (beside) return beside;
    const detail =
      answer.status >= 200 && answer.status < 300
        ? ''
        : ` (${answer.url} answered HTTP ${answer.status})`;
    return fail(
      `Raygun found no repository, addon, server or extension at ${hostOf(first)}${detail}`,
    );
  }

  /** The first of `places`, in order, that's something to add (or that refused, saying why). */
  private async firstFound(places: Array<{ url: string; base: string }>): Promise<Reading | null> {
    const answers = places.map((place) =>
      this.fetchOne(place.url).then(
        (answer) => (answer.status >= 200 && answer.status < 300 ? read(answer, place.base) : null),
        () => null,
      ),
    );
    let refused: Reading | null = null;
    for (const answer of answers) {
      const reading = await answer;
      if (!reading) continue;
      if (reading.kind === 'refused') refused ??= reading;
      else return reading;
    }
    return refused;
  }

  // ---- Add several (source_queue.rs): one address a line, one after another ----

  status(): SourceQueueStatus {
    return structuredClone(this.queue);
  }

  private changed() {
    const items = this.queue.items;
    const count = (state: SourceQueueItem['state']) =>
      items.filter((item) => item.state === state).length;
    this.queue = {
      ...this.queue,
      total: items.length,
      added: count('added'),
      alreadyAdded: count('alreadyAdded'),
      needsSignIn: count('needsSignIn'),
      needsSetup: count('needsSetup'),
      failed: count('failed'),
      cancelled: count('cancelled'),
      running: items.some((item) => ['waiting', 'checking', 'adding'].includes(item.state)),
      current:
        items.find((item) => item.state === 'checking' || item.state === 'adding')?.address ?? null,
    };
    this.context.emit('sourceQueueEvent', this.status());
  }

  enqueue(list: string): SourceQueueAdded {
    const known = new Set(this.queue.items.map((item) => keyOf(item.address)));
    let queued = 0;
    let duplicates = 0;
    let overLimit = 0;
    const batch = this.queue.batch + 1;
    for (const line of list.split(/\r?\n/)) {
      const address = line.trim();
      if (!address || address.startsWith('#')) continue;
      const key = keyOf(address);
      if (known.has(key)) {
        duplicates += 1;
        continue;
      }
      if (this.queue.items.length >= MAX_QUEUED) {
        overLimit += 1;
        continue;
      }
      known.add(key);
      queued += 1;
      this.queue.items.push({
        id: this.nextId++,
        address,
        state: 'waiting',
        kind: null,
        format: null,
        server: null,
        name: null,
        url: null,
        configureUrl: null,
        error: null,
        batch,
      });
    }
    if (queued > 0) this.queue.batch = batch;
    this.changed();
    void this.work();
    return { queued, duplicates, overLimit, status: this.status() };
  }

  private async work() {
    if (this.working) return;
    this.working = true;
    try {
      for (;;) {
        const item = this.queue.items.find((candidate) => candidate.state === 'waiting');
        if (!item) break;
        item.state = 'checking';
        this.changed();
        try {
          const found = await this.detect(item.address);
          Object.assign(item, {
            kind: found.kind,
            format: found.format,
            server: found.server,
            name: found.name,
            url: found.url,
            configureUrl: found.addon?.configureUrl ?? null,
          });
          // Cancelled while it was looked at.
          if (stateOf(item) !== 'checking') continue;
          if (found.added) {
            item.state = 'alreadyAdded';
          } else if (found.kind === 'repository' && found.format === 'raygun') {
            item.state = 'adding';
            this.changed();
            await this.repos.addRepo(found.url);
            item.state = 'added';
          } else {
            item.state = 'failed';
            item.error = found.note ?? NEEDS_APP;
          }
        } catch (error) {
          if (stateOf(item) === 'cancelled') continue;
          item.state = 'failed';
          item.error = error instanceof Error ? error.message : String(error);
        }
        this.changed();
      }
    } finally {
      this.working = false;
      this.changed();
    }
  }

  cancel(id: number | null): SourceQueueStatus {
    for (const item of this.queue.items) {
      if ((id === null || item.id === id) && ['waiting', 'checking'].includes(item.state)) {
        item.state = 'cancelled';
      }
    }
    this.changed();
    return this.status();
  }

  retry(): SourceQueueStatus {
    for (const item of this.queue.items) {
      if (item.state === 'failed' || item.state === 'cancelled') {
        item.state = 'waiting';
        item.error = null;
      }
    }
    this.changed();
    void this.work();
    return this.status();
  }

  dismiss(id: number | null): SourceQueueStatus {
    this.queue.items = this.queue.items.filter(
      (item) =>
        (id !== null && item.id !== id) || ['waiting', 'checking', 'adding'].includes(item.state),
    );
    this.changed();
    return this.status();
  }
}

/**
 * Whether two repository addresses are one: GitHub's raw files of the default branch are the same
 * under `HEAD`, `main` or `master` (Paraline is kept under `main`, its page maps to `HEAD`).
 */
export const sameRepo = (left: string, right: string) => {
  const key = (url: string) =>
    url.replace(
      /^(https:\/\/raw\.githubusercontent\.com\/[^/]+\/[^/]+)\/(?:HEAD|main|master)\//,
      '$1/HEAD/',
    );
  return key(left) === key(right);
};

/** An item's state now (another call may have changed it meanwhile). */
const stateOf = (item: SourceQueueItem): SourceQueueItem['state'] => item.state;

/** Addresses queued at most, as the apps' queue (`MAX_QUEUED`). */
const MAX_QUEUED = 500;

const keyOf = (typed: string) => {
  try {
    return (addressesOf(typed.trim())[0] ?? typed).replace(/\/+$/, '');
  } catch {
    return typed.trim().toLowerCase();
  }
};

const emptyQueue = (): SourceQueueStatus => ({
  batch: 0,
  items: [],
  total: 0,
  added: 0,
  alreadyAdded: 0,
  needsSignIn: 0,
  needsSetup: 0,
  failed: 0,
  cancelled: 0,
  running: false,
  paused: false,
  loading: false,
  current: null,
});
