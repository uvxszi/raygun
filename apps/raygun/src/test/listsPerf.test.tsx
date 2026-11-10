import type {
  CatalogEntry,
  ExtensionInfo,
  MangaSource,
  RepoCatalog,
  SavedTrack,
} from '@raygun/bindings';
import { QueryClient, dehydrate } from '@tanstack/react-query';
import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { packQueries } from '../shell/cachePack';
import { bindingsMock } from './bindingsMock';
import { libraryEntry } from './libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('./bindingsMock')).bindingsMock);

/**
 * Timings of the app's longest lists with many entries: thousands of catalogue entries from many
 * repositories, hundreds of installed extensions, thousands of sources and songs. Opt-in, as it's
 * slow: `RAYGUN_PERF=1 pnpm --filter @raygun/app exec vitest run src/test/listsPerf` (with
 * `RAYGUN_PERF_OUT=<file>` the timings go to that file). Each step runs a few times and the best
 * is kept, wall time and the process's CPU time: other work on the computer stretches wall time.
 * jsdom and React's development build are slower than a phone's webview; compare runs, not
 * absolute numbers.
 */

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands, events } = bindingsMock;

const REPOS = 20;
const PER_REPO = 250;
const INSTALLED = 400;
const SOURCES = 3000;
const SONGS = 3000;
const RUNS = 3;

const WORDS = ['comics', 'novels', 'radio', 'films', 'manga', 'webtoons', 'scans', 'books'];
const KINDS = [
  ['manga.source'],
  ['novel.source'],
  ['music.metadata', 'music.stream'],
  ['video.source'],
  ['podcast.directory'],
];
const LANGS = ['en', 'ja', 'fr', 'es', 'multi', null];

const catalogEntry = (repo: number, index: number): CatalogEntry => {
  const mihon = index % 3 === 0;
  const state = index % 50 === 0 ? 'installed' : index % 97 === 0 ? 'updateAvailable' : 'available';
  return {
    repoId: `repo-${repo}`,
    id: mihon ? `mihon.en.source${repo}x${index}` : `dev.repo${repo}.ext${index}`,
    name: `Source ${repo}-${index} ${WORDS[index % WORDS.length]}`,
    version: '1.2.3',
    description: `Reads ${WORDS[(index + 3) % WORDS.length]} from example${index}.org, with search, latest and filters.`,
    author: 'Someone',
    provides: KINDS[index % KINDS.length] ?? [],
    net: [`example${index}.org`],
    contentRating: index % 17 === 0 ? 'nsfw' : 'safe',
    adult: index % 17 === 0,
    lang: LANGS[index % LANGS.length] ?? null,
    size: 12_000,
    state,
    installedVersion: state === 'available' ? null : '1.2.0',
    nuclear: false,
    icon: `http://127.0.0.1:9/cover/${repo}-${index}`,
    unsupported: mihon ? 'Raygun can’t run Mihon extensions on this device yet.' : null,
  };
};

const CATALOG: RepoCatalog[] = Array.from({ length: REPOS }, (_, repo) => ({
  repo: { id: `repo-${repo}`, url: `https://example${repo}.org/index.json`, name: `Repo ${repo}` },
  entries: Array.from({ length: PER_REPO }, (_, index) => catalogEntry(repo, index)),
  error: null,
}));

const extension = (index: number): ExtensionInfo => ({
  id: `dev.example.ext${index}`,
  name: `Extension ${index} ${WORDS[index % WORDS.length]}`,
  contentRating: 'safe',
  version: '1.0.0',
  description: `Reads ${WORDS[(index + 1) % WORDS.length]} from example${index}.org.`,
  origin: 'installed',
  status: 'loaded',
  error: null,
  provides: KINDS[index % KINDS.length] ?? [],
  net: [`example${index}.org`],
  providers: (KINDS[index % KINDS.length] ?? []).map((kind) => ({
    id: `dev.example.ext${index}/${kind}`,
    extensionId: `dev.example.ext${index}`,
    kind,
    name: `Extension ${index}`,
  })),
  settings: [],
  compatibility: null,
  icon: `http://127.0.0.1:9/cover/ext-${index}`,
  website: null,
  repository: 'Repo 1',
});

const EXTENSIONS = Array.from({ length: INSTALLED }, (_, index) => extension(index));

