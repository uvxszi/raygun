// Comics, books and videos from sources in the browser (docs/WEB.md): the catalogue commands the
// app answers with raygun-app's `catalog.rs`, `manga.rs`, `novel.rs` and `video_sources.rs`,
// answered here through the sandboxed sources (`manga.source`, `novel.source`, `video.source`).
// Titles opened or saved are kept in the `titles` table (device-local, as in the app), with their
// chapters or episodes and what was read or watched of them.
import type {
  ContinueWatching,
  FilterValue,
  LibraryList,
  MangaBrowseMode,
  MangaCard,
  MangaChapterInfo,
  MangaChapterPages,
  MangaLibraryEntry,
  MangaListingPage,
  MangaSource_Serialize,
  MangaTitleView,
  NovelChapter_Serialize,
  ReaderMode,
  Settings,
  SourceFilter,
  SourceSearchResult_Serialize,
  TitleMarks,
  VideoEpisodeView,
  VideoLinkView,
  VideoServerView,
  VideoSourcePlay,
  VideoSubtitle,
  VideoTitleView,
} from '@raygun/bindings';

import type { ExtensionsService } from '../extensions/service';
import { fail } from './context';
import type { CoreContext } from './context';
import { passageBlocks } from './passage';

export type Medium = 'manga' | 'novel' | 'video';

const KIND: Record<Medium, string> = {
  manga: 'manga.source',
  novel: 'novel.source',
  video: 'video.source',
};

export const mediumOf = (value: unknown): Medium =>
  value === 'novel' || value === 'video' ? value : 'manga';

const STATUSES = new Set(['ongoing', 'completed', 'hiatus', 'cancelled', 'unknown']);
const MEDIA_TYPES = new Set(['movie', 'series', 'anime', 'other']);

/** A chapter or episode as kept. */
export type StoredUnit = {
  id: string;
  remoteId: string;
  name: string;
  number: number | null;
  volume: string | null;
  season: number | null;
  scanlator: string | null;
  dateUpload: number | null;
  read: boolean;
  lastPage: number | null;
  totalPages: number | null;
  positionMs: number | null;
  durationMs: number | null;
};

/** A title as kept in the `titles` table. */
export type StoredTitle = {
  id: string;
  medium: Medium;
  sourceId: string;
  sourceName: string;
  remoteId: string;
  title: string;
  coverUrl: string | null;
  authors: string[];
  artists: string[];
  description: string | null;
  genres: string[];
  status: string;
  url: string | null;
  mediaType: string | null;
  year: number | null;
  adult: boolean;
  inLibrary: boolean;
  addedAt: number | null;
  list: LibraryList | null;
  favorite: boolean;
  pinned: boolean;
  lastReadAt: number | null;
  /** The chapter or episode read or watched last. */
  lastUnitId: string | null;
  refreshedAt: number | null;
  readerMode: ReaderMode | null;
  units: StoredUnit[];
};

/** 64 bits of FNV-1a over `text`, as hex: stable ids for titles and their chapters. */
export const stableId = (prefix: string, text: string) => {
  let high = 0xcbf29ce4;
  let low = 0x84222325;
  for (let index = 0; index < text.length; index += 1) {
    low ^= text.charCodeAt(index);
    // Multiply the 64-bit value by FNV's prime (0x100000001b3) in two halves.
    const lowProduct = low * 0x1b3;
    const carry = Math.floor(lowProduct / 0x100000000);
    high = (Math.imul(high, 0x1b3) + Math.imul(low, 0x100) + carry) >>> 0;
    low = lowProduct >>> 0;
  }
  return `${prefix}${high.toString(16).padStart(8, '0')}${low.toString(16).padStart(8, '0')}`;
};

export const titleIdOf = (medium: Medium, sourceId: string, remoteId: string) =>
  stableId('t', `${medium}\u0000${sourceId}\u0000${remoteId}`);

const unitIdOf = (titleId: string, remoteId: string) =>
  stableId('c', `${titleId}\u0000${remoteId}`);

const text = (value: unknown): string | null =>
  typeof value === 'string' && value.trim() ? value.trim() : null;

const idText = (value: unknown): string | null =>
  typeof value === 'number' && Number.isFinite(value) ? String(value) : text(value);

const texts = (value: unknown): string[] =>
  Array.isArray(value)
    ? value.map(text).filter((item): item is string => item !== null)
    : text(value)
      ? [String(value).trim()]
      : [];

