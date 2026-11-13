// Sources in the browser (docs/WEB.md): Raygun's own JavaScript extensions, installed from a
// repository (Paraline by default) and run in sandboxed iframes (sandbox.ts). The browser fetches
// for them, so only sources whose sites allow web pages to read them (CORS) work; others say so.
// Other formats (Nuclear, Shosetsu, Kodi, Mihon, Stremio) need the app.
import type {
  CatalogEntry,
  ExtensionInfo_Serialize,
  ExtensionInstall,
  ExtensionInstallRequest,
  ExtensionInstallStatus,
  ExtensionRepo,
  ExtensionSetting_Serialize,
  MediaSource,
  OnlineTrack,
  ProviderInfo_Serialize,
  ProviderSearchResult_Serialize,
  QueueItem,
  RepoCatalog,
  SettingValue,
  Settings,
  SuggestedRepo,
} from '@raygun/bindings';

import { uuidV7Of } from '../core/context';
import type { CoreContext } from '../core/context';
import { Sandbox } from './sandbox';
import { readZip } from './zip';

export const PARALINE_URL =
  'https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/index.json';
const PARALINE: SuggestedRepo = {
  url: PARALINE_URL,
  name: 'Paraline',
  description: 'Public-domain and openly licensed books, audiobooks, films, comics and music.',
  added: false,
};
const REPOS = 'ext.repos';
const NOT_HERE = "Only Raygun's own sources run in the browser; this one needs the app.";

type ManifestSetting = {
  key: string;
  label?: string;
  type?: string;
  options?: string[];
  default?: SettingValue;
  required?: boolean;
};

type Manifest = {
  id: string;
  name: string;
  version: string;
  runtime?: string;
  entry?: string;
  description?: string;
  provides?: string[];
  permissions?: { net?: string[] };
  contentRating?: string;
  settings?: ManifestSetting[];
  website?: string;
};

/** An installed extension, as kept in the `extensions` table. */
export type InstalledExtension = {
  manifest: Manifest;
  source: string;
  repoId: string | null;
  repository: string | null;
  installedAt: number;
  values: Record<string, SettingValue>;
};

type IndexEntry = {
  id: string;
  name?: string;
  version?: string;
  description?: string;
  author?: string;
  provides?: string[];
  permissions?: { net?: string[] };
  contentRating?: string;
  package?: string;
  sha256?: string;
  size?: number;
  icon?: string;
  lang?: string;
  format?: string;
  runtime?: string;
};

type RepoIndex = { name?: string; extensions?: IndexEntry[] };

