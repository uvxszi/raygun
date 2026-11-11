// Types for Raygun extensions (ARCHITECTURE §7). They describe the `raygun` global that the
// extension host installs (crates/raygun-ext-host/src/prelude.js) and the provider contracts the
// app calls, as the host's parsers read them (crates/raygun-ext-host/src: host.rs, manga.rs,
// novel.rs, video.rs, podcast.rs, audiobook.rs); keep them in step with both. The guide is
// docs/extensions.
//
// How Raygun reads every answer: entries missing what they need are dropped one by one (the
// rest still show), text is trimmed and empty text counts as missing, and addresses must be
// http(s) unless a field says otherwise. Throw an `Error` with a plain sentence when a call
// can't be answered; the app shows it under the source's name.

export type JsonValue =
  null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

// ---- Models ----

/**
 * Where a track's audio comes from. Extensions may hand out web streams (the stream proxy sends
 * `headers`, such as Referer, for them) and YouTube video ids; local files are never accepted.
 */
export type MediaSource =
  /** `headers` is required: pass `{}` when the stream needs none, or the track is dropped. */
  | { kind: 'url'; url: string; headers: Record<string, string> }
  | { kind: 'youTube'; videoId: string };

/**
 * A song a provider found. Without a `source` it plays through `music.stream` providers: Raygun
 * asks them for the title and artist when it plays.
 */
export type Track = {
  /** The provider's own id for the song. */
  id?: string | number;
  /** Required; a track without one is dropped. */
  title: string;
  artist?: string | null;
  album?: string | null;
  /** The length in milliseconds. */
  durationMs?: number | null;
  /** http(s) only. */
  artworkUrl?: string | null;
  source?: MediaSource | null;
  /** The source marks it explicit (M46.36): hidden while the user's Explicit music is off. */
  explicit?: boolean | null;
};

// ---- Providers ----

/**
 * What every provider has. `kind` must be listed in the manifest's `provides`, and `id` must be
 * unique within the extension.
 */
type ProviderBase = {
  /** `[a-z0-9][a-z0-9_-]*`, at most 64 characters. Raygun knows it as `<extension id>/<id>`. */
  id: string;
  /** The source's name in lists and messages; defaults to the extension's name. */
  name?: string;
};

/** A search: what was typed and how many results to give at most. */
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
  /** A whole number, like 1999. */
  year?: number | null;
  /** http(s) only. */
  artworkUrl?: string | null;
  /** A whole number. */
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

/** An artist's event: a concert, a festival (M46.42). */
export type ArtistEvent = {
  name: string;
  /** `YYYY-MM-DD`, or as much of it as the source knows. */
  date: string;
  /** The venue and its city. */
  place?: string | null;
  /** The event's page. */
  url?: string | null;
};

/**
 * Online music search. Raygun asks every `music.metadata` provider at once and shows each one's
 * results as they come; a search waits 12 s for a provider. Every call but `search` is optional:
 * Raygun leaves a provider out of what it can't do.
 */
export type MusicMetadataProvider = ProviderBase & {
  kind: 'music.metadata';
  /** Songs for what was typed (Search → Music); answer no tracks for nothing found. */
  search(args: SearchArgs): Promise<SearchResult>;
  /** Artists by name (M46.9); Search → Music → Artists. */
  searchArtists?(args: SearchArgs): Promise<{ artists: Artist[] }>;
  /** Albums by title or artist (M46.9); Search → Music → Albums. */
  searchAlbums?(args: SearchArgs): Promise<{ albums: Album[] }>;
  /**
   * An album's songs, in order (M46.9). An answer without an `album` (id and title) fails: the
   * album page says the source changed.
   */
  album?(args: { id: string }): Promise<{ album: Album; tracks: Track[] }>;
  /** An artist's releases, newest first (M46.36); `query` is the artist's name. */
  artistReleases?(args: SearchArgs): Promise<{ releases: Album[] }>;
  /** An artist's coming events, soonest first (M46.42); `query` is the artist's name. */
  artistEvents?(args: SearchArgs): Promise<{ events: ArtistEvent[] }>;
  /** Playlists by name (M46.39); `featured` asks for the service's own only. */
  searchPlaylists?(args: SearchArgs & { featured: boolean }): Promise<{ playlists: Playlist[] }>;
  /** A playlist's songs, in order (M46.39). An answer without a `playlist` fails. */
  playlist?(args: { id: string }): Promise<{ playlist: Playlist; tracks: Track[] }>;
};