const webUrl = (value: unknown): string | null => {
  const address = text(value);
  return address && /^https?:\/\//i.test(address) ? address : null;
};

const finite = (value: unknown): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null;

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

type Listed = { id: string; title: string; coverUrl: string | null; authors: string[] } & Pick<
  MangaCard,
  'live' | 'subtitle' | 'folder' | 'adult'
>;

/** A listing answer (`parse_listing`): items with an id and a title. */
export const listingOf = (answer: unknown): { items: Listed[]; hasNextPage: boolean } => {
  const value = (answer ?? {}) as Record<string, unknown>;
  const items = Array.isArray(value.items) ? value.items : [];
  return {
    items: items.flatMap((raw) => {
      const item = (raw ?? {}) as Record<string, unknown>;
      const id = idText(item.id);
      const title = text(item.title);
      if (!id || !title) return [];
      const live = item.live === true;
      return [
        {
          id,
          title,
          coverUrl: webUrl(item.coverUrl),
          authors: texts(item.authors).slice(0, 4),
          adult: item.nsfw === true,
          live: live ? { viewers: finite(item.viewers), startedAt: finite(item.startedAt) } : null,
          subtitle: text(item.subtitle)?.slice(0, 120) ?? null,
          folder: item.folder === true,
        },
      ];
    }),
    hasNextPage: value.hasNextPage === true,
  };
};

/** A source's chapters or episodes (`parse_units`); unnamed ones are named by their number. */
export const unitsOf = (answer: unknown, key: string, noun: string) => {
  const value = (answer ?? {}) as Record<string, unknown>;
  const list = Array.isArray(value[key]) ? (value[key] as unknown[]) : [];
  return list.flatMap((raw) => {
    const unit = (raw ?? {}) as Record<string, unknown>;
    const id = idText(unit.id);
    const number = finite(unit.number);
    const name = text(unit.name) ?? (number !== null ? `${noun} ${number}` : null);
    if (!id || !name) return [];
    const season = finite(unit.season);
    return [
      {
        remoteId: id,
        name,
        number,
        volume: idText(unit.volume) ?? (season !== null && season >= 0 ? String(season) : null),
        season: season !== null && season >= 0 ? season : null,
        scanlator: text(unit.scanlator),
        dateUpload: finite(unit.dateUpload ?? unit.date),
      },
    ];
  });
};

/** A filters answer (`parse_filters`): select, tags and toggles with their options. */
export const filtersOf = (answer: unknown): SourceFilter[] => {
  const value = (answer ?? {}) as Record<string, unknown>;
  const list = Array.isArray(value) ? value : Array.isArray(value.filters) ? value.filters : [];
  return (list as Array<Record<string, unknown>>).flatMap((filter): SourceFilter[] => {
    const key = text(filter?.key);
    if (!key) return [];
    const label = text(filter.label) ?? key;
    const options = (Array.isArray(filter.options) ? filter.options : []).flatMap((option) => {
      if (typeof option === 'string') return [{ value: option, label: option }];
      const raw = (option ?? {}) as Record<string, unknown>;
      const optionValue = idText(raw.value);
      return optionValue ? [{ value: optionValue, label: text(raw.label) ?? optionValue }] : [];
    });
    switch (filter.kind ?? filter.type) {
      case 'select':
        return options.length
          ? [{ kind: 'select', key, label, options, default: idText(filter.default) }]
          : [];
      case 'tags':
        return options.length ? [{ kind: 'tags', key, label, options }] : [];
      case 'toggle':
        return [{ kind: 'toggle', key, label, default: filter.default === true }];
      default:
        return [];
    }
  });
};

const videoOf = (raw: unknown, preferred: number | null): VideoLinkView | null => {
  const video = (raw ?? {}) as Record<string, unknown>;
  const format = text(video.format);
  if (format === 'torrent') return null;
  const url = webUrl(video.url);
  if (!url) return null;
  const quality = finite(video.quality);
  const subtitles: VideoSubtitle[] = (
    Array.isArray(video.subtitles) ? video.subtitles : []
  ).flatMap((track) => {
    const item = (track ?? {}) as Record<string, unknown>;
    const address = webUrl(item.url);
    return address
      ? [
          {
            url: address,
            lang: text(item.lang),
            label: text(item.label),
            format: text(item.format),
          },
        ]
      : [];
  });
  const headers =
    video.headers && typeof video.headers === 'object'
      ? (video.headers as Record<string, string>)
      : {};
  return {
    url,
    label: text(video.label) ?? (quality !== null ? `${quality}p` : null),
    quality,
    headers,
    format,
    subtitles,
    preferred: preferred !== null && quality === preferred,
    live: video.live === true,
  };
};

