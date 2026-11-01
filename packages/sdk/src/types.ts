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

/** A track found by a provider. Without a `source` it plays through `music.stream` providers. */
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

/** The track the app wants a stream for (M14). */
export type StreamQuery = {
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
  limit: number;
};

/** A stream a provider can offer for a track. Without a `source`, the app calls `resolve` later. */
export type StreamCandidate = {
  /** Passed back to `resolve`. */
  id: string | number;
  /** As the source names it; the app matches it against the track. */
  title: string;
  artist?: string | null;
  durationMs?: number | null;
  /** Shown to the user, e.g. "VBR MP3". */
  quality?: string | null;
  source?: MediaSource | null;
};

export type ResolvedStream = {
  source: MediaSource;
  /** Unix milliseconds after which the URL stops working (signed URLs). */
  expiresAt?: number | null;
};

/**
 * Streams for tracks found elsewhere. The app asks every `music.stream` provider at once, scores
 * the candidates against the track and plays the best, falling back to the next on failure.
 */
export type MusicStreamProvider = {
  kind: 'music.stream';
  id: string;
  name?: string;
  candidates(query: StreamQuery): Promise<{ candidates: StreamCandidate[] }>;
  /** Only needed when candidates come without a `source`. */
  resolve?(args: { id: string }): Promise<ResolvedStream>;
};

export type LyricsQuery = {
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
};

export type LyricsResult = {
  /** Timed lines; the app highlights the current one and seeks when one is tapped. */
  synced?: { timeMs: number; text: string }[];
  plain?: string | null;
};

/** Lyrics for the playing track. Return `null` when there are none (or the track is instrumental). */
export type LyricsProvider = {
  kind: 'lyrics';
  id: string;
  name?: string;
  lyrics(query: LyricsQuery): Promise<LyricsResult | null>;
};

// ---- Manga (M24) ----

/** A title in a catalogue list. */
export type MangaSummary = {
  /** The source's own id, passed back to `details` and `chapters`. */
  id: string;
  title: string;
  /** http(s) only. */
  coverUrl?: string | null;
  /** Sent when the app fetches the cover, e.g. a User-Agent or Referer the host requires. */
  coverHeaders?: Record<string, string>;
  /** An adult title; the app hides it while NSFW content is off. */
  nsfw?: boolean;
};

export type MangaPage = { items: MangaSummary[]; hasNextPage: boolean };

export type MangaStatus =
  'ongoing' | 'completed' | 'hiatus' | 'cancelled' | 'publishingFinished' | 'unknown';

export type MangaDetails = {
  id?: string;
  title: string;
  coverUrl?: string | null;
  coverHeaders?: Record<string, string>;
  authors?: string[];
  artists?: string[];
  description?: string | null;
  genres?: string[];
  status?: MangaStatus;
  /** The title's page on the source's website. */
  url?: string | null;
};

export type MangaChapter = {
  /** Passed back to `pages`. */
  id: string;
  /** Defaults to "Chapter <number>". */
  name?: string | null;
  number?: number | null;
  volume?: string | null;
  scanlator?: string | null;
  /** Unix milliseconds. */
  dateUpload?: number | null;
};

/** One page image; `headers` (e.g. Referer) are sent when the app fetches it. */
export type PageImage = string | { url: string; headers?: Record<string, string> };

/**
 * A manga catalogue (Mihon-shaped). List chapters in the source's order, usually newest first;
 * the app reads by chapter number.
 */
// ---- Source filters (M40) ----

export type FilterOption = { value: string; label: string };

/** A filter a source offers for its search; the app draws them and passes the choices back. */
export type SourceFilter =
  | { kind: 'select'; key: string; label: string; options: FilterOption[]; default?: string }
  /** Tags the user can include or exclude. */
  | { kind: 'tags'; key: string; label: string; options: FilterOption[] }
  | { kind: 'toggle'; key: string; label: string; default?: boolean };

/** The user's choices, by filter key; only filters the user changed are present. */
export type FilterValues = Record<
  string,
  string | boolean | { include: string[]; exclude: string[] }
>;

/** What `search` receives; `query` may be empty when filters are set. */
export type CatalogSearchArgs = { query: string; page: number; filters?: FilterValues };

export type MangaSourceProvider = {
  kind: 'manga.source';
  id: string;
  name?: string;
  popular(args: { page: number }): Promise<MangaPage>;
  latest?(args: { page: number }): Promise<MangaPage>;
  search(args: CatalogSearchArgs): Promise<MangaPage>;
  /** The filters `search` understands (M40). */
  filters?(): Promise<SourceFilter[]>;
  details(args: { id: string }): Promise<MangaDetails>;
  chapters(args: { id: string }): Promise<{ chapters: MangaChapter[] }>;
  pages(args: { chapterId: string }): Promise<{ pages: PageImage[] }>;
};

