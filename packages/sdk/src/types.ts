// Types for Raygun extensions (ARCHITECTURE §7). They describe the `raygun` global that the
// extension host installs (crates/raygun-ext-host/src/prelude.js) and the provider contracts the
// app calls; keep them in step with both.

export type JsonValue =
  null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

// ---- Models ----

/**
 * Where a track's audio comes from. Extensions may hand out web streams (the stream proxy sends
 * `headers`, such as Referer, for them) and YouTube video ids; local files are never accepted.
 */
export type MediaSource =
  | { kind: 'url'; url: string; headers: Record<string, string> }
  | { kind: 'youTube'; videoId: string };

/** A track found by a provider. Without a `source` it is shown but not playable yet. */
export type Track = {
  /** The provider's own id for the track. */
  id?: string | number;
  title: string;
  artist?: string | null;
  album?: string | null;
  durationMs?: number | null;
  /** http(s) only. */
  artworkUrl?: string | null;
  source?: MediaSource | null;
};

// ---- Providers ----

export type SearchArgs = { query: string; limit: number };
export type SearchResult = { tracks: Track[] };

/** Online music search. The app asks every `music.metadata` provider at once. */
export type MusicMetadataProvider = {
  kind: 'music.metadata';
  /** Unique within the extension: `[a-z0-9][a-z0-9_-]*`. */
  id: string;
  /** Shown as the heading of this provider's results; defaults to the extension's name. */
  name?: string;
  search(args: SearchArgs): Promise<SearchResult>;
};

export type Provider = MusicMetadataProvider;
export type ProviderKind = Provider['kind'];

// ---- Host API ----

export type FetchInit = {
  method?: string;
  headers?: Record<string, string>;
  body?: string | null;
};

export type FetchResponse = {
  status: number;
  /** `status` is 2xx. */
  ok: boolean;
  /** Final URL, after redirects. */
  url: string;
  /** Lower-case names; repeated headers are joined with `, `. */
  headers: Record<string, string>;
  text(): Promise<string>;
  json<T = unknown>(): Promise<T>;
};

export type HtmlElement = {
  /** All descendant text, concatenated. */
  text: string;
  /** Outer HTML. */
  html: string;
  inner: string;
  attrs: Record<string, string>;
  attr(name: string): string | null;
  /** Selects inside this element (re-parsed as a fragment; select table rows from the document). */
  select(selector: string): HtmlElement[];
};

export type RaygunApi = {
  apiVersion: 1;
  log: Record<'debug' | 'info' | 'warn' | 'error', (...args: unknown[]) => void>;
  http: {
    /** Only hosts in the manifest's `permissions.net` (redirects included); bodies up to 8 MiB. */
    fetch(url: string, init?: FetchInit): Promise<FetchResponse>;
  };
  html: {
    select(html: string, selector: string): HtmlElement[];
  };
  /** Per-extension storage, kept across app restarts and extension updates; up to 1 MiB. */
  storage: {
    get<T extends JsonValue = JsonValue>(key: string): T | null;
    set(key: string, value: JsonValue): void;
    remove(key: string): void;
    keys(): string[];
  };
  settings: {
    /** The user's value, else the manifest default, else `null`. */
    get<T extends JsonValue = JsonValue>(key: string): T | null;
  };
  providers: {
    register(provider: Provider): void;
  };
};

// ---- Manifest (raygun-extension.json) ----

export type SettingDef = {
  key: string;
  type: 'text' | 'number' | 'boolean' | 'select';
  default?: JsonValue;
  label?: string;
  options?: string[];
};

export type Manifest = {
  /** Reverse-DNS, lower case: `dev.example.my-source`. */
  id: string;
  name: string;
  /** Semver. */
  version: string;
  apiVersion: 1;
  runtime: 'js';
  /** The bundle `raygun-ext build` writes, normally `dist/index.js`. */
  entry: string;
  description?: string;
  author?: string;
  provides: ProviderKind[];
  permissions?: {
    /** `api.example.org` or `*.example.org`. */
    net?: string[];
    webview?: boolean;
  };
  contentRating?: 'safe' | 'suggestive' | 'nsfw';
  settings?: SettingDef[];
};
