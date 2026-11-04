import type { MangaCard, MangaChapterInfo, MangaTitleView } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { createLimiter, videoSize } from './matchSize';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const source = (id: string, name: string) => ({ id, name, extensionId: id, enabled: true });
const HERE = source('dev.example/here', 'Here');
const SMALL = source('dev.example/small', 'Small Scans');
const BIG = source('dev.example/big', 'Big Library');
const BROKEN = source('dev.example/broken', 'Broken');

const chapter = (number: number, dateUpload: number | null = null): MangaChapterInfo => ({
  id: `c${number}`,
  name: `Chapter ${number}`,
  number,
  volume: null,
  scanlator: null,
  dateUpload,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
});

const view = (
  sourceId: string,
  remoteId: string,
  chapters: MangaChapterInfo[],
  inLibrary = false,
): MangaTitleView => ({
  id: `stored-${remoteId}`,
  hiddenNsfw: false,
  sourceId,
  sourceName: '',
  remoteId,
  title: 'Frieren',
  coverUrl: null,
  authors: [],
  artists: [],
  description: null,
  genres: [],
  status: 'ongoing',
  url: null,
  inLibrary,
  chapters,
  continueChapterId: null,
});

const card = (sourceId: string, id: string): MangaCard => ({
  sourceId,
  id,
  title: 'Frieren',
  coverUrl: null,
  inLibrary: false,
});

const VIEWS: Record<string, MangaTitleView> = {
  here: view(HERE.id, 'here', [chapter(1)], true),
  small: view(SMALL.id, 'small', [chapter(1), chapter(2)]),
  big: view(BIG.id, 'big', [
    chapter(1),
    chapter(2),
    chapter(3),
    chapter(3.5, Date.UTC(2026, 9, 1)),
  ]),
};

const mount = async () => {
  await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(HERE.id)}&id=here`);
  const page = await screen.findByTestId('manga-title-page');
  await userEvent
    .setup()
    .click(within(page).getByRole('button', { name: 'Find in other sources' }));
  return screen.findByTestId('other-sources');
};

const rows = (section: HTMLElement) =>
  within(section)
    .getAllByRole('listitem')
    .map((item) => within(item).getAllByText(/^(Small Scans|Big Library|Broken)$/)[0]?.textContent);

describe('Find in other sources', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(ok([HERE, SMALL, BIG, BROKEN]));
    commands.mangaTitle.mockImplementation((_source: string, id: string) =>
      id === 'broken'
        ? Promise.resolve({ status: 'error' as const, error: 'The site is down' })
        : Promise.resolve(ok(VIEWS[id])),
    );
    commands.catalogGlobalSearch.mockImplementation(
      (_medium: string, _query: string, only: string[]) =>
        Promise.resolve(
          ok(
            [SMALL, BIG, BROKEN]
              .filter((known) => only.includes(known.id))
              .map((known) => ({
                source: known,
                items: [
                  card(known.id, known === SMALL ? 'small' : known === BIG ? 'big' : 'broken'),
                ],
                hasNextPage: false,
                error: null,
              })),
          ),
        ),
    );
  });

  it("shows each match's size, marks the one with the most and sorts by it", async () => {
    const user = userEvent.setup();
    const section = await mount();
    const big = await within(section).findByText('4 chapters');
    expect(within(section).getByText('2 chapters')).toBeInTheDocument();
    const bigRow = big.closest('li') as HTMLElement;
    expect(within(bigRow).getByText('Most chapters')).toBeInTheDocument();
    expect(within(bigRow).getByText('Latest ch. 3.5')).toBeInTheDocument();
    expect(within(bigRow).getByText(/2026/)).toBeInTheDocument();
    // A source whose title page fails shows no chips and no error.
    await vi.waitFor(() =>
      expect(within(section).queryByTestId('match-size-loading')).not.toBeInTheDocument(),
    );
    expect(within(section).queryByText(/The site is down/)).not.toBeInTheDocument();

    await user.click(within(section).getByRole('radio', { name: 'Most chapters' }));
    expect(rows(section)).toEqual(['Big Library', 'Small Scans', 'Broken']);
  });

  it('moves the library entry to another source from the match menu', async () => {
    const user = userEvent.setup();
    const section = await mount();
    await within(section).findByText('4 chapters');
    await user.click(
      within(section).getByRole('button', { name: 'More for the copy from Big Library' }),
    );
    await user.click(await screen.findByRole('menuitem', { name: 'Use this source instead' }));
    const confirm = await screen.findByRole('alertdialog');
    await user.click(within(confirm).getByRole('button', { name: 'Use this source instead' }));
    expect(commands.libraryMigrate).toHaveBeenCalledWith('manga', 'stored-here', BIG.id, 'big');
  });

  it('adds a match to a category and to favorites', async () => {
    const user = userEvent.setup();
    commands.libraryCategories.mockResolvedValue(ok([{ id: 'k1', name: 'Weekly', display: null }]));
    const section = await mount();
    await within(section).findByText('2 chapters');
    const menu = () =>
      within(section).getByRole('button', { name: 'More for the copy from Small Scans' });
    await user.click(menu());
    await user.click(await screen.findByRole('menuitem', { name: 'Add to category' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add to category' });
    await user.click(within(sheet).getByRole('button', { name: 'Weekly' }));
    await vi.waitFor(() =>
      expect(commands.libraryChangeCategories).toHaveBeenCalledWith(
        'manga',
        ['stored-small'],
        ['k1'],
        [],
      ),
    );
    expect(commands.mangaSetInLibrary).toHaveBeenCalledWith('stored-small', true);
    await user.click(menu());
    await user.click(await screen.findByRole('menuitem', { name: 'Favorite' }));
    await vi.waitFor(() =>
      expect(commands.librarySetFavorite).toHaveBeenCalledWith('manga', ['stored-small'], true),
    );
  });
});

describe('Match sizes', () => {
  it('counts seasons and the latest episode, leaving specials out', () => {
    const episode = (season: number | null, number: number, durationMs: number | null) => ({
      id: `${season}-${number}`,
      remoteId: `${season}-${number}`,
      name: '',
      number,
      season,
      date: null,
      positionMs: 0,
      durationMs,
      watched: false,
      downloaded: false,
    });
    const size = videoSize({
      id: 't',
      sourceId: 's',
      sourceName: '',
      remoteId: 'r',
      title: 'Show',
      coverUrl: null,
      mediaType: 'series',
      year: null,
      description: null,
      genres: [],
      cast: [],
      directors: [],
      status: '',
      url: null,
      inLibrary: false,
      hiddenNsfw: false,
      episodes: [
        episode(0, 1, 1_200_000),
        episode(1, 1, 1_200_000),
        episode(1, 2, 1_200_000),
        episode(2, 1, 1_200_000),
      ],
      continueEpisodeId: null,
    });
    expect(size).toMatchObject({
      units: 4,
      seasons: 2,
      latestSeason: 2,
      latest: 1,
      durationMs: 4_800_000,
    });
  });

  it('runs a few tasks at once', async () => {
    const limit = createLimiter(2);
    let running = 0;
    let most = 0;
    const task = () =>
      limit(async () => {
        running += 1;
        most = Math.max(most, running);
        await new Promise((resolve) => setTimeout(resolve, 5));
        running -= 1;
      });
    await Promise.all([task(), task(), task(), task(), task()]);
    expect(most).toBe(2);
  });
});
