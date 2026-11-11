import type {
  AudiobookResult,
  LibraryTrack,
  MangaCard,
  MangaSource,
  MusicAlbum,
  MusicArtist,
  MusicPlaylist,
  OnlineTrack,
  PodcastSearchResult,
  ProviderInfo,
  ProviderSearchResult,
  SourceSearchResult,
  StremioCatalogRow,
} from '@raygun/bindings';
import { RouterProvider, createMemoryHistory } from '@tanstack/react-router';
import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { Profiler } from 'react';
import type { ProfilerOnRenderCallback } from 'react';

import { createAppRouter } from '../router';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from './bindingsMock';
import { libraryEntry } from './libraryEntry';
import { renderWithProviders } from './renderWithProviders';

vi.mock('@raygun/bindings', async () => (await import('./bindingsMock')).bindingsMock);

/**
 * Timings of Browse → Search's switches once every kind has answered: its kinds (All, Audio,
 * Videos, Comics, Books, Your library), Audio's pills, Best matches and By source, Custom sources
 * opened and closed, with dozens of sources each answering a page. Opt-in, as it's slow:
 * `RAYGUN_PERF=1 pnpm --filter @raygun/app exec vitest run src/test/searchPerf` (with
 * `RAYGUN_PERF_OUT=<file>` the timings go to that file). For each switch: when the pill pressed
 * showed as chosen (`pill`), when the new pill's first results were on the screen (`rows`), when
 * the last drawing it led to was done (`settled`), and the time React spent drawing (`render`),
 * each the best of a few runs. jsdom and React's development build are slower than a phone's
 * webview; compare runs, not absolute numbers.
 */

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const COMIC_SOURCES = 40;
const BOOK_SOURCES = 30;
const VIDEO_SOURCES = 24;
/** Titles each comic, book or video source answers with. */
const TITLES = 20;
const MUSIC_SOURCES = 20;
/** Songs each music source answers with. */
const SONGS = 25;
const ARTISTS = 120;
const ALBUMS = 150;
const PLAYLISTS = 80;
const PODCASTS = 60;
const BOOKS = 40;
const ADDON_ITEMS = 20;
const LIBRARY_TITLES = 300;
const RUNS = 5;
/** No drawing for this long: what a switch led to is done. */
const QUIET_MS = 60;

const WORDS = ['night', 'river', 'stars', 'city', 'storm', 'garden', 'moon', 'empire', 'road'];
const word = (index: number) => WORDS[index % WORDS.length] ?? 'night';

const catalogSource = (medium: string, index: number, pill?: string): MangaSource => ({
  id: `dev.${medium}${index}/${medium}`,
  name: `${medium} source ${index}`,
  extensionId: `dev.${medium}${index}`,
  enabled: true,
  adult: false,
  ...(pill ? { searchPill: pill } : {}),
});

const SOURCES = {
  manga: Array.from({ length: COMIC_SOURCES }, (_, index) => catalogSource('manga', index)),
  novel: Array.from({ length: BOOK_SOURCES }, (_, index) => catalogSource('novel', index)),
  video: [
    ...Array.from({ length: VIDEO_SOURCES }, (_, index) => catalogSource('video', index)),
    catalogSource('video', VIDEO_SOURCES, 'Open Videos'),
    catalogSource('video', VIDEO_SOURCES + 1, 'Tube'),
  ],
};
const ALL_SOURCES = [...SOURCES.manga, ...SOURCES.novel, ...SOURCES.video];

const card = (source: MangaSource, index: number, sourceIndex: number): MangaCard => ({
  sourceId: source.id,
  id: `${source.id}-${index}`,
  title: `Dark ${word(index + sourceIndex)} ${(index * 3 + sourceIndex) % 50}`,
  coverUrl: `http://127.0.0.1:9/cover/${source.id}-${index}`,
  inLibrary: index % 9 === 0,
});

/** Each source's page: a few fail, as some always do. */
const answerOf = (id: string): SourceSearchResult | null => {
  const sourceIndex = ALL_SOURCES.findIndex((source) => source.id === id);
  const source = ALL_SOURCES[sourceIndex];
  if (!source) return null;
  const failing = sourceIndex % 13 === 5;
  return {
    source,
    items: failing
      ? []
      : Array.from({ length: TITLES }, (_, index) => card(source, index, sourceIndex)),
    hasNextPage: !failing,
    error: failing ? 'The site took too long to answer.' : null,
    fromListings: false,
  };
};