// ---- Novels (M31) ----

/**
 * A chapter's text. Prefer `html`: the app keeps paragraphs, headings, rules and images (resolved
 * against `baseUrl`) and drops everything else. `text` is split into paragraphs at blank lines.
 */
export type Passage = { html?: string; text?: string; baseUrl?: string };

/**
 * A novel catalogue: the calls and shapes of `manga.source`, with `passage` in place of `pages`.
 * List chapters in reading order or newest first; the app reads by chapter number.
 */
export type NovelSourceProvider = {
  kind: 'novel.source';
  id: string;
  name?: string;
  popular(args: { page: number }): Promise<MangaPage>;
  latest?(args: { page: number }): Promise<MangaPage>;
  search(args: CatalogSearchArgs): Promise<MangaPage>;
  filters?(): Promise<SourceFilter[]>;
  details(args: { id: string }): Promise<MangaDetails>;
  chapters(args: { id: string }): Promise<{ chapters: MangaChapter[] }>;
  passage(args: { chapterId: string }): Promise<Passage>;
};

// ---- Podcasts (M37) ----

/** A podcast a directory found; the app reads the feed itself. */
export type PodcastResult = {
  title: string;
  author?: string | null;
  /** The RSS/Atom feed, http(s) only. */
  feedUrl: string;
  /** Square artwork, http(s) only. */
  artworkUrl?: string | null;
  description?: string | null;
};

export type PodcastDirectoryProvider = {
  kind: 'podcast.directory';
  id: string;
  name?: string;
  search(args: { query: string; limit: number }): Promise<PodcastResult[]>;
  /** Popular podcasts, shown before the user searches. */
  top?(args: { limit: number }): Promise<PodcastResult[]>;
};

// ---- Video (M42) ----

export type VideoType = 'movie' | 'series' | 'anime' | 'other';

/** A title's details: the fields of `MangaDetails` (authors and artists unused) and these. */
export type VideoDetails = Omit<MangaDetails, 'authors' | 'artists' | 'status'> & {
  type?: VideoType;
  year?: number | null;
  cast?: string[];
  directors?: string[];
  status?: MangaStatus;
};

export type VideoEpisode = {
  /** Passed back to `servers`. */
  id: string;
  /** Defaults to "Episode <number>". A movie's single episode can be named "Film". */
  name?: string | null;
  number?: number | null;
  season?: number | null;
  /** Unix milliseconds. */
  dateUpload?: number | null;
};

export type VideoSubtitle = { url: string; lang?: string; label?: string };

/** One playable link; `headers` (e.g. Referer) are sent with every request the player makes. */
export type Video = {
  /** http(s) only. */
  url: string;
  /** "1080p", "Dub", ...; defaults to the quality. */
  label?: string;
  /** Height in pixels; the app orders links by the user's preferred quality. */
  quality?: number;
  headers?: Record<string, string>;
  /** Streaming manifests; leave it out for a plain video file. */
  format?: 'hls' | 'dash' | 'progressive';
  subtitles?: VideoSubtitle[];
};

/** A server (hoster) for an episode. Include `videos` when they cost nothing to resolve. */
export type VideoServer = { id: string; name?: string; videos?: Video[] };

/**
 * A video catalogue (aniyomi-shaped): the catalogue calls of `manga.source`, then an episode's
 * servers, each resolved on its own with `videos`, so slow or broken hosts don't hold up the rest.
 */
export type VideoSourceProvider = {
  kind: 'video.source';
  id: string;
  name?: string;
  popular(args: { page: number }): Promise<MangaPage>;
  latest?(args: { page: number }): Promise<MangaPage>;
  search(args: CatalogSearchArgs): Promise<MangaPage>;
  filters?(): Promise<SourceFilter[]>;
  details(args: { id: string }): Promise<VideoDetails>;
  /** Episodes in order; a movie has one. */
  episodes(args: { id: string }): Promise<{ episodes: VideoEpisode[] }>;
  servers(args: { episodeId: string }): Promise<{ servers: VideoServer[] }>;
  videos(args: { serverId: string }): Promise<{ videos: Video[] }>;
};

export type Provider =
  | MusicMetadataProvider
  | MusicStreamProvider
  | LyricsProvider
  | MangaSourceProvider
  | NovelSourceProvider
  | PodcastDirectoryProvider
  | VideoSourceProvider;
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
  time: {
    /** Waits without using CPU, e.g. to respect an API's rate limit. Capped at 10 s per call. */
    sleep(ms: number): Promise<void>;
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
  /** The app-wide content switch. */
  content: {
    /**
     * Whether the user shows NSFW (18+) content; off by default. While it is off, leave adult
     * results out (or mark them `nsfw`, and the app hides them).
     */
    showNsfw(): boolean;
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