const PREFERRED_HEIGHT: Record<string, number> = {
  '2160p': 2160,
  '1440p': 1440,
  '1080p': 1080,
  '720p': 720,
  '480p': 480,
  '360p': 360,
};

/** Links ordered by the preferred resolution: that one, then lower, then higher. */
export const orderLinks = (links: VideoLinkView[], preferred: number | null) => {
  const rank = (link: VideoLinkView) => {
    if (link.format === 'page') return 3;
    if (preferred === null || link.quality === null) return 1;
    return link.quality <= preferred ? 0 : 2;
  };
  return [...links].sort(
    (a, b) =>
      rank(a) - rank(b) ||
      (preferred !== null && a.quality !== null && b.quality !== null
        ? Math.abs(preferred - a.quality) - Math.abs(preferred - b.quality)
        : 0),
  );
};

/** What the page's video player is told (the desktop player's `PlayerRequest`). */
export type PlayerOpen = (request: Record<string, unknown>) => void;

export class CatalogService {
  constructor(
    private readonly context: CoreContext,
    private readonly extensions: ExtensionsService,
    private readonly settings: () => Settings,
    private readonly openPlayer: PlayerOpen,
  ) {}

  // ---- Stored titles ----

  private stored(id: string): StoredTitle | undefined {
    return this.context.store.get<StoredTitle>('titles', id);
  }

  private save(title: StoredTitle) {
    this.context.store.put('titles', title.id, title);
  }

  private titleOrFail(id: string): StoredTitle {
    return this.stored(id) ?? fail('That title isnâ€™t on this browser any more. Open it again.');
  }

  private byRemote(medium: Medium, sourceId: string, remoteId: string) {
    return this.stored(titleIdOf(medium, sourceId, remoteId));
  }

  /** The title a chapter or episode (stored or source id) belongs to, and the unit. */
  private unitOf(unitId: string, titleId?: string): { title: StoredTitle; unit: StoredUnit } {
    const titles = titleId
      ? [this.titleOrFail(titleId)]
      : this.context.store.all<StoredTitle>('titles');
    for (const title of titles) {
      const unit = title.units.find((each) => each.id === unitId || each.remoteId === unitId);
      if (unit) return { title, unit };
    }
    return fail('That chapter isnâ€™t on this browser any more. Open its title again.');
  }

  private changed(medium: Medium) {
    void medium;
    this.context.emit('quickAccessChangedEvent', 'library');
  }

  // ---- Sources and listings ----

  async sources(medium: Medium): Promise<MangaSource_Serialize[]> {
    const listed = await this.extensions.listed(KIND[medium]);
    return listed.map(({ info, enabled }) => ({
      id: info.id,
      name: info.name,
      extensionId: info.extensionId,
      enabled,
      searchPill: info.searchPill ?? null,
      adult: info.adult,
    }));
  }

  private card(medium: Medium, sourceId: string, item: Listed): MangaCard {
    const kept = this.byRemote(medium, sourceId, item.id);
    return {
      sourceId,
      id: item.id,
      title: item.title,
      coverUrl: item.coverUrl ?? kept?.coverUrl ?? null,
      inLibrary: kept?.inLibrary ?? false,
      adult: item.adult,
      authors: item.authors.length ? item.authors : (kept?.authors ?? []),
      live: item.live,
      subtitle: item.subtitle,
      folder: item.folder,
    };
  }

  private page(medium: Medium, sourceId: string, answer: unknown): MangaListingPage {
    const showNsfw = this.settings().showNsfw;
    const listing = listingOf(answer);
    return {
      items: listing.items
        .filter((item) => showNsfw || !item.adult)
        .map((item) => this.card(medium, sourceId, item)),
      hasNextPage: listing.hasNextPage,
    };
  }

  async browse(
    medium: Medium,
    sourceId: string,
    mode: MangaBrowseMode,
    query: string | null,
    page: number,
  ): Promise<MangaListingPage> {
    const asked = (query ?? '').trim();
    const [method, args] =
      mode === 'search' && asked
        ? ['search', { query: asked, page }]
        : [mode === 'latest' ? 'latest' : 'popular', { page }];
    const answer = await this.extensions.call(sourceId, method, args).catch((error) => {
      // A source without a latest list shows its popular one, as the app does.
      if (method === 'latest' && /has no method/.test(message(error))) {
        return this.extensions.call(sourceId, 'popular', { page });
      }
      throw error;
    });
    return this.page(medium, sourceId, answer);
  }