const provider = (index: number): ProviderInfo => ({
  id: `dev.music${index}/music`,
  extensionId: `dev.music${index}`,
  kind: 'music.metadata',
  name: `Music source ${index}`,
  adult: false,
});

/** A song several sources share now and then, as they do. */
const song = (number: number): OnlineTrack => ({
  externalId: `song-${number}`,
  title: `Dark ${word(number)} ${number}`,
  artist: `Artist ${number % 60}`,
  album: `Album ${number % 40}`,
  durationMs: 180_000 + number * 1000,
  artworkUrl: `http://127.0.0.1:9/cover/song-${number}`,
  source: null,
});

const PROVIDERS: ProviderSearchResult[] = Array.from({ length: MUSIC_SOURCES }, (_, index) => ({
  provider: provider(index),
  tracks:
    index % 9 === 4 ? [] : Array.from({ length: SONGS }, (_, at) => song((index * 7 + at) % 300)),
  error: index % 9 === 4 ? 'The site took too long to answer.' : null,
  fromListings: false,
}));

const libraryTrack = (index: number): LibraryTrack => ({
  id: `track-${index}`,
  title: `Dark ${word(index)} (live) ${index}`,
  artist: `Artist ${index}`,
  albumArtist: `Artist ${index}`,
  album: `Live ${index}`,
  trackNo: index + 1,
  discNo: 1,
  year: 2001,
  durationMs: 200_000,
  artworkUrl: null,
});

const ARTIST_LIST: MusicArtist[] = Array.from({ length: ARTISTS }, (_, index) => ({
  provider: provider(index % 10),
  id: `artist-${index}`,
  name: `Dark ${word(index)} band ${index}`,
  imageUrl: `http://127.0.0.1:9/cover/artist-${index}`,
}));

const ALBUM_LIST: MusicAlbum[] = Array.from({ length: ALBUMS }, (_, index) => ({
  provider: provider(index % 10),
  id: `album-${index}`,
  title: `Dark ${word(index)} album ${index}`,
  artist: `Artist ${index % 60}`,
  year: 1990 + (index % 30),
  artworkUrl: `http://127.0.0.1:9/cover/album-${index}`,
  trackCount: 12,
}));

const PLAYLIST_LIST: MusicPlaylist[] = Array.from({ length: PLAYLISTS }, (_, index) => ({
  provider: provider(index % 10),
  id: `playlist-${index}`,
  title: `Dark ${word(index)} mix ${index}`,
  author: 'Someone',
  artworkUrl: `http://127.0.0.1:9/cover/playlist-${index}`,
  trackCount: 30,
  featured: index % 5 === 0,
}));

const PODCAST_LIST: PodcastSearchResult[] = Array.from({ length: PODCASTS }, (_, index) => ({
  title: `Dark ${word(index)} talk ${index}`,
  author: `Host ${index}`,
  feedUrl: `https://podcast${index}.example/feed.xml`,
  artworkUrl: `http://127.0.0.1:9/cover/podcast-${index}`,
  description: null,
  subscribed: false,
}));

const BOOK_LIST: AudiobookResult[] = Array.from({ length: BOOKS }, (_, index) => ({
  title: `Dark ${word(index)} tale ${index}`,
  author: `Writer ${index}`,
  edition: null,
  feedUrl: `https://books.example/${index}.xml`,
  artworkUrl: `http://127.0.0.1:9/cover/book-${index}`,
  runtime: null,
  language: 'en',
  description: null,
  source: 'Books source',
}));

const ADDON_CATALOGS = ['movie', 'series'].flatMap((kind) =>
  ['cinema', 'archive'].map((addon) => ({
    addonId: addon,
    addonName: `Addon ${addon}`,
    kind,
    catalogId: 'top',
    name: 'Popular',
    extras: [],
    board: true,
    searchable: true,
    discoverable: true,
    pages: true,
    failing: false,
  })),
);