/** The track the app wants a stream for (M14). */
export type StreamQuery = {
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
  /** How many candidates to give at most. */
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
  /** As the source names it; the app matches it against the track. Required. */
  title: string;
  artist?: string | null;
  durationMs?: number | null;
  /** Shown to the user, e.g. "VBR MP3". A bitrate in it ("320 kbps", "FLAC") counts too. */
  quality?: string | null;
  /** The stream's bitrate (M46.18): among equal matches, the app plays the nearest to the setting. */
  bitrateKbps?: number | null;
  /** A source here plays at once; a candidate whose source isn't allowed is dropped. */
  source?: MediaSource | null;
};

export type ResolvedStream = {
  source: MediaSource;
  /** Unix milliseconds after which the URL stops working (signed URLs). */
  expiresAt?: number | null;
};

/** The candidate whose stream the app wants now. */
export type ResolveArgs = {
  /** The candidate's `id`, as text. */
  id: string;
  /** As in {@link StreamQuery} (M46.51): the bitrate to aim for, `null` for the best. */
  targetKbps?: number | null;
  /**
   * Raygun reads the stream itself (a download, captions), not only plays it (M46.51): a provider
   * with several formats of it picks one Raygun decodes (M4A or MP3 over Opus).
   */
  decodable?: boolean;
};

/**
 * Streams for tracks found elsewhere. The app asks every `music.stream` provider at once (12 s
 * each), scores the candidates against the track (title, artist and length) and plays the best,
 * falling back to the next on failure. Answer no candidates rather than poor guesses.
 */
export type MusicStreamProvider = ProviderBase & {
  kind: 'music.stream';
  candidates(query: StreamQuery): Promise<{ candidates: StreamCandidate[] }>;
  /** Only needed when candidates come without a `source`; it has 12 s. */
  resolve?(args: ResolveArgs): Promise<ResolvedStream>;
};

/** The song lyrics, tags or credits are asked for. */
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

/** Lyrics with `synced` lines, `plain` text, or both; neither counts as none. */
export type LyricsResult = {
  /**
   * Timed lines; the app highlights the current one and seeks when one is tapped. Lines with a
   * negative or missing `timeMs` are dropped; the rest are sorted by time.
   */
  synced?: { timeMs: number; text: string }[];
  plain?: string | null;
  /** Translations and romanizations the source has (at most 8); shown under the lines. */
  translations?: LyricsTranslationResult[];
};

/**
 * Lyrics for the playing track. Raygun asks every `lyrics` provider at once (12 s each) and
 * prefers synced lyrics. Answer `null` when there are none (or the track is instrumental).
 */