  async search(
    medium: Medium,
    sourceId: string,
    query: string,
    page: number,
    filters: Record<string, FilterValue>,
  ): Promise<MangaListingPage> {
    const answer = await this.extensions.call(sourceId, 'search', {
      query: query.trim(),
      page,
      filters,
    });
    return this.page(medium, sourceId, answer);
  }

  async filters(sourceId: string): Promise<SourceFilter[]> {
    try {
      return filtersOf(await this.extensions.call(sourceId, 'filters', {}));
    } catch (error) {
      if (/has no method/.test(message(error))) return [];
      throw error;
    }
  }

  /** Every chosen source's first page (popular when nothing is typed), each answering alone. */
  async globalSearch(
    medium: Medium,
    query: string,
    only: string[],
  ): Promise<SourceSearchResult_Serialize[]> {
    const sources = (await this.sources(medium)).filter(
      (source) => source.enabled && (only.length === 0 || only.includes(source.id)),
    );
    const asked = query.trim();
    return Promise.all(
      sources.map(async (source) => {
        try {
          const answer = await this.extensions.call(
            source.id,
            asked ? 'search' : 'popular',
            asked ? { query: asked, page: 1 } : { page: 1 },
          );
          const page = this.page(medium, source.id, answer);
          return { source, ...page, error: null, fromListings: false };
        } catch (error) {
          return {
            source,
            items: [],
            hasNextPage: false,
            error: message(error),
            fromListings: false,
          };
        }
      }),
    );
  }

  // ---- Titles ----

  /** A title fresh from its source (details and chapters), merged into what was kept. */
  private async fetchTitle(medium: Medium, sourceId: string, remoteId: string) {
    const [details, units] = await Promise.all([
      this.extensions.call<Record<string, unknown>>(sourceId, 'details', { id: remoteId }),
      this.extensions.call(sourceId, medium === 'video' ? 'episodes' : 'chapters', {
        id: remoteId,
      }),
    ]);
    const name = text(details?.title) ?? fail(`${this.extensions.nameOf(sourceId)} sent no title`);
    const id = titleIdOf(medium, sourceId, remoteId);
    const before = this.stored(id);
    const kept = new Map(before?.units.map((unit) => [unit.remoteId, unit]));
    const parsed = unitsOf(
      units,
      medium === 'video' ? 'episodes' : 'chapters',
      medium === 'video' ? 'Episode' : 'Chapter',
    );
    const status = text(details.status);
    const mediaType = text(details.type);
    const year = finite(details.year);
    const title: StoredTitle = {
      id,
      medium,
      sourceId,
      sourceName: this.extensions.nameOf(sourceId),
      remoteId,
      title: name,
      coverUrl: webUrl(details.coverUrl) ?? before?.coverUrl ?? null,
      authors: texts(details.authors).length ? texts(details.authors) : texts(details.cast),
      artists: texts(details.artists).length ? texts(details.artists) : texts(details.directors),
      description: text(details.description),
      genres: texts(details.genres),
      status: status && STATUSES.has(status) ? status : 'unknown',
      url: webUrl(details.url),
      mediaType: mediaType && MEDIA_TYPES.has(mediaType) ? mediaType : null,
      year: year !== null && year >= 1800 && year <= 2200 ? Math.round(year) : null,
      adult: details.nsfw === true,
      inLibrary: before?.inLibrary ?? false,
      addedAt: before?.addedAt ?? null,
      list: before?.list ?? null,
      favorite: before?.favorite ?? false,
      pinned: before?.pinned ?? false,
      lastReadAt: before?.lastReadAt ?? null,
      lastUnitId: before?.lastUnitId ?? null,
      refreshedAt: this.context.now(),
      readerMode: before?.readerMode ?? null,
      units: parsed.map((unit) => {
        const old = kept.get(unit.remoteId);
        return {
          ...unit,
          id: unitIdOf(id, unit.remoteId),
          read: old?.read ?? false,
          lastPage: old?.lastPage ?? null,
          totalPages: old?.totalPages ?? null,
          positionMs: old?.positionMs ?? null,
          durationMs: old?.durationMs ?? null,
        };
      }),
    };
    this.save(title);
    return title;
  }

