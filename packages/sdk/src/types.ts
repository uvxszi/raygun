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

/** An artist a source knows (M46.9). */
export type Artist = {
  /** The source's own id. */
  id: string;
  name: string;
  /** http(s) only. */
  imageUrl?: string | null;
};

/** An album or single a source knows (M46.9). */
export type Album = {
  /** The source's own id, passed back to `album`. */
  id: string;
  title: string;
  artist?: string | null;
  year?: number | null;
  /** http(s) only. */
  artworkUrl?: string | null;
  trackCount?: number | null;
};

/** Online music search. The app asks every `music.metadata` provider at once. */
export type MusicMetadataProvider = {
  kind: 'music.metadata';
  /** Unique within the extension: `[a-z0-9][a-z0-9_-]*`. */
  id: string;
  /** Shown as the heading of this provider's results; defaults to the extension's name. */
  name?: string;
  search(args: SearchArgs): Promise<SearchResult>;
  /** Artists by name (M46.9); Search → Music → Artists. */
  searchArtists?(args: SearchArgs): Promise<{ artists: Artist[] }>;
  /** Albums by title or artist (M46.9); Search → Music → Albums. */
  searchAlbums?(args: SearchArgs): Promise<{ albums: Album[] }>;
  /** An album's songs, in order (M46.9). */
  album?(args: { id: string }): Promise<{ album: Album; tracks: Track[] }>;
};

/** The track the app wants a stream for (M14). */
export type StreamQuery = {
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
  limit: number;
  /**
   * The bitrate the user's Audio quality setting asks for, in kbps (M46.18): streaming, mobile
   * data or downloads. `null` asks for the best there is, lossless included; absent means no
   * preference. Providers with several streams of a song pick the nearest.
   */
  targetKbps?: number | null;
};

/** A stream a provider can offer for a track. Without a `source`, the app calls `resolve` later. */
export type StreamCandidate = {
  /** Passed back to `resolve`. */
  id: string | number;
  /** As the source names it; the app matches it against the track. */
  title: string;
  artist?: string | null;
  durationMs?: number | null;
  /** Shown to the user, e.g. "VBR MP3". A bitrate in it ("320 kbps", "FLAC") counts too. */
  quality?: string | null;
  /** The stream's bitrate (M46.18): among equal matches, the app plays the nearest to the setting. */
  bitrateKbps?: number | null;
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

// ---- Music discovery (M46.9) ----

/** A chart a source keeps: its most played, trending or a genre's top songs. */
export type Chart = {
  /** Passed back to `chart`. */
  id: string;
  name: string;
  description?: string | null;
  /** http(s) only; the app uses the first song's artwork without one. */
  artworkUrl?: string | null;
  /** The genre the chart belongs to ("Rock"), so the genre's page can show it. */
  tag?: string | null;
};

/** Where the user is, as an ISO 3166 country code ("us"), when the source has charts by country. */
export type ChartArgs = { region: string | null };

/** Popular and trending songs (M46.9). Songs without a `source` play through `music.stream`. */
export type MusicChartsProvider = {
  kind: 'music.charts';
  id: string;
  name?: string;
  charts(args: ChartArgs): Promise<{ charts: Chart[] }>;
  chart(args: { id: string; region: string | null; limit: number }): Promise<{ tracks: Track[] }>;
};

/** A genre a source gives a song ("Hip-Hop", "Breakcore"); a higher weight is more certain. */
export type MusicTag = { name: string; weight?: number | null };

/** Genres for a song (M46.9); the app derives moods from them. Return no tags when unsure. */
export type MusicTagsProvider = {
  kind: 'music.tags';
  id: string;
  name?: string;
  tags(query: LyricsQuery): Promise<{ tags: MusicTag[] }>;
};

/** What open databases say about an artist (M46.14). */
export type ArtistInfo = {
  name: string;
  /** "Person", "Group"... */
  kind?: string | null;
  /** A few sentences, plain text. */
  bio?: string | null;
  imageUrl?: string | null;
  /** Where to read more. */
  url?: string | null;
  origin?: string | null;
  /** "1975–", "1962–1970". */
  active?: string | null;
  members?: string[];
};

/** People with one role on a song: "Written by", "Produced by", "Guitar"... */
export type CreditGroup = { role: string; names: string[] };

/** Artist facts and song credits for the player's cards (M46.14). Answer null or no credits when
 * unsure; the app asks the next provider. */
export type MusicInfoProvider = {
  kind: 'music.info';
  id: string;
  name?: string;
  artist?(args: { name: string }): Promise<{ artist: ArtistInfo | null }>;
  credits?(query: LyricsQuery): Promise<{ credits: CreditGroup[]; url?: string | null }>;
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
  /** Chapters: the player marks them and offers to skip openings, endings and recaps. */
  timestamps?: VideoTimestamp[];
};

/** A chapter of a video, in seconds from the start (Aniyomi's `TimeStamp`). */
export type VideoTimestamp = {
  start: number;
  /** The next chapter's start when left out. */
  end?: number;
  name?: string;
  type?: 'opening' | 'ending' | 'recap' | 'mixedOp' | 'mixedEd' | 'other';
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
  | MusicChartsProvider
  | MusicTagsProvider
  | MusicInfoProvider
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
  /** The site the sources read (http or https); a source's menu opens it in Raygun's browser. */
  website?: string;
};