export type LyricsProvider = ProviderBase & {
  kind: 'lyrics';
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

/**
 * Popular and trending songs (M46.9). Raygun lists every provider's charts in Browse (30 s each)
 * and opens one with `chart`. Songs without a `source` play through `music.stream`.
 */
export type MusicChartsProvider = ProviderBase & {
  kind: 'music.charts';
  charts(args: ChartArgs): Promise<{ charts: Chart[] }>;
  chart(args: { id: string; region: string | null; limit: number }): Promise<{ tracks: Track[] }>;
  /**
   * Songs for any genre or mood (M46.19), shown as a row on its page, named `name` (default
   * "<tag> on <source>"). Answer no tracks when the source has nothing for it; 12 s.
   */
  tagSongs?(args: TagSongsArgs): Promise<{ tracks: Track[]; name?: string | null }>;
};

/**
 * A genre a source gives a song ("Hip-Hop", "Breakcore"), at most 60 characters; `weight` from 0
 * to 1, higher being more certain (1 when left out).
 */
export type MusicTag = { name: string; weight?: number | null };

/**
 * Genres for a song (M46.9); the app maps them onto its own genres and derives moods from them.
 * A background tagger asks every provider (30 s each). Return no tags when unsure.
 */
export type MusicTagsProvider = ProviderBase & {
  kind: 'music.tags';
  tags(query: LyricsQuery): Promise<{ tags: MusicTag[] }>;
};

/** What open databases say about an artist (M46.14). */
export type ArtistInfo = {
  name: string;
  /** "Person", "Group"... */
  kind?: string | null;
  /** A few sentences, plain text (at most 4,000 characters). */
  bio?: string | null;
  /** http(s) only. */
  imageUrl?: string | null;
  /** Where to read more; http(s) only. */
  url?: string | null;
  origin?: string | null;
  /** The years they were active: "1975" while still active, "1962–1970". */
  active?: string | null;
  members?: string[];
};

/** People with one role on a song: "Written by", "Produced by", "Guitar"... */
export type CreditGroup = { role: string; names: string[] };

/**
 * Artist facts and song credits for the player's cards (M46.14). Raygun asks providers one after
 * another (12 s each) until one knows: answer `{ artist: null }` or no credits when unsure.
 */
export type MusicInfoProvider = ProviderBase & {
  kind: 'music.info';
  artist?(args: { name: string }): Promise<{ artist: ArtistInfo | null }>;
  credits?(query: LyricsQuery): Promise<{ credits: CreditGroup[]; url?: string | null }>;
};

// ---- Catalogues: comics (M24), books (M31), videos (M42) ----

/** A title in a catalogue list. */
export type MangaSummary = {
  /** The source's own id, passed back to `details` and `chapters`. Required. */
  id: string;
  /** Required. */
  title: string;
  /** http(s) only. */
  coverUrl?: string | null;
  /** Sent when the app fetches the cover, e.g. a User-Agent or Referer the host requires. */
  coverHeaders?: Record<string, string>;
  /**
   * Who wrote it, when the list's own answer says (send no extra request for it): a book without
   * a cover gets one drawn with its title and authors. At most 6 are kept.
   */
  authors?: string[];
  /** An adult title; the app hides it while NSFW content is off. */
  nsfw?: boolean;
};

/**
 * One page of a list. Pages count from 1; the app asks for the next page while `hasNextPage` is
 * true and the reader scrolls.
 */
export type MangaPage = { items: MangaSummary[]; hasNextPage: boolean };

export type MangaStatus =
  'ongoing' | 'completed' | 'hiatus' | 'cancelled' | 'publishingFinished' | 'unknown';

/** A title's page. Only `title` is required; any other status reads as `unknown`. */
export type MangaDetails = {
  /** Defaults to the id that was asked for. */
  id?: string;
  title: string;
  /** http(s) only. */
  coverUrl?: string | null;
  coverHeaders?: Record<string, string>;
  authors?: string[];
  artists?: string[];
  description?: string | null;
  genres?: string[];
  status?: MangaStatus;
  /** The title's page on the source's website; http(s) only. */
  url?: string | null;
};

export type MangaChapter = {
  /** Passed back to `pages` or `passage`. Required. */
  id: string;
  /** Defaults to "Chapter <number>"; a chapter with neither a name nor a number is dropped. */
  name?: string | null;
  number?: number | null;
  volume?: string | number | null;
  scanlator?: string | null;
  /** Unix milliseconds. */
  dateUpload?: number | null;
};

/**
 * One page image, http(s) only; `headers` (e.g. Referer) are sent when the app fetches it, except
 * Host, Content-Length, Connection and Transfer-Encoding.
 */
export type PageImage = string | { url: string; headers?: Record<string, string> };

/**
 * A chapter that is a PDF document, http(s) only, or some of its pages: `firstPage` and
 * `lastPage` count from 1 and are both included (the whole document when left out). The app
 * fetches it once, draws its pages on the device and reads its text; a download keeps the file.
 */
export type PdfDocument =
  | string
  | {
      url: string;
      headers?: Record<string, string>;
      firstPage?: number;
      lastPage?: number;
    };

// ---- Source filters (M40) ----

/** A choice; `label` defaults to `value`. */
export type FilterOption = { value: string; label?: string };

/**
 * One choice among options, such as a sort order. The app passes the chosen `value` back, only
 * once the user changes it; `default` must be one of the options.
 */
export type SelectFilter = {
  kind: 'select';
  /** Letters, digits, `-` and `_`, at most 40 characters. */
  key: string;
  label: string;
  options: FilterOption[];
  default?: string;
};
/** Tags the user can include or exclude. */
export type TagsFilter = { kind: 'tags'; key: string; label: string; options: FilterOption[] };
/** A switch. */
export type ToggleFilter = { kind: 'toggle'; key: string; label: string; default?: boolean };

/**
 * A filter a source offers for its search; the app draws them and passes the choices back. At
 * most 20 filters of 500 options each; a select or tags filter without options is left out.
 */
export type SourceFilter = SelectFilter | TagsFilter | ToggleFilter;

/**
 * The user's choices, by filter key: a select's value, a toggle's state, or the tags included and
 * excluded. Only filters the user changed are present; read them with `selectValue`,
 * `toggleValue` and `tagsValue` from '@raygun/sdk'.
 */
export type FilterValues = Record<
  string,
  string | boolean | { include: string[]; exclude: string[] }
>;

/**
 * What `search` receives. `query` may be empty when filters are set; Search shows each source's
 * first page as a row and gives it 15 s.
 */
export type CatalogSearchArgs = { query: string; page: number; filters?: FilterValues };

/**
 * A comics catalogue (Mihon-shaped). Lists and searches go a page at a time; a title's chapters
 * come at once, in the source's order (usually newest first): the app reads by chapter number.
 * Calls have 30 s, search rows 15 s.
 */
export type MangaSourceProvider = ProviderBase & {
  kind: 'manga.source';
  /** The source's popular titles; also Search's row before anything is typed. */
  popular(args: { page: number }): Promise<MangaPage>;
  /** Titles updated or added lately. */
  latest?(args: { page: number }): Promise<MangaPage>;
  search(args: CatalogSearchArgs): Promise<MangaPage>;
  /** The filters `search` understands (M40). */
  filters?(): Promise<SourceFilter[]>;
  details(args: { id: string }): Promise<MangaDetails>;
  chapters(args: { id: string }): Promise<{ chapters: MangaChapter[] }>;
  /**
   * A chapter's pages in reading order; an answer without pages fails. A chapter that is a PDF
   * answers `{ pdf }` instead.
   */
  pages(args: { chapterId: string }): Promise<{ pages: PageImage[] } | { pdf: PdfDocument }>;
};

// ---- Novels (M31) ----

/**
 * A chapter's text. Prefer `html`: the app keeps paragraphs, headings, rules and images (resolved
 * against `baseUrl`) and drops everything else, scripts and styles included. Formulas are kept
 * and drawn: MathML, and TeX between `$$…$$`, `\[…\]`, `\(…\)` or `$…$` (MathJax's `math/tex`
 * scripts too). `text` is split into paragraphs at blank lines. At most 2 MB of text. A chapter
 * that is a PDF answers `pdf` instead, which wins over `html` and `text`.
 */
export type Passage = { html?: string; text?: string; baseUrl?: string; pdf?: PdfDocument };

/**
 * A book catalogue: the calls and shapes of `manga.source`, with `passage` in place of `pages`.
 * List chapters in reading order or newest first; the app reads by chapter number.
 */
export type NovelSourceProvider = ProviderBase & {
  kind: 'novel.source';
  popular(args: { page: number }): Promise<MangaPage>;
  latest?(args: { page: number }): Promise<MangaPage>;
  search(args: CatalogSearchArgs): Promise<MangaPage>;
  filters?(): Promise<SourceFilter[]>;
  details(args: { id: string }): Promise<MangaDetails>;
  chapters(args: { id: string }): Promise<{ chapters: MangaChapter[] }>;
  /** A chapter's text; an answer with nothing to read fails. */
  passage(args: { chapterId: string }): Promise<Passage>;
};

// ---- Podcasts (M37) ----

/** A podcast a directory found; the app reads the feed itself when it's opened or followed. */
export type PodcastResult = {
  title: string;
  author?: string | null;
  /** The RSS/Atom feed, http(s) only. Results repeating a feed are dropped. */
  feedUrl: string;
  /** Square artwork, http(s) only. */
  artworkUrl?: string | null;
  description?: string | null;
};

/** Podcasts to find (30 s a call). Both calls answer a plain list, at most 100 results. */
export type PodcastDirectoryProvider = ProviderBase & {
  kind: 'podcast.directory';
  search(args: { query: string; limit: number }): Promise<PodcastResult[]>;
  /** Popular podcasts, shown before the user searches. */
  top?(args: { limit: number }): Promise<PodcastResult[]>;
};

// ---- Video (M42) ----

export type VideoType = 'movie' | 'series' | 'anime' | 'other';

/** A title's details: the fields of `MangaDetails` (authors and artists unused) and these. */
export type VideoDetails = Omit<MangaDetails, 'authors' | 'artists' | 'status'> & {
  type?: VideoType;
  /** From 1800 to 2200. */
  year?: number | null;
  cast?: string[];
  directors?: string[];
  status?: MangaStatus;
};

export type VideoEpisode = {
  /** Passed back to `servers`. Each id once. */
  id: string;
  /**
   * Defaults to "Episode <number>". A movie's single episode can be named "Film". Raygun reads
   * numbers and seasons from names like "S2 E5" when they're left out.
   */
  name?: string | null;
  number?: number | null;
  season?: number | null;
  /** Unix milliseconds. */
  dateUpload?: number | null;
};

/** A subtitle file (WebVTT or SubRip), http(s) only. */
export type VideoSubtitle = { url: string; lang?: string; label?: string };

/** One playable link; `headers` (e.g. Referer) are sent with every request the player makes. */
export type Video = {
  /** http(s), or `youtube:<11-character video id>` to play in YouTube's embedded player. */
  url: string;
  /** "1080p", "Dub", ...; defaults to the quality. */
  label?: string;
  /** Height in pixels (a whole number); the app orders links by the user's preferred quality. */
  quality?: number;
  headers?: Record<string, string>;
  /** Streaming manifests; leave it out (or `progressive`) for a plain video file. */
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

/** One comment on a video (M46.45); `author` and `text` are required. */
export type VideoComment = {
  author: string;
  /** http(s) only. */
  avatarUrl?: string | null;
  text: string;
  /** As the site shows it: "220K". */
  likes?: string | null;
  /** As the site shows it: "6 years ago". */
  published?: string | null;
  /** A whole number. */
  replies?: number | null;
};

/**
 * A server (hoster) for an episode, at most 50. Include `videos` when they cost nothing to
 * resolve: Raygun then never calls `videos` for it.
 */
export type VideoServer = { id: string; name?: string; videos?: Video[] };

/**
 * A video catalogue (aniyomi-shaped): the catalogue calls of `manga.source`, then an episode's
 * servers, each resolved on its own with `videos`, so slow or broken hosts don't hold up the rest.
 * Raygun plays the first link that works and falls back to the next.
 */
export type VideoSourceProvider = ProviderBase & {
  kind: 'video.source';
  /**
   * Gives the source its own pill in Search (M46.35), named this ("Open Videos", at most 24 characters); its results then
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
  /** One server's videos, at most 50. */
  videos(args: { serverId: string }): Promise<{ videos: Video[] }>;
  /**
   * A title's comments, a page at a time (M46.45): `page` is the `next` the last page gave, left
   * out for the first. A comment without an author or a text makes the page unreadable.
   */
  comments?(args: {
    id: string;
    page?: string | null;
  }): Promise<{ comments: VideoComment[]; next?: string | null }>;
};

/**
 * Recommendations (M46.20) for Made for You and song radios: artists like an artist, an artist's
 * best-known songs, their latest releases, and radios. Every call is optional; Raygun asks
 * providers one after another (30 s each) until one has some. Songs without a `source` play
 * through `music.stream`.
 */
export type MusicRecommendProvider = ProviderBase & {
  kind: 'music.recommend';
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

// ---- Audiobooks (M46.22) ----

/**
 * A book an audiobook directory lists (M46.22); `feedUrl` is its chapter feed (RSS), which the
 * app reads like a podcast's. A book without a feed is opened by its `id` through the provider's
 * `chapters` (M46.48).
 */
export type Audiobook = {
  id: string;
  title: string;
  author?: string | null;
  feedUrl?: string | null;
  /** http(s) only. */
  artworkUrl?: string | null;
  /** Its length as the source writes it ("13:06:44"). */
  runtime?: string | null;
  language?: string | null;
  description?: string | null;
};

/** One page of books (at most 100), and whether there are more. */
export type AudiobookPage = { books: Audiobook[]; hasMore: boolean };

/** One chapter of a book a source lists itself (M46.48). */
export type AudiobookChapter = {
  /** Stable within the book; the audio address when left out. */
  id?: string | null;
  title: string;
  /** http(s) only; a chapter without one is dropped. */
  audioUrl: string;
  /** "audio/mpeg"… */
  audioType?: string | null;
  durationMs?: number | null;
  description?: string | null;
};

/** A book with its chapters in order, for books without a feed (M46.48). */
export type AudiobookDetails = {
  title: string;
  author?: string | null;
  description?: string | null;
  artworkUrl?: string | null;
  /** The book's page on the source's site. */
  link?: string | null;
  /** At most 2,000; a book without any fails to open. */
  chapters: AudiobookChapter[];
};

/**
 * Audiobooks to find (M46.22): searches and lists, a page at a time (from 1), 12 s a call. The app
 * reads each book's chapter feed itself; sources whose books have no feed list a book's chapters
 * with `chapters` (M46.48). `subject` is a genre by its English name ("Mystery", "Science
 * fiction", "Children", "Poetry"…), which each source matches to its own.
 */
export type AudiobookDirectoryProvider = ProviderBase & {
  kind: 'audiobook.directory';
  search(args: { query: string; page: number }): Promise<AudiobookPage>;
  popular(args: { page: number }): Promise<AudiobookPage>;
  latest?(args: { page: number }): Promise<AudiobookPage>;
  subject?(args: { subject: string; page: number }): Promise<AudiobookPage>;
  chapters?(args: { id: string }): Promise<AudiobookDetails>;
};

// ---- Dictionaries: words looked up in the book reader ----

/** How a word sounds: IPA, a recording, or both. */
export type DictionaryPronunciation = {
  /** With its slashes or brackets: `/ˈhæpi/`. */
  ipa?: string | null;
  /** Whose: an accent or a region (`UK`). */
  label?: string | null;
  /** http(s) only. */
  audioUrl?: string | null;
};

/** One sense, with sentences using it. */
export type DictionaryDefinition = { text: string; examples?: string[] };

/** The word in another language, for one sense (`gloss`). */
export type DictionaryTranslation = { language: string; terms: string[]; gloss?: string | null };

/** The word as one part of speech. */
export type DictionaryMeaning = {
  /** `noun`, `verb`… */
  partOfSpeech: string;
  /** A sense without examples can be just its text. Meanings without any are dropped. */
  definitions: (DictionaryDefinition | string)[];
  synonyms?: string[];
  antonyms?: string[];
  translations?: DictionaryTranslation[];
};

/** What a dictionary says about a word. */
export type DictionaryEntry = {
  /** The headword. Required. */
  word: string;
  /** BCP 47; the language asked for when left out. */
  language?: string;
  /** The language's name (`French`). */
  languageName?: string | null;
  pronunciations?: DictionaryPronunciation[];
  /** At least one with a definition. */
  meanings: DictionaryMeaning[];
  /** Where the word comes from, plain text. */
  etymology?: string | null;
  /**
   * The word is a form of another (`went` → `go`). With `only` (the default), every sense is,
   * and the reader shows the lemma's entry instead.
   */
  formOf?: { lemma: string; note?: string; only?: boolean } | null;
  /** The full entry; http(s) only. */
  url?: string | null;
  /** Shown with the answer (`CC BY-SA 4.0`). */
  license?: { name: string; url?: string | null } | null;
};

/**
 * Words selected in the book reader, looked up. Raygun asks every dictionary at once (12 s
 * each) and merges their answers in the user's order, after its own (Wiktionary, the Free
 * Dictionary API). `language` is the words' language (BCP 47): answer `{ entries: [] }` for a
 * word or a language the dictionary doesn't know. A word not found as selected is asked again
 * in lower case and, in English, without its ending.
 */
export type DictionaryProvider = ProviderBase & {
  kind: 'dictionary';
  define(args: { word: string; language: string }): Promise<{ entries: DictionaryEntry[] }>;
};

export type Provider =
  | AudiobookDirectoryProvider
  | DictionaryProvider
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
  /** `GET` when left out. */
  method?: string;
  /**
   * Request headers. Raygun sends its own User-Agent unless one is given here, and the
   * extension's cookies unless a `Cookie` header is.
   */
  headers?: Record<string, string>;
  /** Text only, e.g. `JSON.stringify(...)` with a `Content-Type` header. */
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
  /** The body as UTF-8 text; binary bodies can't be read. */
  text(): Promise<string>;
  json<T = unknown>(): Promise<T>;
};

/** An element `raygun.html.select` found. */
export type HtmlElement = {
  /** All descendant text, concatenated. */
  text: string;
  /** Outer HTML. */
  html: string;
  /** Inner HTML. */
  inner: string;
  attrs: Record<string, string>;
  /** An attribute's value, or `null`. */
  attr(name: string): string | null;
  /** Selects inside this element (re-parsed as a fragment; select table rows from the document). */
  select(selector: string): HtmlElement[];
};

/** A picture of a YouTube video. */
export type YouTubeThumbnail = { url: string; width: number | null; height: number | null };

/** A video a YouTube search or playlist found. */
export type YouTubeVideo = {
  id: string;
  title: string;
  /** A song's artists, else the channel that uploaded it ("… - Topic" by the artist's name). */
  channel: string | null;
  album: string | null;
  durationMs: number | null;
  /** The largest picture. */
  thumbnailUrl: string | null;
  thumbnails: YouTubeThumbnail[];
  /** `song`: YouTube Music's own audio of a song. */
  kind: 'song' | 'video';
};

/** A YouTube video's audio stream, checked to play to its end. */
export type YouTubeStream = {
  url: string;
  /** Send these with it (the player and downloads do, as a `url` source's `headers`). */
  headers: Record<string, string>;
  /** Unix milliseconds after which the address stops working. */
  expiresAt: number | null;
  mimeType: string | null;
  /** `webm` or `m4a`. */
  container: string | null;
  /** `opus` or `mp4a.40.2`. */
  codec: string | null;
  bitrateKbps: number | null;
  contentLength: number | null;
  durationMs: number | null;
  title: string | null;
  channel: string | null;
  album: string | null;
  artists: string[];
  /** `YYYYMMDD`, when known. */
  uploadDate: string | null;
  /** What gave it: one of YouTube's app clients, or `yt-dlp`. */
  client: string;
};

/** A YouTube playlist and its videos, in order. */
export type YouTubePlaylist = {
  id: string;
  title: string;
  author: string | null;
  videos: YouTubeVideo[];
};

/**
 * The `raygun` global: everything an extension can reach. Extensions run in QuickJS, not a
 * browser or Node: there is no `fetch`, `URL`, `setTimeout`, `require` or DOM.
 */
export type RaygunApi = {
  apiVersion: 1;
  /** Goes to Raygun's log (and `raygun-ext test`'s output); `console` writes here too. */
  log: Record<'debug' | 'info' | 'warn' | 'error', (...args: unknown[]) => void>;
  http: {
    /**
     * An HTTP request. Only hosts in the manifest's `permissions.net` (redirects included, at
     * most 10); bodies up to 8 MiB; 20 s per request. Each extension keeps its own cookies across
     * restarts. Requests take turns: 6 at once per site, 32 in all. Rejects when the request
     * can't be made; an HTTP error status resolves, so check `ok`.
     */
    fetch(url: string, init?: FetchInit): Promise<FetchResponse>;
  };
  html: {
    /** Elements matching a CSS selector, in document order. Throws on an invalid selector. */
    select(html: string, selector: string): HtmlElement[];
  };
  time: {
    /** Waits without using CPU, e.g. to respect an API's rate limit. Capped at 10 s per call. */
    sleep(ms: number): Promise<void>;
  };
  /**
   * Per-extension storage of JSON values, kept across app restarts and extension updates; up to
   * 1 MiB in all. Clearing the extension's data empties it.
   */
  storage: {
    /** The stored value, or `null`. */
    get<T extends JsonValue = JsonValue>(key: string): T | null;
    /** Throws when the store would grow past 1 MiB. */
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
  /**
   * YouTube through its own web API, answered by Raygun (on desktop yt-dlp answers what it
   * can't). Requests go out as the extension's: its `permissions.net` must name
   * `www.youtube.com`, `music.youtube.com` and `*.googlevideo.com`. Converted Nuclear plugins
   * reach it as their `Ytdlp` API. Each rejects with a plain sentence when YouTube won't answer.
   */
  youtube: {
    /** Songs, then music videos, from YouTube Music (10 without a limit, at most 50). */
    search(query: string, limit?: number): Promise<YouTubeVideo[]>;
    /**
     * A video's audio (an id or a YouTube address): the format nearest `targetKbps` (the best
     * without one), AAC over Opus when `decodable`.
     */
    stream(
      video: string,
      options?: { targetKbps?: number | null; decodable?: boolean },
    ): Promise<YouTubeStream>;
    /** A playlist's videos (an id or an address with `list=`), every page of them. */
    playlist(playlist: string): Promise<YouTubePlaylist>;
  };
  providers: {
    /**
     * Registers a provider. Call it while the script first runs (at the top level): providers
     * registered later aren't seen. Throws for a kind missing from `provides`, an invalid id, or
     * an id registered twice.
     */
    register(provider: Provider): void;
  };
  /**
   * Says the extension can't work until someone sets it up: its key or sign-in is missing, or
   * its API refused it (an HTTP 401 or 403 from its own API). Throws; Raygun then asks the
   * extension nothing (no calls, no requests) until its settings change, and lists it with the
   * sources that need a key instead of the ones that failed. `reason` is shown with it. Prefer
   * the `needsSetup` helper, which also works in older Raygun versions.
   */
  needsSetup(reason?: string): never;
};

// ---- Manifest (raygun-extension.json) ----

/** A setting the user can change in the extension's page; read it with `raygun.settings.get`. */
export type SettingDef = {
  /** Unique, not empty. */
  key: string;
  type: 'text' | 'number' | 'boolean' | 'select';
  /** Must fit the type: a string, a number, a boolean, or one of a select's options. */
  default?: JsonValue;
  /** Defaults to the key in words: `itemsPerSearch` reads "Items per search". */
  label?: string;
  /** A select's choices; required for a select. */
  options?: string[];
  /**
   * The extension can't work without it: an API key, a secret, a server's address, a user name.
   * While it's empty (no value, or only spaces) Raygun asks the extension nothing and lists it
   * with the sources that need a key. Not for a `boolean`, which always has a value.
   */
  required?: boolean;
};

/** `raygun-extension.json`. Raygun refuses a manifest with fields it doesn't know. */
export type Manifest = {
  /** Reverse-DNS, lower case: `dev.example.my-source`. Never change it: updates match by id. */
  id: string;
  /** 1–80 characters. */
  name: string;
  /** Semver; a new version for every published change. */
  version: string;
  apiVersion: 1;
  /** `site` for site scripts, which run in the pages of Raygun's websites browser. */
  runtime: 'js' | 'site';
  /** The bundle `raygun-ext build` writes, normally `dist/index.js`; one of a site's scripts. */
  entry: string;
  description?: string;
  author?: string;
  /** Every provider kind the extension registers; `site.scripts` alone for site scripts. */
  provides: (ProviderKind | 'site.scripts')[];
  permissions?: {
    /**
     * Hosts `raygun.http.fetch` may reach: `api.example.org` (that host only), `*.example.org`
     * (its subdomains, not itself) or `*` (any public host, never this device or the local
     * network). Nothing else is reachable.
     */
    net?: string[];
    /** Reserved; no API uses it yet. Leave it out. */
    webview?: boolean;
  };
  /**
   * `nsfw` extensions count as 18+: they're hidden while NSFW content is off (music ones while
   * Explicit music is off). `suggestive` and `safe` are shown; `safe` when left out. Users can
   * mark any extension 18+ or not themselves.
   */
  contentRating?: 'safe' | 'suggestive' | 'nsfw';
  settings?: SettingDef[];
  /** The site the sources read (http or https); a source's menu opens it in Raygun's browser. */
  website?: string;
  /** Site scripts' groups (runtime `site`): docs/extensions/site-scripts.md. */
  site?: { scripts: SiteScriptGroup[] };
};

/** Scripts and styles for the pages some match patterns name (runtime `site`). */
export type SiteScriptGroup = {
  /** Match patterns like https://*.example.com/path, where a star in the path matches anything. */
  matches: string[];
  excludeMatches?: string[];
  /** Scripts, run in order; `dist/<name>.js` is built from `src/<name>.ts` when there. */
  js?: string[];
  /** Style sheets, added at the start of the page. */
  css?: string[];
  /** `document-idle` when left out. */
  runAt?: 'document-start' | 'document-end' | 'document-idle';
  /** Frames too; the top page only when left out. */
  allFrames?: boolean;
};