const compareVersions = (a: string, b: string) => {
  const parts = (version: string) =>
    version.split(/[.-]/).map((part) => Number.parseInt(part, 10) || 0);
  const [left, right] = [parts(a), parts(b)];
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const difference = (left[index] ?? 0) - (right[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
};

const hexOf = (buffer: ArrayBuffer) =>
  Array.from(new Uint8Array(buffer), (byte) => byte.toString(16).padStart(2, '0')).join('');

const httpUrl = (value: unknown): string | null =>
  typeof value === 'string' && /^https?:\/\//i.test(value) ? value : null;

/** A source as a provider gave it, checked as the app's host checks it. */
export const mediaSourceOf = (value: unknown): MediaSource | null => {
  if (!value || typeof value !== 'object') return null;
  const source = value as Record<string, unknown>;
  if (source.kind === 'url' && httpUrl(source.url)) {
    const headers = source.headers;
    if (!headers || typeof headers !== 'object') return null;
    return { kind: 'url', url: String(source.url), headers: headers as Record<string, string> };
  }
  if (source.kind === 'youTube' && typeof source.videoId === 'string' && source.videoId) {
    return { kind: 'youTube', videoId: source.videoId };
  }
  return null;
};

/** A track as a provider gave it; one without a title is dropped. */
export const trackOf = (value: unknown): OnlineTrack | null => {
  if (!value || typeof value !== 'object') return null;
  const track = value as Record<string, unknown>;
  if (typeof track.title !== 'string' || !track.title.trim()) return null;
  const text = (field: unknown) => (typeof field === 'string' && field.trim() ? field : null);
  const duration =
    typeof track.durationMs === 'number' && track.durationMs > 0 ? track.durationMs : null;
  return {
    externalId: track.id === undefined || track.id === null ? null : String(track.id),
    title: track.title,
    artist: text(track.artist),
    album: text(track.album),
    durationMs: duration === null ? null : Math.round(duration),
    artworkUrl: httpUrl(track.artworkUrl),
    source: mediaSourceOf(track.source),
    explicit: track.explicit === true,
  };
};

const words = (text: string | null | undefined) =>
  new Set(
    (text ?? '')
      .toLowerCase()
      .normalize('NFD')
      .replace(/[̀-ͯ]/g, '')
      .split(/[^\p{L}\p{N}]+/u)
      .filter(Boolean),
  );

/** How well a candidate's names match a song's, 0 to 1 (title counts most). */
export const matchScore = (
  wanted: { title: string; artist: string | null },
  found: { title: string; artist?: string | null },
) => {
  const overlap = (a: Set<string>, b: Set<string>) => {
    if (a.size === 0 || b.size === 0) return 0;
    let shared = 0;
    for (const word of a) if (b.has(word)) shared += 1;
    return shared / Math.max(a.size, b.size);
  };
  const title = overlap(words(wanted.title), words(found.title));
  const artist = wanted.artist ? overlap(words(wanted.artist), words(found.artist)) : 1;
  return title * 0.7 + artist * 0.3;
};

export class ExtensionsService {
  private readonly sandboxes = new Map<string, Sandbox>();
  private readonly failures = new Map<string, string>();
  private readonly indexes = new Map<string, RepoIndex>();
  private queue: ExtensionInstall[] = [];
  private batch = 0;
  private running = false;

  constructor(
    private readonly context: CoreContext,
    private readonly settings: () => Settings,
  ) {}

  // ---- Installed extensions ----

  installed(): InstalledExtension[] {
    return this.context.store
      .all<InstalledExtension>('extensions')
      .sort((a, b) => a.manifest.name.localeCompare(b.manifest.name));
  }

  private sandboxFor(extension: InstalledExtension): Sandbox {
    const id = extension.manifest.id;
    const running = this.sandboxes.get(id);
    if (running) return running;
    const storage = this.context.store.get<Record<string, unknown>>('ext_storage', id) ?? {};
    const sandbox = new Sandbox(
      {
        id,
        source: extension.source,
        net: extension.manifest.permissions?.net ?? [],
        settings: this.values(extension),
        storage,
        showNsfw: this.settings().showNsfw,
      },
      (key, value, removed) => {
        const kept = {
          ...(this.context.store.get<Record<string, unknown>>('ext_storage', id) ?? {}),
        };
        if (removed) delete kept[key];
        else kept[key] = value;
        this.context.store.put('ext_storage', id, kept);
      },
    );
    this.sandboxes.set(id, sandbox);
    sandbox.ready().then(
      () => this.failures.delete(id),
      (error: unknown) =>
        this.failures.set(id, error instanceof Error ? error.message : String(error)),
    );
    return sandbox;
  }

  /** Loads every installed extension (once); resolves when all have run. */
  async start(): Promise<void> {
    if (typeof document === 'undefined') return;
    await Promise.allSettled(
      this.installed().map((extension) => this.sandboxFor(extension).ready()),
    );
  }

  private stopOne(id: string) {
    this.sandboxes.get(id)?.dispose();
    this.sandboxes.delete(id);
    this.failures.delete(id);
  }

  private values(extension: InstalledExtension): Record<string, unknown> {
    const values: Record<string, unknown> = {};
    for (const setting of extension.manifest.settings ?? []) {
      values[setting.key] = extension.values[setting.key] ?? setting.default ?? null;
    }
    return values;
  }

  private adult(manifest: Manifest) {
    return manifest.contentRating === 'nsfw' || manifest.contentRating === 'adult';
  }

  list(): ExtensionInfo_Serialize[] {
    return this.installed().map((extension) => {
      const manifest = extension.manifest;
      const sandbox = this.sandboxes.get(manifest.id);
      const error = this.failures.get(manifest.id) ?? null;
      const providers: ProviderInfo_Serialize[] = (sandbox?.providers ?? []).map((provider) => ({
        id: `${manifest.id}/${provider.id}`,
        extensionId: manifest.id,
        kind: provider.kind,
        name: provider.name ?? manifest.name,
        searchPill: provider.searchPill ?? null,
        adult: this.adult(manifest),
      }));
      const settings: ExtensionSetting_Serialize[] = (manifest.settings ?? []).map((setting) => ({
        key: setting.key,
        label: setting.label ?? setting.key,
        kind: (['text', 'number', 'boolean', 'select'].includes(setting.type ?? '')
          ? setting.type
          : 'text') as ExtensionSetting_Serialize['kind'],
        options: setting.options ?? [],
        default: setting.default ?? null,
        value: extension.values[setting.key] ?? null,
        required: setting.required === true,
      }));
      return {
        id: manifest.id,
        name: manifest.name,
        version: manifest.version,
        description: manifest.description ?? null,
        contentRating: manifest.contentRating ?? 'safe',
        adult: this.adult(manifest),
        origin: 'installed',
        status: error ? 'failed' : 'loaded',
        error,
        provides: manifest.provides ?? [],
        net: manifest.permissions?.net ?? [],
        providers,
        settings,
        compatibility: null,
        icon: null,
        website: manifest.website ?? null,
        repository: extension.repository,
      };
    });
  }

  private changed() {
    this.context.emit('extensionsChangedEvent', this.list());
  }

  uninstall(id: string) {
    this.stopOne(id);
    this.context.store.remove('extensions', id);
    this.context.store.remove('ext_storage', id);
    this.changed();
  }

  clearData(id: string) {
    this.context.store.remove('ext_storage', id);
    this.reload(id);
  }

  setSetting(id: string, key: string, value: SettingValue) {
    const extension = this.context.store.get<InstalledExtension>('extensions', id);
    if (!extension) throw new Error('That source isn’t installed.');
    this.context.store.put('extensions', id, {
      ...extension,
      values: { ...extension.values, [key]: value },
    });
    this.reload(id);
  }

  reload(id?: string) {
    for (const extension of this.installed()) {
      if (id !== undefined && extension.manifest.id !== id) continue;
      this.stopOne(extension.manifest.id);
      this.sandboxFor(extension);
    }
    this.changed();
  }

  // ---- Repositories and the catalogue ----

  repos(): ExtensionRepo[] {
    return (
      this.context.meta<ExtensionRepo[]>(REPOS) ?? [
        { id: 'paraline', url: PARALINE_URL, name: PARALINE.name },
      ]
    );
  }

  suggested(): SuggestedRepo[] {
    return [{ ...PARALINE, added: this.repos().some((repo) => repo.url === PARALINE_URL) }];
  }

  private async index(url: string, refresh = false): Promise<RepoIndex> {
    const kept = this.indexes.get(url);
    if (kept && !refresh) return kept;
    let response: Response;
    try {
      response = await fetch(url, { credentials: 'omit', cache: refresh ? 'no-cache' : 'default' });
    } catch {
      throw new Error(`${new URL(url).hostname} doesn't let web pages read it`);
    }
    if (!response.ok) throw new Error(`HTTP ${response.status} from ${url}`);
    const index = (await response.json()) as RepoIndex;
    if (!Array.isArray(index.extensions))
      throw new Error("That address isn't a Raygun repository.");
    this.indexes.set(url, index);
    return index;
  }

  async addRepo(url: string): Promise<ExtensionRepo[]> {
    const address = url.trim();
    if (!/^https:\/\//i.test(address))
      throw new Error('A repository’s address starts with https://');
    if (this.repos().some((repo) => repo.url === address)) return this.repos();
    const index = await this.index(address, true);
    const repos = [
      ...this.repos(),
      { id: uuidV7Of(this.context), url: address, name: index.name ?? new URL(address).hostname },
    ];
    this.context.setMeta(REPOS, repos);
    return repos;
  }

  removeRepo(id: string): ExtensionRepo[] {
    const repos = this.repos().filter((repo) => repo.id !== id);
    this.context.setMeta(REPOS, repos);
    return repos;
  }

  /** Why an index entry can't run here, if it can't. */
  private unsupported(entry: IndexEntry): string | null {
    const format = entry.format ?? 'raygun';
    if (format !== 'raygun' || !entry.package) return NOT_HERE;
    if (entry.runtime && entry.runtime !== 'js') return NOT_HERE;
    return null;
  }

  async catalog(refresh: boolean): Promise<RepoCatalog[]> {
    const installed = new Map(
      this.installed().map((extension) => [extension.manifest.id, extension]),
    );
    return Promise.all(
      this.repos().map(async (repo) => {
        try {
          const index = await this.index(repo.url, refresh);
          const entries: CatalogEntry[] = (index.extensions ?? []).map((entry) => {
            const have = installed.get(entry.id);
            const version = entry.version ?? '0.0.0';
            const order = have ? compareVersions(have.manifest.version, version) : 0;
            const unsupported = this.unsupported(entry);
            return {
              repoId: repo.id,
              id: entry.id,
              name: entry.name ?? entry.id,
              version,
              description: entry.description ?? null,
              author: entry.author ?? null,
              provides: entry.provides ?? [],
              net: entry.permissions?.net ?? [],
              contentRating: entry.contentRating ?? 'safe',
              adult: entry.contentRating === 'nsfw',
              lang: entry.lang ?? null,
              size: entry.size ?? 0,
              state: !have
                ? 'available'
                : order < 0
                  ? 'updateAvailable'
                  : order > 0
                    ? 'newerInstalled'
                    : 'installed',
              nuclear: false,
              installedVersion: have?.manifest.version ?? null,
              icon: httpUrl(entry.icon),
              unsupported,
            };
          });
          return { repo, entries, error: null };
        } catch (error) {
          return {
            repo,
            entries: [],
            error: error instanceof Error ? error.message : String(error),
          };
        }
      }),
    );
  }

  /** Downloads, checks and keeps one extension from a repository. */
  async install(repoId: string, extensionId: string): Promise<void> {
    const repo = this.repos().find((candidate) => candidate.id === repoId);
    if (!repo) throw new Error('That repository isn’t added any more.');
    const index = await this.index(repo.url);
    const entry = index.extensions?.find((candidate) => candidate.id === extensionId);
    if (!entry?.package) throw new Error('That source isn’t in the repository any more.');
    const why = this.unsupported(entry);
    if (why) throw new Error(why);
    const address = new URL(entry.package, repo.url).href;
    let response: Response;
    try {
      response = await fetch(address, { credentials: 'omit' });
    } catch {
      throw new Error(`${new URL(address).hostname} doesn't let web pages download it`);
    }
    if (!response.ok) throw new Error(`HTTP ${response.status} from ${address}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (entry.sha256) {
      const digest = hexOf(await crypto.subtle.digest('SHA-256', bytes));
      if (digest !== entry.sha256.toLowerCase()) {
        throw new Error('The package doesn’t match its repository (its checksum differs).');
      }
    }
    const files = await readZip(bytes);
    const manifestFile = files.get('raygun-extension.json');
    if (!manifestFile) throw new Error('The package has no manifest.');
    const manifest = JSON.parse(new TextDecoder().decode(manifestFile)) as Manifest;
    if (manifest.id !== extensionId) throw new Error('The package names another source.');
    if ((manifest.runtime ?? 'js') !== 'js') throw new Error(NOT_HERE);
    const entryFile = files.get(manifest.entry ?? 'dist/index.js');
    if (!entryFile) throw new Error('The package has no script.');
    const before = this.context.store.get<InstalledExtension>('extensions', extensionId);
    this.stopOne(extensionId);
    this.context.store.put('extensions', extensionId, {
      manifest,
      source: new TextDecoder().decode(entryFile),
      repoId,
      repository: repo.name,
      installedAt: this.context.now(),
      values: before?.values ?? {},
    } satisfies InstalledExtension);
  }

  // ---- The install queue ----

  installStatus(): ExtensionInstallStatus {
    const items = this.queue;
    const current = items.find((item) => item.state === 'installing');
    return {
      batch: this.batch,
      items,
      total: items.length,
      done: items.filter((item) => item.state === 'done').length,
      failed: items.filter((item) => item.state === 'failed').length,
      cancelled: 0,
      running: this.running,
      current: current?.extensionId ?? null,
      phase: current ? 'downloading' : null,
    };
  }

  enqueue(requests: ExtensionInstallRequest[]): ExtensionInstallStatus {
    if (!this.running) {
      this.batch += 1;
      this.queue = this.queue.filter((item) => item.state === 'queued');
    }
    for (const request of requests) {
      if (
        this.queue.some(
          (item) => item.extensionId === request.extensionId && item.state === 'queued',
        )
      ) {
        continue;
      }
      this.queue.push({
        ...request,
        state: 'queued',
        phase: null,
        error: null,
        finishedAt: null,
        batch: this.batch,
      });
    }
    void this.work();
    return this.installStatus();
  }

  dismiss(): ExtensionInstallStatus {
    if (!this.running) this.queue = [];
    return this.installStatus();
  }

  private async work() {
    if (this.running) return;
    this.running = true;
    for (;;) {
      const next = this.queue.find((item) => item.state === 'queued');
      if (!next) break;
      next.state = 'installing';
      this.context.emit('extensionInstallsEvent', this.installStatus());
      try {
        await this.install(next.repoId, next.extensionId);
        next.state = 'done';
      } catch (error) {
        next.state = 'failed';
        next.error = error instanceof Error ? error.message : String(error);
      }
      next.finishedAt = this.context.now();
    }
    this.running = false;
    for (const extension of this.installed()) this.sandboxFor(extension);
    await this.start();
    this.context.emit('extensionInstallsEvent', this.installStatus());
    this.changed();
  }

  // ---- Using sources ----

  /** Every loaded provider of `kind`, the content filter applied. */
  private async providersOf(kind: string) {
    await this.start();
    const showNsfw = this.settings().showNsfw;
    const disabled = new Set(this.settings().disabledSources);
    return this.installed().flatMap((extension) => {
      const id = extension.manifest.id;
      const sandbox = this.sandboxes.get(id);
      if (!sandbox || this.failures.has(id) || disabled.has(id)) return [];
      if (this.adult(extension.manifest) && !showNsfw) return [];
      return sandbox.providers
        .filter((provider) => provider.kind === kind && !disabled.has(`${id}/${provider.id}`))
        .map((provider) => ({ extension, sandbox, provider }));
    });
  }

  /** Song search through every `music.metadata` source (Search → Music). */
  async searchMusic(query: string): Promise<ProviderSearchResult_Serialize[]> {
    const explicitOk = this.settings().explicitMusic;
    const sources = await this.providersOf('music.metadata');
    return Promise.all(
      sources.map(async ({ extension, sandbox, provider }) => {
        const info: ProviderInfo_Serialize = {
          id: `${extension.manifest.id}/${provider.id}`,
          extensionId: extension.manifest.id,
          kind: provider.kind,
          name: provider.name ?? extension.manifest.name,
          searchPill: provider.searchPill ?? null,
          adult: this.adult(extension.manifest),
        };
        try {
          const answer = await sandbox.call<{ tracks?: unknown[] }>(provider.id, 'search', {
            query,
            limit: 20,
          });
          const tracks = (answer?.tracks ?? [])
            .map(trackOf)
            .filter((track): track is OnlineTrack => track !== null)
            .filter((track) => explicitOk || !track.explicit);
          return { provider: info, tracks, error: null, fromListings: false };
        } catch (error) {
          return {
            provider: info,
            tracks: [],
            error: error instanceof Error ? error.message : String(error),
            fromListings: false,
          };
        }
      }),
    );
  }

  /** A stream for a song found elsewhere: `music.stream` sources' best candidate. */
  async streamFor(item: QueueItem): Promise<MediaSource | null> {
    const wanted = {
      title: item.source.kind === 'lookup' ? item.source.title : item.title,
      artist: item.source.kind === 'lookup' ? item.source.artist : item.artist,
    };
    const sources = await this.providersOf('music.stream');
    const answers = await Promise.all(
      sources.map(async ({ sandbox, provider }) => {
        try {
          const answer = await sandbox.call<{ candidates?: Array<Record<string, unknown>> }>(
            provider.id,
            'candidates',
            {
              title: wanted.title,
              artist: wanted.artist,
              album: item.album,
              durationMs: item.durationMs,
              limit: 5,
            },
          );
          return (answer?.candidates ?? []).map((candidate) => ({ candidate, sandbox, provider }));
        } catch {
          return [];
        }
      }),
    );
    const ranked = answers
      .flat()
      .map((found) => ({
        ...found,
        score: matchScore(wanted, {
          title: String(found.candidate.title ?? ''),
          artist: typeof found.candidate.artist === 'string' ? found.candidate.artist : null,
        }),
      }))
      .filter((found) => found.score >= 0.6)
      .sort((a, b) => b.score - a.score);
    for (const found of ranked) {
      const direct = mediaSourceOf(found.candidate.source);
      if (direct) return direct;
      try {
        const resolved = await found.sandbox.call<{ source?: unknown }>(
          found.provider.id,
          'resolve',
          {
            id: String(found.candidate.id),
          },
        );
        const source = mediaSourceOf(resolved?.source);
        if (source) return source;
      } catch {
        // The next candidate.
      }
    }
    return null;
  }
}
