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
  /** The source marks it explicit (M46.36): hidden while the user's Explicit music is off. */
  explicit?: boolean | null;
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
  /** The release's kind (M46.36): an artist's discography groups by it. */
  kind?: 'album' | 'single' | 'ep' | 'compilation' | 'live' | 'other' | null;
};

/** A playlist a source has (M46.39). */
export type Playlist = {
  /** The source's own id, passed back to `playlist`. */
  id: string;
  title: string;
  /** Who made it. */
  author?: string | null;
  /** http(s) only. */
  artworkUrl?: string | null;
  trackCount?: number | null;
  /** One of the service's own (editorial) playlists. */
  featured?: boolean | null;
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
  /** An artist's releases, newest first (M46.36); `query` is the artist's name. */
  artistReleases?(args: SearchArgs): Promise<{ releases: Album[] }>;
  /** An artist's coming events, soonest first (M46.42); `query` is the artist's name. */
  artistEvents?(args: SearchArgs): Promise<{
    events: { name: string; date: string; place?: string | null; url?: string | null }[];
  }>;
  /** Playlists by name (M46.39); `featured` asks for the service's own only. */
  searchPlaylists?(args: SearchArgs & { featured: boolean }): Promise<{ playlists: Playlist[] }>;
  /** A playlist's songs, in order (M46.39). */
  playlist?(args: { id: string }): Promise<{ playlist: Playlist; tracks: Track[] }>;
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

/**
 * The lyrics in another language, or romanized (the original's sounds in Latin letters), line for
 * line: `lines[i]` belongs to `synced[i]` as returned, or to the i-th line of `plain`. Use `''`
 * for a line without one.
 */
export type LyricsTranslationResult = {
  /** BCP 47 tag: the translation's language (`en`, `zh-Hans`); for a romanization, the original's with `-Latn` (`ja-Latn`). */
  language: string;
  /** `translation` when left out. */
  kind?: 'translation' | 'romanization';
  lines: string[];
};

export type LyricsResult = {
  /** Timed lines; the app highlights the current one and seeks when one is tapped. */
  synced?: { timeMs: number; text: string }[];
  plain?: string | null;
  /** Translations and romanizations the source has; shown under the lines. */
  translations?: LyricsTranslationResult[];
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
  /**
   * What kind of chart it is (M46.19), for grouping in Browse. Without one, a chart with a `tag`
   * counts as `genre`, else `top`.
   */
  group?: ChartGroup | null;
};

export type ChartGroup =
  'top' | 'trending' | 'new' | 'viral' | 'genre' | 'mood' | 'decade' | 'underground';

/** A genre or mood the app wants songs for (M46.19). */
export type TagSongsArgs = {
  /** The app's name for it ("Hip-Hop", "Chill"). */
  tag: string;
  /** Other spellings sources use ("hip hop", "rap"), lowercase. */
  aliases: string[];
  kind: 'genre' | 'mood';
  region: string | null;
  limit: number;
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
  /**
   * Songs for any genre or mood (M46.19), shown as a row on its page, named `name` (default
   * "<tag> on <source>"). Answer no tracks when the source has nothing for it.
   */
  tagSongs?(args: TagSongsArgs): Promise<{ tracks: Track[]; name?: string | null }>;
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

/** One comment on a video (M46.45). */
export type VideoComment = {
  author: string;
  avatarUrl?: string | null;
  text: string;
  /** As the site shows it: "220K". */
  likes?: string | null;
  /** As the site shows it: "6 years ago". */
  published?: string | null;
  replies?: number | null;
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
  /**
   * Gives the source its own pill in Search (M46.35), named this ("YouTube"); its results then
   * stay out of the Videos pill.
   */
  searchPill?: string;
  popular(args: { page: number }): Promise<MangaPage>;
  latest?(args: { page: number }): Promise<MangaPage>;
  search(args: CatalogSearchArgs): Promise<MangaPage>;
  filters?(): Promise<SourceFilter[]>;
  details(args: { id: string }): Promise<VideoDetails>;
  /** Episodes in order; a movie has one. */
  episodes(args: { id: string }): Promise<{ episodes: VideoEpisode[] }>;
  servers(args: { episodeId: string }): Promise<{ servers: VideoServer[] }>;
  videos(args: { serverId: string }): Promise<{ videos: Video[] }>;
  /**
   * A title's comments, a page at a time (M46.45): `page` is the `next` the last page gave, left
   * out for the first.
   */
  comments?(args: {
    id: string;
    page?: string | null;
  }): Promise<{ comments: VideoComment[]; next?: string | null }>;
};

/**
 * Recommendations (M46.20) for Made for You and song radios: artists like an artist, an artist's
 * best-known songs, their latest releases, and radios. Every call is optional; songs without a
 * `source` play through `music.stream`.
 */
export type MusicRecommendProvider = {
  kind: 'music.recommend';
  id: string;
  name?: string;
  /** Artists like this one, the closest first. */
  similarArtists?(args: { artist: string; limit: number }): Promise<{ artists: Artist[] }>;
  /** The artist's most played songs. */
  topTracks?(args: { artist: string; limit: number }): Promise<{ tracks: Track[] }>;
  /** Songs from the artist's releases of the last `days` days, newest first. */
  newReleases?(args: { artist: string; days: number; limit: number }): Promise<{ tracks: Track[] }>;
  /** Songs like the artist's (and like this song, when `title` is given). */
  radio?(args: {
    artist: string;
    title: string | null;
    limit: number;
  }): Promise<{ tracks: Track[] }>;
};

/** A book an audiobook directory lists (M46.22); `feedUrl` is its chapter feed (RSS). */
export type Audiobook = {
  id: string;
  title: string;
  author?: string | null;
  feedUrl: string;
  /** http(s) only. */
  artworkUrl?: string | null;
  /** Its length as the source writes it ("13:06:44"). */
  runtime?: string | null;
  language?: string | null;
  description?: string | null;
};

export type AudiobookPage = { books: Audiobook[]; hasMore: boolean };

/**
 * Audiobooks to find (M46.22): searches and lists, a page at a time (from 1). The app reads each
 * book's chapter feed itself.
 */
export type AudiobookDirectoryProvider = {
  kind: 'audiobook.directory';
  id: string;
  name?: string;
  search(args: { query: string; page: number }): Promise<AudiobookPage>;
  popular(args: { page: number }): Promise<AudiobookPage>;
  latest?(args: { page: number }): Promise<AudiobookPage>;
  subject?(args: { subject: string; page: number }): Promise<AudiobookPage>;
};

export type Provider =
  | AudiobookDirectoryProvider
  | MusicRecommendProvider
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