  private async title(medium: Medium, sourceId: string, remoteId: string, refresh: boolean) {
    const kept = this.byRemote(medium, sourceId, remoteId);
    if (kept && !refresh && kept.units.length > 0) return kept;
    try {
      return await this.fetchTitle(medium, sourceId, remoteId);
    } catch (error) {
      if (kept) return kept;
      throw error;
    }
  }

  private chapterInfo(unit: StoredUnit): MangaChapterInfo {
    return {
      id: unit.id,
      name: unit.name,
      number: unit.number,
      volume: unit.volume,
      scanlator: unit.scanlator,
      dateUpload: unit.dateUpload,
      read: unit.read,
      lastPage: unit.lastPage,
      totalPages: unit.totalPages,
      download: null,
    };
  }

  /** The chapter to go on with: the one read last unless finished, else the next unread. */
  continueOf(title: StoredTitle): string | null {
    const units = title.units;
    if (units.length === 0) return null;
    const last = units.findIndex((unit) => unit.id === title.lastUnitId);
    if (last >= 0 && !units[last]?.read) return units[last]?.id ?? null;
    // Sources list newest first or oldest first; reading goes by number when there is one.
    const ordered = [...units].sort(
      (a, b) => (a.number ?? Number.MAX_SAFE_INTEGER) - (b.number ?? Number.MAX_SAFE_INTEGER),
    );
    return (ordered.find((unit) => !unit.read) ?? ordered[0])?.id ?? null;
  }

  private titleView(title: StoredTitle): MangaTitleView {
    const hidden = title.adult && !this.settings().showNsfw;
    return {
      id: title.id,
      sourceId: title.sourceId,
      sourceName: title.sourceName,
      remoteId: title.remoteId,
      title: title.title,
      coverUrl: title.coverUrl,
      authors: title.authors,
      artists: title.artists,
      description: title.description,
      genres: title.genres,
      status: title.status,
      url: title.url,
      inLibrary: title.inLibrary,
      hiddenNsfw: hidden,
      adult: title.adult,
      chapters: title.units.map((unit) => this.chapterInfo(unit)),
      continueChapterId: this.continueOf(title),
    };
  }

  async mangaTitle(medium: Medium, sourceId: string, remoteId: string, refresh: boolean) {
    return this.titleView(await this.title(medium, sourceId, remoteId, refresh));
  }

  async videoTitle(sourceId: string, remoteId: string, refresh: boolean): Promise<VideoTitleView> {
    const title = await this.title('video', sourceId, remoteId, refresh);
    const episodes: VideoEpisodeView[] = title.units.map((unit) => ({
      id: unit.id,
      remoteId: unit.remoteId,
      name: unit.name,
      number: unit.number,
      season: unit.season,
      date: unit.dateUpload,
      positionMs: unit.positionMs,
      durationMs: unit.durationMs,
      watched: unit.read,
      downloaded: false,
    }));
    return {
      id: title.id,
      sourceId: title.sourceId,
      sourceName: title.sourceName,
      remoteId: title.remoteId,
      title: title.title,
      coverUrl: title.coverUrl,
      mediaType: title.mediaType,
      year: title.year,
      description: title.description,
      genres: title.genres,
      cast: title.authors,
      directors: title.artists,
      status: title.status,
      url: title.url,
      inLibrary: title.inLibrary,
      hiddenNsfw: title.adult && !this.settings().showNsfw,
      adult: title.adult,
      episodes,
      continueEpisodeId: this.continueOf(title),
    };
  }

  chapters(titleId: string): MangaChapterInfo[] {
    return this.titleOrFail(titleId).units.map((unit) => this.chapterInfo(unit));
  }

  private neighbours(title: StoredTitle, unit: StoredUnit) {
    // Previous and next go by reading order (numbers when the source gives them).
    const ordered = [...title.units].sort(
      (a, b) => (a.number ?? Number.MAX_SAFE_INTEGER) - (b.number ?? Number.MAX_SAFE_INTEGER),
    );
    const index = ordered.findIndex((each) => each.id === unit.id);
    return {
      previousChapterId: index > 0 ? (ordered[index - 1]?.id ?? null) : null,
      nextChapterId: ordered[index + 1]?.id ?? null,
    };
  }