const addonRow = (addonId: string, kind: string): StremioCatalogRow => ({
  addonId,
  addonName: `Addon ${addonId}`,
  kind,
  catalogId: 'top',
  name: 'Popular',
  items: Array.from({ length: ADDON_ITEMS }, (_, index) => ({
    kind,
    id: `tt-${addonId}-${kind}-${index}`,
    name: `Dark ${word(index)} ${kind} ${index}`,
    poster: `http://127.0.0.1:9/cover/${addonId}-${kind}-${index}`,
    posterShape: 'poster',
    releaseInfo: '2020',
    rating: null,
  })),
  genres: [],
  hasMore: false,
  error: null,
});

/** The test runner's process (Node's), without Node's types in the app's build. */
type RunnerProcess = {
  env: Record<string, string | undefined>;
  cpuUsage: () => { user: number; system: number };
};
const PROCESS = (globalThis as unknown as { process?: RunnerProcess }).process;
const ENV = PROCESS?.env ?? {};
const cpuMs = () => {
  const usage = PROCESS?.cpuUsage();
  return usage ? (usage.user + usage.system) / 1000 : 0;
};

/** What the switch being timed waits for: its pill shown as chosen, then its first results. */
type Watch = {
  pill: () => boolean;
  rows: () => boolean;
  pillAt: number | null;
  rowsAt: number | null;
};
let watch: Watch | null = null;
let lastCommit = 0;
let renderMs = 0;

/** Every commit of the app: the screen as React left it, before the browser draws it. */
const onRender: ProfilerOnRenderCallback = (_id, _phase, actualDuration) => {
  const now = performance.now();
  lastCommit = now;
  renderMs += actualDuration;
  if (!watch) return;
  if (watch.pillAt === null && watch.pill()) watch.pillAt = now;
  if (watch.rowsAt === null && watch.rows()) watch.rowsAt = now;
};

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Until nothing has been drawn for {@link QUIET_MS}. */
const settle = async () => {
  for (;;) {
    const before = lastCommit;
    await pause(QUIET_MS);
    if (lastCommit === before) return;
  }
};

type Switch = { pill: number; rows: number; settled: number; render: number; cpu: number };

const tenths = (ms: number) => Math.round(ms * 10) / 10;

/**
 * One switch: `target` finds the pill to press (before the clock starts: finding it isn't the
 * app's time); `pill` and `rows` say when each part shows.
 */
const timeSwitch = async (
  target: () => HTMLElement,
  pill: () => boolean,
  rows: () => boolean,
): Promise<Switch> => {
  await settle();
  const button = target();
  renderMs = 0;
  const started = performance.now();
  const cpu = cpuMs();
  const current: Watch = { pill, rows, pillAt: null, rowsAt: null };
  watch = current;
  fireEvent.click(button);
  await waitFor(() => expect(current.rowsAt).not.toBeNull(), { timeout: 120_000, interval: 5 });
  await settle();
  watch = null;
  return {
    pill: tenths((current.pillAt ?? started) - started),
    rows: tenths((current.rowsAt ?? started) - started),
    settled: tenths(lastCommit - started),
    render: tenths(renderMs),
    // The quiet wait at the end is no work.
    cpu: Math.round(cpuMs() - cpu),
  };
};

const bestOf = (runs: Switch[]): Switch =>
  runs.reduce(
    (best, run) => ({
      pill: Math.min(best.pill, run.pill),
      rows: Math.min(best.rows, run.rows),
      settled: Math.min(best.settled, run.settled),
      render: Math.min(best.render, run.render),
      cpu: Math.min(best.cpu, run.cpu),
    }),
    { pill: Infinity, rows: Infinity, settled: Infinity, render: Infinity, cpu: Infinity },
  );

const shown = (timing: Switch) =>
  `pill ${timing.pill} ms, rows ${timing.rows} ms, settled ${timing.settled} ms ` +
  `(render ${timing.render} ms, CPU ${timing.cpu} ms)`;

const note = async (line: string) => {
  // The timings go to a file when one is named (vitest may hold back a test's console).
  const out = ENV.RAYGUN_PERF_OUT;
  if (!out) {
    console.log(`[perf] ${line}`);
    return;
  }
  const fs = (await import(/* @vite-ignore */ 'node:fs' as string)) as {
    appendFileSync: (path: string, text: string) => void;
  };
  fs.appendFileSync(out, `${line}\n`);
};