const mangaSource = (index: number): MangaSource => ({
  id: `dev.example.ext${index % INSTALLED}/source${index}`,
  name: `Source ${index} ${WORDS[index % WORDS.length]}`,
  extensionId: `dev.example.ext${index % INSTALLED}`,
  enabled: true,
  adult: false,
});

const song = (index: number): SavedTrack => ({
  trackKey: `artist ${index % 300}|song ${index}`,
  title: `Song ${index} ${WORDS[index % WORDS.length]}`,
  artist: `Artist ${index % 300}`,
  album: `Album ${index % 500}`,
  durationMs: 180_000 + index,
  artworkUrl: `http://127.0.0.1:9/cover/song-${index}`,
  source: null,
  local: false,
  addedAt: 1_700_000_000_000 + index,
  playCount: index % 7,
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

type Timing = { wall: number; cpu: number };

/** How long `step` takes until `done` holds: wall time and CPU time, in ms. */
const time = async (step: () => unknown, done: () => void): Promise<Timing> => {
  const wall = performance.now();
  const cpu = cpuMs();
  await step();
  await waitFor(done, { timeout: 120_000, interval: 5 });
  return { wall: Math.round(performance.now() - wall), cpu: Math.round(cpuMs() - cpu) };
};

/** The best of `RUNS` runs of `run` (given the run's number). */
const best = async (run: (turn: number) => Promise<Timing>): Promise<Timing> => {
  let found: Timing = { wall: Infinity, cpu: Infinity };
  for (let turn = 0; turn < RUNS; turn++) {
    const timing = await run(turn);
    found = { wall: Math.min(found.wall, timing.wall), cpu: Math.min(found.cpu, timing.cpu) };
  }
  return found;
};

const shown = (timing: Timing) => `${timing.wall} ms (CPU ${timing.cpu} ms)`;

/** Opens `path` in a fresh app, waiting as long as a long list takes. */
const open = (path: string, done: () => void) =>
  best(() => {
    cleanup();
    return time(() => AppShellWrapper.mount(path).catch(() => undefined), done);
  });

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

describe.skipIf(!ENV.RAYGUN_PERF)('long lists', { timeout: 600_000 }, () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.extensionCatalog.mockResolvedValue(ok(CATALOG));
    commands.extensionRepos.mockResolvedValue(ok(CATALOG.map((listing) => listing.repo)));
    commands.extensionsList.mockResolvedValue(ok(EXTENSIONS));
    commands.mangaSources.mockResolvedValue(
      ok(Array.from({ length: SOURCES }, (_, index) => mangaSource(index))),
    );
    commands.favoritesList.mockResolvedValue(
      ok(Array.from({ length: SONGS }, (_, index) => song(index))),
    );
    // A phone's screen, for lists that only draw what's on it.
    vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(800);
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(400);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('opens the extension catalogue and searches it', async () => {
    const listens = events.mihonRuntimeEvent.listen.mock.calls.length;
    const opened = await open('/browse?tab=extensions&list=all', () =>
      expect(screen.getAllByTestId('catalog-entry').length).toBeGreaterThan(0),
    );
    await note(
      `catalogue (${REPOS * PER_REPO} entries): opened in ${shown(opened)}, ` +
        `${screen.getAllByTestId('catalog-entry').length} rows drawn, ` +
        `${(events.mihonRuntimeEvent.listen.mock.calls.length - listens) / RUNS} event listeners`,
    );
    const field = screen.getByRole('searchbox', { name: 'Search extensions' });
    const typed = await best((turn) => {
      const [query, gone] =
        turn % 2 ? ['novels', 'Source 0-0 comics'] : ['comics', 'Source 0-1 novels'];
      return time(
        () => fireEvent.change(field, { target: { value: query } }),
        () => expect(screen.queryByText(gone)).not.toBeInTheDocument(),
      );
    });
    await note(`catalogue: one search keystroke ${shown(typed)}`);
    fireEvent.change(field, { target: { value: '' } });
    const pill = await best(() =>
      time(
        () => fireEvent.click(screen.getByRole('button', { name: 'Comics' })),
        () => expect(screen.getAllByTestId('catalog-entry').length).toBeGreaterThan(0),
      ),
    );
    await note(`catalogue: kind pill ${shown(pill)}`);
  });

  it('opens Sources and switches one extension off', async () => {
    const opened = await open('/sources', () =>
      expect(screen.getAllByTestId('source-extension-row').length).toBeGreaterThan(0),
    );
    await note(
      `Sources (${INSTALLED} extensions, ${REPOS} repositories): opened in ${shown(opened)}, ` +
        `${screen.getAllByTestId('source-extension-row').length} rows drawn`,
    );
    const switched = await best((turn) => {
      const toggle = () => screen.getAllByRole('switch', { name: /^Use Extension 0 / })[0];
      const want = turn % 2 ? 'true' : 'false';
      return time(
        () => fireEvent.click(toggle()!),
        () => expect(toggle()).toHaveAttribute('aria-checked', want),
      );
    });
    await note(`Sources: one switch ${shown(switched)}`);
  });

  it('opens Browse → Sources and finds one', async () => {
    const opened = await open('/browse?tab=sources', () =>
      expect(screen.getAllByRole('link', { name: /^Source 1 / }).length).toBeGreaterThan(0),
    );
    await note(`Browse → Sources (${SOURCES} sources): opened in ${shown(opened)}`);
    const field = screen.getByRole('searchbox', { name: 'Find a source' });
    const typed = await best((turn) => {
      const [query, gone] = turn % 2 ? ['source 28', /^Source 29 /] : ['source 29', /^Source 28 /];
      return time(
        () => fireEvent.change(field, { target: { value: query } }),
        () => expect(screen.queryAllByRole('link', { name: gone })).toHaveLength(0),
      );
    });
    await note(`Browse → Sources: one search keystroke ${shown(typed)}`);
  });

  it('opens a comics library of thousands of titles', async () => {
    commands.mangaLibrary.mockResolvedValue(
      ok(
        Array.from({ length: SONGS }, (_, index) =>
          libraryEntry({
            titleId: `t${index}`,
            remoteId: `t${index}`,
            sourceId: 'dev.example.comics/comics',
            title: `Title ${index} ${WORDS[index % WORDS.length]}`,
            unread: index % 4,
          }),
        ),
      ),
    );
    const opened = await open('/library?media=manga', () =>
      expect(screen.getAllByTestId('library-item').length).toBeGreaterThan(0),
    );
    await note(
      `comics library (${SONGS} titles): opened in ${shown(opened)}, ` +
        `${screen.getAllByTestId('library-item').length} titles drawn`,
    );
  });

  it('saves the query cache again after one answer changed', async () => {
    const client = new QueryClient();
    client.setQueryData(['extensionCatalog'], CATALOG);
    client.setQueryData(['extensions'], EXTENSIONS);
    client.setQueryData(
      ['manga', 'sources'],
      Array.from({ length: SOURCES }, (_, index) => mangaSource(index)),
    );
    client.setQueryData(
      ['collections', 'favorites'],
      Array.from({ length: SONGS }, (_, index) => song(index)),
    );
    const save = () => {
      const started = performance.now();
      packQueries(dehydrate(client));
      return Math.round(performance.now() - started);
    };
    const first = save();
    let again = Infinity;
    for (let turn = 0; turn < RUNS; turn++) {
      client.setQueryData(['settings', 'zoom'], turn);
      again = Math.min(again, save());
    }
    await note(`query cache save: first ${first} ms, again after one change ${again} ms`);
  });

  it('opens a table of thousands of songs and filters it', async () => {
    const opened = await open('/library?view=favorites', () =>
      expect(screen.getAllByTestId('track-table-row').length).toBeGreaterThan(0),
    );
    await note(
      `songs (${SONGS}): opened in ${shown(opened)}, ` +
        `${screen.getAllByTestId('track-table-row').length} rows drawn`,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Find in these songs' }));
    const field = screen.getByRole('searchbox', { name: 'Find in these songs' });
    const typed = await best((turn) => {
      const [query, gone] =
        turn % 2 ? ['song 28', 'Song 29 webtoons'] : ['song 29', 'Song 28 manga'];
      return time(
        () => fireEvent.change(field, { target: { value: query } }),
        () => expect(screen.queryByText(gone)).not.toBeInTheDocument(),
      );
    });
    await note(`songs: one filter keystroke ${shown(typed)}`);
  });
});