  private touch(title: StoredTitle, unit: StoredUnit) {
    title.lastUnitId = unit.id;
    title.lastReadAt = this.context.now();
    this.save(title);
  }

  async chapterPages(titleId: string, chapterId: string): Promise<MangaChapterPages> {
    const { title, unit } = this.unitOf(chapterId, titleId);
    const answer = await this.extensions.call<Record<string, unknown>>(title.sourceId, 'pages', {
      chapterId: unit.remoteId,
    });
    if (answer?.pdf) {
      fail('This chapter is a PDF document, which only the Raygun app can show.');
    }
    const pages = (Array.isArray(answer?.pages) ? answer.pages : []).flatMap((page) => {
      const address =
        typeof page === 'string' ? webUrl(page) : webUrl((page as { url?: unknown })?.url);
      return address ? [address] : [];
    });
    if (pages.length === 0) fail(`${title.sourceName} has no pages for this chapter`);
    this.touch(title, unit);
    const mode = this.settings().mangaReader.defaultMode as ReaderMode;
    return {
      titleId: title.id,
      title: title.title,
      readerMode: title.readerMode ?? mode,
      ownReaderMode: title.readerMode,
      chapter: this.chapterInfo(unit),
      pages,
      ...this.neighbours(title, unit),
    };
  }

  async novelChapter(titleId: string, chapterId: string): Promise<NovelChapter_Serialize> {
    const { title, unit } = this.unitOf(chapterId, titleId);
    const answer = await this.extensions.call(title.sourceId, 'passage', {
      chapterId: unit.remoteId,
    });
    const blocks = passageBlocks(answer);
    if (blocks.length === 0) fail(`${title.sourceName} has no text for this chapter`);
    this.touch(title, unit);
    return {
      titleId: title.id,
      title: title.title,
      chapter: this.chapterInfo(unit),
      blocks,
      ...this.neighbours(title, unit),
      pdf: null,
    };
  }

  setReaderMode(titleId: string, mode: ReaderMode | null) {
    const title = this.titleOrFail(titleId);
    title.readerMode = mode;
    this.save(title);
  }

  setProgress(chapterId: string, page: number, total: number) {
    const { title, unit } = this.unitOf(chapterId);
    unit.lastPage = Math.max(0, Math.round(page));
    unit.totalPages = Math.max(0, Math.round(total));
    if (total > 0 && page >= total - 1) unit.read = true;
    this.touch(title, unit);
  }

  setRead(chapterIds: string[], read: boolean) {
    for (const chapterId of chapterIds) {
      const { title, unit } = this.unitOf(chapterId);
      unit.read = read;
      if (!read) unit.lastPage = null;
      if (title.medium === 'video' && !read) unit.positionMs = null;
      this.save(title);
    }
  }

  markTitlesRead(titleIds: string[], read: boolean) {
    for (const id of titleIds) {
      const title = this.stored(id);
      if (!title) continue;
      for (const unit of title.units) unit.read = read;
      this.save(title);
    }
  }

  markPreviousRead(chapterId: string) {
    const { title, unit } = this.unitOf(chapterId);
    const before = unit.number;
    for (const each of title.units) {
      if (before !== null && each.number !== null && each.number < before) each.read = true;
    }
    this.save(title);
  }

  // ---- The library ----

  setInLibrary(titleId: string, inLibrary: boolean) {
    const title = this.titleOrFail(titleId);
    title.inLibrary = inLibrary;
    title.addedAt = inLibrary ? (title.addedAt ?? this.context.now()) : null;
    this.save(title);
    this.changed(title.medium);
  }

  library(medium: Medium): MangaLibraryEntry[] {
    const showNsfw = this.settings().showNsfw;
    return this.context.store
      .all<StoredTitle>('titles')
      .filter((title) => title.medium === medium && title.inLibrary)
      .filter((title) => showNsfw || !title.adult)
      .map((title) => ({
        titleId: title.id,
        sourceId: title.sourceId,
        remoteId: title.remoteId,
        title: title.title,
        coverUrl: title.coverUrl,
        chapters: title.units.length,
        unread: title.units.filter((unit) => !unit.read).length,
        genres: title.genres,
        status: title.status,
        addedAt: title.addedAt,
        lastReadAt: title.lastReadAt,
        latestChapterAt: title.units.reduce<number | null>(
          (latest, unit) =>
            unit.dateUpload !== null && (latest === null || unit.dateUpload > latest)
              ? unit.dateUpload
              : latest,
          null,
        ),
        started: title.units.some((unit) => unit.read || unit.lastPage !== null),
        downloaded: 0,
        categoryIds: [],
        list: title.list,
        favorite: title.favorite,
        pinned: title.pinned,
        authors: title.authors,
        artists: title.artists,
        sourceName: title.sourceName,
        refreshedAt: title.refreshedAt,
        latestFetchedAt: title.refreshedAt,
        local: false,
      }));
  }