type ProfilerSession = {
  connect: () => void;
  disconnect: () => void;
  post: (method: string, params?: object) => Promise<unknown>;
};

/**
 * With `RAYGUN_PERF_PROFILE=<folder>`, a CPU profile of the timed switches (Node's inspector),
 * saved there as `<name>.cpuprofile` by the function returned; without it, that does nothing.
 */
const profile = async (name: string): Promise<() => Promise<void>> => {
  const dir = ENV.RAYGUN_PERF_PROFILE;
  if (!dir) return async () => undefined;
  const { Session } = (await import(/* @vite-ignore */ 'node:inspector/promises' as string)) as {
    Session: new () => ProfilerSession;
  };
  const session = new Session();
  session.connect();
  await session.post('Profiler.enable');
  await session.post('Profiler.setSamplingInterval', { interval: 200 });
  await session.post('Profiler.start');
  return async () => {
    const { profile: taken } = (await session.post('Profiler.stop')) as { profile: unknown };
    session.disconnect();
    const fs = (await import(/* @vite-ignore */ 'node:fs' as string)) as {
      writeFileSync: (path: string, text: string) => void;
    };
    fs.writeFileSync(`${dir}/${name}.cpuprofile`, JSON.stringify(taken));
  };
};

const query = (selector: string) => document.querySelector(selector) !== null;
const count = (selector: string) => document.querySelectorAll(selector).length;

/** Whether the pill named `name` among `selector`'s shows as chosen (`attribute` true). */
const chosen = (selector: string, attribute: string, name: string) => () =>
  [...document.querySelectorAll(`${selector}[${attribute}="true"]`)].some(
    (element) => element.textContent?.trim().startsWith(name) ?? false,
  );

const KIND_TAB = '[role="tablist"][aria-label="Search in"] [role="tab"]';
const MUSIC_PILL = '[role="group"][aria-label="What to find"] button';
const ARRANGE_PILL = '[role="group"][aria-label="Arrange results"] button';

const kindPill = (name: string) => chosen(KIND_TAB, 'aria-selected', name);
const musicPill = (name: string) => chosen(MUSIC_PILL, 'aria-pressed', name);
const arrangePill = (name: string) => chosen(ARRANGE_PILL, 'aria-pressed', name);

/** The button among `selector`'s whose text is (or starts with) `name`. */
const button = (selector: string, name: string) => () => {
  const found = [...document.querySelectorAll<HTMLElement>(selector)].find(
    (element) => element.textContent?.trim().startsWith(name) ?? false,
  );
  if (!found) throw new Error(`No pill named ${name}`);
  return found;
};
const kindTab = (name: string) => button(KIND_TAB, name);
const tapKind = (name: string) => () => fireEvent.click(kindTab(name)());
const tapPill = (selector: string, name: string) => () => fireEvent.click(button(selector, name)());

/** Each kind's first results, as they show on its own pill. */
const KINDS = [
  { name: 'All', rows: '[data-testid="all-results"] [data-testid="online-track-row"]' },
  { name: 'Audio', rows: '[data-testid="relevant-music"] [data-testid="online-track-row"]' },
  { name: 'Videos', rows: '[aria-label="From your sources"] [data-testid="manga-card"]' },
  { name: 'Comics', rows: '[data-testid="manga-search"] [data-testid="manga-card"]' },
  { name: 'Books', rows: '[data-testid="novel-search"] [data-testid="manga-card"]' },
  { name: 'Your library', rows: '[data-testid="library-matches"] [data-testid="manga-card"]' },
];

const MUSIC_PILLS = [
  { name: 'Relevant', rows: '[data-testid="relevant-music"] [data-testid="online-track-row"]' },
  { name: 'Songs', rows: '[data-testid="best-songs"] [data-testid="online-track-row"]' },
  { name: 'Artists', rows: '[data-testid="artist-finds"] li' },
  { name: 'Albums', rows: '[data-testid="album-finds"] li' },
  { name: 'Playlists', rows: '[data-testid="playlist-finds"] li' },
  { name: 'Podcasts', rows: '[data-testid="podcast-search"] li' },
  { name: 'Audiobooks', rows: '[data-testid="audiobook-search"] [data-testid="book-card"]' },
];