  marks(titleId: string): TitleMarks {
    const title = this.stored(titleId);
    return {
      inLibrary: title?.inLibrary ?? false,
      list: title?.list ?? null,
      favorite: title?.favorite ?? false,
      pinned: title?.pinned ?? false,
    };
  }

  setMark(titleIds: string[], mark: Partial<Pick<StoredTitle, 'list' | 'favorite' | 'pinned'>>) {
    let medium: Medium | null = null;
    for (const id of titleIds) {
      const title = this.stored(id);
      if (!title) continue;
      medium = title.medium;
      // Marking a title saves it, as in the app.
      if (!title.inLibrary) {
        title.inLibrary = true;
        title.addedAt = this.context.now();
      }
      this.save({ ...title, ...mark });
    }
    if (medium) this.changed(medium);
  }

  remove(titleIds: string[]) {
    let medium: Medium | null = null;
    for (const id of titleIds) {
      const title = this.stored(id);
      if (!title) continue;
      medium = title.medium;
      this.save({ ...title, inLibrary: false, addedAt: null, favorite: false, pinned: false });
    }
    if (medium) this.changed(medium);
  }

  /** Library titles pinned to quick access. */
  pinned(): StoredTitle[] {
    return this.context.store
      .all<StoredTitle>('titles')
      .filter((title) => title.inLibrary && title.pinned);
  }

  continueChapter(titleId: string): string | null {
    const title = this.stored(titleId);
    return title ? this.continueOf(title) : null;
  }

  /** Titles kept in the library, matching words in their names (Library â†’ search). */
  searchLibrary(medium: Medium, query: string): MangaCard[] {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return this.library(medium)
      .filter((entry) => words.every((word) => entry.title.toLowerCase().includes(word)))
      .map((entry) => ({
        sourceId: entry.sourceId,
        id: entry.remoteId,
        title: entry.title,
        coverUrl: entry.coverUrl,
        inLibrary: true,
        authors: entry.authors,
      }));
  }

  // ---- Videos ----

  private preferredHeight(): number | null {
    const prefs = this.settings().videoPlayer as { preferredResolution?: string };
    return PREFERRED_HEIGHT[prefs.preferredResolution ?? ''] ?? null;
  }

  /** An episode's servers, each with its links when the source gave them at once. */
  async servers(sourceId: string, episodeId: string): Promise<VideoServerView[]> {
    let remoteId = episodeId;
    try {
      remoteId = this.unitOf(episodeId).unit.remoteId;
    } catch {
      // A source's own id.
    }
    const answer = await this.extensions.call<Record<string, unknown>>(sourceId, 'servers', {
      episodeId: remoteId,
    });
    const preferred = this.preferredHeight();
    const servers = Array.isArray(answer?.servers) ? answer.servers : [];
    return servers.flatMap((raw) => {
      const server = (raw ?? {}) as Record<string, unknown>;
      const id = idText(server.id);
      if (!id) return [];
      const videos = Array.isArray(server.videos)
        ? orderLinks(
            server.videos
              .map((video) => videoOf(video, preferred))
              .filter((link): link is VideoLinkView => link !== null),
            preferred,
          )
        : null;
      return [{ id, name: text(server.name) ?? id, links: videos }];
    });
  }

  async links(sourceId: string, serverId: string): Promise<VideoLinkView[]> {
    const answer = await this.extensions.call<Record<string, unknown>>(sourceId, 'videos', {
      serverId,
    });
    const preferred = this.preferredHeight();
    return orderLinks(
      (Array.isArray(answer?.videos) ? answer.videos : [])
        .map((video) => videoOf(video, preferred))
        .filter((link): link is VideoLinkView => link !== null),
      preferred,
    );
  }

  /** Plays one link in the page's player (or the site's page in a new tab). */
  play(request: VideoSourcePlay) {
    const { title, unit } = this.unitOf(request.episodeId, request.titleId);
    const link = request.link;
    if (link.format === 'page') {
      window.open(link.url, '_blank', 'noopener');
      return;
    }
    this.touch(title, unit);
    const label = title.units.length > 1 || title.mediaType === 'series' ? unit.name : null;
    this.openPlayer({
      id: unit.id,
      url: link.url,
      title: title.title,
      subtitle: label,
      format: link.format,
      subtitles: link.subtitles,
      startMs: link.live || unit.read ? 0 : (unit.positionMs ?? 0),
      group: `title:${title.id}`,
      prefs: this.settings().videoPlayer,
      seekStepMs: this.settings().seekSeconds * 1000,
      autoplay: true,
      live: link.live === true,
      session: false,
    });
  }

  /** Plays an episode (the one to go on with when none is named) from its best link. */
  async watch(titleId: string, episodeId: string | null) {
    const title = this.titleOrFail(titleId);
    const chosen = episodeId ?? this.continueOf(title);
    if (!chosen) fail('This title has no episodes.');
    const { unit } = this.unitOf(String(chosen), titleId);
    const problems: string[] = [];
    for (const server of await this.servers(title.sourceId, unit.remoteId)) {
      try {
        const links = server.links ?? (await this.links(title.sourceId, server.id));
        const link = links[0];
        if (link) {
          this.play({ titleId, episodeId: unit.id, link });
          return;
        }
      } catch (error) {
        problems.push(message(error));
      }
    }
    fail(problems[0] ?? `${title.sourceName} has no video for this episode`);
  }

  /** A report from the page's player: where it is, kept for Continue and the title page. */
  report(raw: string) {
    let report: Record<string, unknown>;
    try {
      report = JSON.parse(raw) as Record<string, unknown>;
    } catch {
      return;
    }
    const id = typeof report.id === 'string' ? report.id : '';
    const position = finite(report.positionMs);
    const duration = finite(report.durationMs);
    if (!id || report.state === 'request' || report.live === true || position === null) return;
    for (const title of this.context.store.all<StoredTitle>('titles')) {
      if (title.medium !== 'video') continue;
      const unit = title.units.find((each) => each.id === id);
      if (!unit) continue;
      unit.positionMs = Math.round(position);
      if (duration !== null && duration > 0) unit.durationMs = Math.round(duration);
      const percent = this.settings().watchedPercent;
      if (duration && position / duration >= percent / 100) unit.read = true;
      title.lastUnitId = unit.id;
      title.lastReadAt = this.context.now();
      this.save(title);
      return;
    }
  }

  /** Videos partly watched (or all watched, for history), latest first. */
  continueWatching(history = false): ContinueWatching[] {
    return this.context.store
      .all<StoredTitle>('titles')
      .filter((title) => title.medium === 'video')
      .flatMap((title) =>
        title.units
          .filter((unit) => unit.positionMs !== null && (history || !unit.read))
          .map((unit) => ({ title, unit })),
      )
      .sort((a, b) => (b.title.lastReadAt ?? 0) - (a.title.lastReadAt ?? 0))
      .map(({ title, unit }) => {
        const series = title.units.length > 1 || title.mediaType === 'series';
        return {
          key: unit.id,
          title: title.title,
          subtitle: series ? unit.name : null,
          poster: title.coverUrl,
          positionMs: unit.positionMs,
          durationMs: unit.durationMs,
          updatedAt: title.lastReadAt,
          watched: unit.read,
          sourceId: title.sourceId,
          remoteId: title.remoteId,
          group: `title:${title.id}`,
          episodeId: unit.remoteId,
          unitId: unit.id,
          series,
        };
      });
  }

  /** Plays a video kept in Continue watching, from where it was or from its start. */
  async resume(key: string, fromStart: boolean) {
    const { title, unit } = this.unitOf(key);
    if (fromStart) {
      unit.positionMs = 0;
      this.save(title);
    }
    await this.watch(title.id, unit.id);
  }

  /** Forgets where videos got to (Continue watching's Remove). */
  forgetProgress(keys: string[]) {
    for (const key of keys) {
      try {
        const { title, unit } = this.unitOf(key);
        unit.positionMs = null;
        this.save(title);
      } catch {
        // Already gone.
      }
    }
    return keys.length;
  }

  setWatched(titleId: string, episodeId: string, watched: boolean) {
    const { title, unit } = this.unitOf(episodeId, titleId);
    unit.read = watched;
    if (!watched) unit.positionMs = null;
    this.save(title);
  }
}