const mount = async (path: string) => {
  cleanup();
  const router = createAppRouter(createMemoryHistory({ initialEntries: [path] }));
  renderWithProviders(
    <Profiler id="app" onRender={onRender}>
      <RouterProvider router={router} />
    </Profiler>,
  );
  await screen.findByTestId('page-content', {}, { timeout: 60_000 });
};

/** Shows every pill once, so what each found is kept, as after a search's first look round. */
const visit = async (steps: { tap: () => void; rows: string }[]) => {
  for (const step of steps) {
    step.tap();
    await waitFor(() => expect(query(step.rows)).toBe(true), { timeout: 120_000 });
    await settle();
  }
};

describe.skipIf(!ENV.RAYGUN_PERF)('switching Search pills', { timeout: 900_000 }, () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(ok(SOURCES.manga));
    commands.novelSources.mockResolvedValue(ok(SOURCES.novel));
    commands.videoSources.mockResolvedValue(ok(SOURCES.video));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, ids) =>
      ok((ids as string[]).flatMap((id) => answerOf(id) ?? [])),
    );
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: Array.from({ length: 8 }, (_, index) => libraryTrack(index)),
        top: Array.from({ length: 30 }, (_, index) => song(index * 3)),
        providers: PROVIDERS,
      }),
    );
    commands.librarySearch.mockResolvedValue(
      ok(Array.from({ length: 12 }, (_, index) => libraryTrack(index))),
    );
    commands.musicSearchArtists.mockResolvedValue(ok(ARTIST_LIST));
    commands.musicSearchAlbums.mockResolvedValue(ok(ALBUM_LIST));
    commands.musicSearchPlaylists.mockResolvedValue(ok(PLAYLIST_LIST));
    commands.podcastSearch.mockResolvedValue(ok(PODCAST_LIST));
    commands.extensionsList.mockResolvedValue(
      ok([
        {
          id: 'dev.podcasts',
          name: 'Podcast directory',
          contentRating: 'safe',
          version: '1.0.0',
          description: 'Finds podcasts.',
          origin: 'installed',
          status: 'loaded',
          error: null,
          provides: ['podcast.directory'],
          net: ['podcasts.example'],
          providers: [
            {
              id: 'dev.podcasts/podcasts',
              extensionId: 'dev.podcasts',
              kind: 'podcast.directory',
              name: 'Podcast directory',
            },
          ],
          settings: [],
          compatibility: null,
          icon: null,
          website: null,
          repository: null,
        },
      ]),
    );
    commands.audiobookDirectory.mockResolvedValue(ok({ books: BOOK_LIST, hasMore: false }));
    commands.stremioCatalogs.mockResolvedValue(ok(ADDON_CATALOGS));
    commands.stremioSearchCatalog.mockImplementation(async (addonId, kind) =>
      ok(addonRow(addonId as string, kind as string)),
    );
    commands.mangaLibrary.mockResolvedValue(
      ok(
        Array.from({ length: LIBRARY_TITLES }, (_, index) =>
          libraryEntry({
            titleId: `t${index}`,
            remoteId: `t${index}`,
            sourceId: SOURCES.manga[0]!.id,
            title: index % 4 === 0 ? `Dark ${word(index)} ${index}` : `Light ${index}`,
            unread: index % 3,
          }),
        ),
      ),
    );
    // A phone's screen, for lists that only draw what's on it.
    vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(800);
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(400);
  });

  afterEach(() => {
    watch = null;
    vi.restoreAllMocks();
  });

  it('switches between the kinds', async () => {
    await mount('/browse?tab=search&q=dark');
    await waitFor(() => expect(query(KINDS[0]!.rows)).toBe(true), { timeout: 120_000 });
    await visit(KINDS.map((kind) => ({ tap: tapKind(kind.name), rows: kind.rows })));
    await visit([{ tap: tapKind('All'), rows: KINDS[0]!.rows }]);

    const profiled = await profile('kinds');
    for (const [index, kind] of KINDS.entries()) {
      const from = KINDS[(index + KINDS.length - 1) % KINDS.length]!;
      const runs: Switch[] = [];
      for (let turn = 0; turn < RUNS; turn++) {
        await visit([{ tap: tapKind(from.name), rows: from.rows }]);
        runs.push(
          await timeSwitch(kindTab(kind.name), kindPill(kind.name), () => query(kind.rows)),
        );
      }
      await note(
        `${from.name} → ${kind.name}: ${shown(bestOf(runs))}, ${count(kind.rows)} results drawn`,
      );
    }
    await profiled();
  });

  it("switches between Audio's pills", async () => {
    await mount('/browse?tab=search&type=music&q=dark');
    await waitFor(() => expect(query(MUSIC_PILLS[0]!.rows)).toBe(true), { timeout: 120_000 });
    await visit(
      MUSIC_PILLS.map((step) => ({ tap: tapPill(MUSIC_PILL, step.name), rows: step.rows })),
    );
    await visit([{ tap: tapPill(MUSIC_PILL, 'Relevant'), rows: MUSIC_PILLS[0]!.rows }]);

    const profiled = await profile('audio');
    for (const [index, step] of MUSIC_PILLS.entries()) {
      const from = MUSIC_PILLS[(index + MUSIC_PILLS.length - 1) % MUSIC_PILLS.length]!;
      const runs: Switch[] = [];
      for (let turn = 0; turn < RUNS; turn++) {
        await visit([{ tap: tapPill(MUSIC_PILL, from.name), rows: from.rows }]);
        runs.push(
          await timeSwitch(button(MUSIC_PILL, step.name), musicPill(step.name), () =>
            query(step.rows),
          ),
        );
      }
      await note(
        `Audio: ${from.name} → ${step.name}: ${shown(bestOf(runs))}, ` +
          `${count(step.rows)} results drawn`,
      );
    }
    await profiled();
  });

  it('arranges Comics and opens Custom sources', async () => {
    await mount('/browse?tab=search&type=manga&q=dark');
    const best = '[data-testid="ranked-results"] [data-testid="manga-card"]';
    const bySource =
      '[data-testid="manga-search"] [data-testid="source-row"] [data-testid="manga-card"]';
    await waitFor(() => expect(query(best)).toBe(true), { timeout: 120_000 });
    await visit([
      { tap: tapPill(ARRANGE_PILL, 'By source'), rows: bySource },
      { tap: tapPill(ARRANGE_PILL, 'Best matches'), rows: best },
    ]);

    const profiled = await profile('comics');
    const arrange = async (name: string, rows: string, back: string, backRows: string) => {
      const runs: Switch[] = [];
      for (let turn = 0; turn < RUNS; turn++) {
        await visit([{ tap: tapPill(ARRANGE_PILL, back), rows: backRows }]);
        runs.push(
          await timeSwitch(button(ARRANGE_PILL, name), arrangePill(name), () => query(rows)),
        );
      }
      await note(`Comics: ${back} → ${name}: ${shown(bestOf(runs))}, ${count(rows)} results drawn`);
    };
    await arrange('By source', bySource, 'Best matches', best);
    await arrange('Best matches', best, 'By source', bySource);

    const customPill = button('button[aria-pressed]', 'Custom sources');
    const open = () => query('[data-testid="source-tile"]');
    const opened: Switch[] = [];
    const closed: Switch[] = [];
    let tiles = 0;
    for (let turn = 0; turn < RUNS; turn++) {
      opened.push(
        await timeSwitch(
          customPill,
          () => customPill().getAttribute('aria-pressed') === 'true',
          open,
        ),
      );
      tiles = count('[data-testid="source-tile"]');
      closed.push(
        await timeSwitch(
          customPill,
          () => customPill().getAttribute('aria-pressed') === 'false',
          () => !query('[data-testid="custom-sources-panel"]') && query(best),
        ),
      );
    }
    await note(`Comics: Custom sources opened: ${shown(bestOf(opened))}, ${tiles} tiles drawn`);
    await note(`Comics: Custom sources closed: ${shown(bestOf(closed))}`);
    await profiled();
  });
});
