import type { MangaCard, MangaChapterInfo, MangaTitleView } from '@raygun/bindings';
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { sourceExtension } from '../test/sourceExtension';
import { createLimiter, videoSize } from './matchSize';
import { authorAgrees, languageFits, sourceFits } from './otherSourcesMatch';

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

  it('adds a match to a group and to favorites', async () => {
    const user = userEvent.setup();
    commands.libraryCategories.mockResolvedValue(ok([{ id: 'k1', name: 'Weekly', display: null }]));
    const section = await mount();
    await within(section).findByText('2 chapters');
    const menu = () =>
      within(section).getByRole('button', { name: 'More for the copy from Small Scans' });
    await user.click(menu());
    await user.click(await screen.findByRole('menuitem', { name: 'Add to group' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add to group' });
    await user.click(within(sheet).getByRole('checkbox', { name: 'Weekly' }));
    await user.click(within(sheet).getByRole('button', { name: 'Add to group' }));
    await vi.waitFor(() =>
      expect(commands.libraryAddTitles).toHaveBeenCalledWith(
        'manga',
        [{ sourceId: SMALL.id, remoteId: 'small', title: 'Frieren' }],
        ['k1'],
      ),
    );
    await user.click(menu());
    await user.click(await screen.findByRole('menuitem', { name: 'Favorite' }));
    await vi.waitFor(() =>
      expect(commands.librarySetFavorite).toHaveBeenCalledWith('manga', ['stored-small'], true),
    );
  });

  it('shows titles in full and searches again for the title and author typed', async () => {
    const user = userEvent.setup();
    commands.mangaTitle.mockImplementation((_source: string, id: string) =>
      Promise.resolve(
        id === 'here'
          ? ok({ ...VIEWS.here!, authors: ['Kanehito Yamada'] })
          : id === 'broken'
            ? { status: 'error' as const, error: 'The site is down' }
            : ok(VIEWS[id]),
      ),
    );
    commands.catalogGlobalSearch.mockImplementation(
      (_medium: string, query: string, only: string[]) =>
        Promise.resolve(
          ok(
            [SMALL, BIG]
              .filter((known) => only.includes(known.id))
              .map((known) => ({
                source: known,
                items: [
                  {
                    ...card(known.id, known === SMALL ? 'small' : 'big'),
                    title: query,
                    // Another writer's book of the same name, at one of them.
                    authors: [known === SMALL ? 'Kanehito Yamada' : 'Someone Else'],
                  },
                ],
                hasNextPage: false,
                error: null,
              })),
          ),
        ),
    );
    const section = await mount();
    const form = within(section).getByRole('search', { name: 'What to look for' });
    expect(within(form).getByRole('textbox', { name: 'Title' })).toHaveValue('Frieren');
    expect(within(form).getByRole('textbox', { name: 'Author' })).toHaveValue('Kanehito Yamada');
    // The other writer's copy is left out once its byline is known.
    const small = await within(section).findByText('Small Scans');
    expect(within(section).queryByText('Big Library')).not.toBeInTheDocument();
    expect(small.closest('li')).toHaveTextContent('Kanehito Yamada');

    const long = "Frieren: Beyond Journey's End, the Complete Collected Edition";
    const title = within(form).getByRole('textbox', { name: 'Title' });
    await user.clear(title);
    await user.type(title, `${long}{Enter}`);
    await vi.waitFor(() =>
      expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('manga', long, [SMALL.id]),
    );
    // A long title shows whole: it wraps, never cut short.
    const shown = await within(section).findByRole('link', { name: long });
    expect(shown).toHaveClass('wrap-anywhere');
    expect(shown).not.toHaveClass('truncate');

    // Without an author, every copy of the title is close.
    await user.clear(within(form).getByRole('textbox', { name: 'Author' }));
    expect(await within(section).findByText('Big Library')).toBeInTheDocument();
  });

  it('searches the sources in another language, with the title translated into it', async () => {
    const user = userEvent.setup();
    commands.extensionCatalog.mockResolvedValue(
      ok([
        {
          repo: { id: 'r', name: 'Repo', url: 'https://example.org/index.json' },
          error: null,
          entries: [
            { id: SMALL.id, lang: 'en' },
            { id: BIG.id, lang: 'ja' },
            { id: BROKEN.id, lang: 'all' },
          ],
        } as never,
      ]),
    );
    const section = await mount();
    await within(section).findByText('4 chapters');
    const form = within(section).getByRole('search', { name: 'What to look for' });
    await user.selectOptions(
      within(form).getByRole('combobox', { name: 'Language of the other sources' }),
      'ja',
    );
    expect(
      await within(form).findByText('2 sources in this language (or in many)'),
    ).toBeInTheDocument();
    // Only the sources in Japanese, or in many languages, are asked.
    await vi.waitFor(() => expect(rows(section)).toEqual(['Big Library', 'Broken']));

    await user.click(within(form).getByRole('switch', { name: /Translate the title into/ }));
    await vi.waitFor(() =>
      expect(commands.textTranslate).toHaveBeenCalledWith({
        text: 'Frieren',
        from: '',
        to: 'ja',
        translator: '',
        hint: '',
      }),
    );
    expect(await within(form).findByText(/Searching for “\[ja\] Frieren”/)).toBeInTheDocument();
    await vi.waitFor(() =>
      expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('manga', '[ja] Frieren', [BIG.id]),
    );
    expect(commands.catalogGlobalSearch).not.toHaveBeenCalledWith('manga', '[ja] Frieren', [
      SMALL.id,
    ]);
  });
});

describe('Find in other sources for audiobooks', () => {
  const BOOK = {
    id: 'b1',
    feedUrl: 'https://librivox.org/rss/42',
    title: 'Frankenstein; or, The Modern Prometheus',
    author: 'Mary Shelley',
    edition: null,
    description: null,
    artworkUrl: null,
    link: null,
    inLibrary: false,
    chapters: [],
    order: { sort: 'number' as const, descending: false, filters: [] },
    customOrder: false,
    chapterCount: 0,
    finished: 0,
    downloaded: 0,
    downloading: 0,
    totalMs: 0,
    listenedMs: 0,
    resume: null,
    speed: null,
  };

  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.extensionsList.mockResolvedValue(ok([sourceExtension(['audiobook.directory'])]));
    commands.audiobook.mockResolvedValue(ok(BOOK));
    commands.audiobooksElsewhere.mockResolvedValue(
      ok([
        {
          title: 'Frankenstein; or, The Modern Prometheus',
          author: 'Mary Shelley',
          edition: 'Version 2',
          feedUrl: 'https://librivox.org/rss/2817',
          artworkUrl: null,
          runtime: '8:30:00',
          language: 'English',
          description: null,
          source: 'LibriVox',
        },
        {
          title: 'Frankenstein',
          author: 'Mary Shelley',
          edition: null,
          feedUrl: 'https://www.loyalbooks.com/book/frankenstein/feed',
          artworkUrl: null,
          runtime: null,
          language: 'French',
          description: null,
          source: 'Loyal Books',
        },
      ]),
    );
    commands.audiobookOpen.mockResolvedValue(ok('b2'));
  });

  it('lists each source’s copies as it answers, before the slowest one', async () => {
    const user = userEvent.setup();
    let finish: () => void = () => {};
    const librivox = {
      title: 'Frankenstein',
      author: 'Mary Shelley',
      edition: null,
      feedUrl: 'https://librivox.org/rss/2817',
      artworkUrl: null,
      runtime: null,
      language: 'English',
      description: null,
      source: 'LibriVox',
    };
    const loyal = { ...librivox, feedUrl: 'https://loyalbooks.example/f', source: 'Loyal Books' };
    commands.audiobooksElsewhere.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = () => resolve(ok([librivox, loyal]));
        }),
    );
    await AppShellWrapper.mount('/audiobook?id=b1');
    const page = await screen.findByTestId('audiobook-page');
    await user.click(await within(page).findByRole('button', { name: 'Find in other sources' }));
    const section = await screen.findByTestId('other-sources');
    await vi.waitFor(() => expect(commands.audiobooksElsewhere).toHaveBeenCalled());
    const request = commands.audiobooksElsewhere.mock.lastCall?.[3] ?? '';
    emitEvent('audiobooksElsewhereEvent', ['another search', [loyal]]);
    expect(within(section).queryByText('Loyal Books')).not.toBeInTheDocument();
    emitEvent('audiobooksElsewhereEvent', [request, [librivox]]);
    expect(await within(section).findByText('LibriVox')).toBeInTheDocument();
    expect(within(section).queryByText('Loyal Books')).not.toBeInTheDocument();
    await act(async () => finish());
    expect(await within(section).findByText('Loyal Books')).toBeInTheDocument();
  });

  it('lists the book at the other sources, in the language picked, to add or open', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/audiobook?id=b1');
    const page = await screen.findByTestId('audiobook-page');
    await user.click(await within(page).findByRole('button', { name: 'Find in other sources' }));
    const section = await screen.findByTestId('other-sources');
    expect(await within(section).findByText('Loyal Books')).toBeInTheDocument();
    expect(commands.audiobooksElsewhere).toHaveBeenCalledWith(
      'Frankenstein; or, The Modern Prometheus',
      'Mary Shelley',
      'https://librivox.org/rss/42',
      expect.any(String),
    );
    // The whole title, and the recording, length and language under it.
    const row = within(section).getByText('LibriVox').closest('li') as HTMLElement;
    expect(within(row).getByText('Frankenstein; or, The Modern Prometheus')).toBeInTheDocument();
    expect(row).toHaveTextContent('Version 2 · 8 h 30 min · English');

    await user.selectOptions(
      within(section).getByRole('combobox', { name: 'Language of the other sources' }),
      'fr',
    );
    await vi.waitFor(() => expect(within(section).queryByText('LibriVox')).not.toBeInTheDocument());

    await user.click(within(section).getByRole('button', { name: 'Add to library: Frankenstein' }));
    await vi.waitFor(() => expect(commands.podcastSetSubscribed).toHaveBeenCalledWith('b2', true));
    expect(commands.audiobookOpen).toHaveBeenCalledWith(
      'https://www.loyalbooks.com/book/frankenstein/feed',
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

describe('Matching beyond the title', () => {
  it('tells bylines, sources and books apart by author and language', () => {
    expect(authorAgrees('Mary Shelley', ['Shelley, Mary Wollstonecraft'])).toBe(true);
    expect(authorAgrees('Mary Shelley', ['Bram Stoker'])).toBe(false);
    expect(authorAgrees('', ['Bram Stoker'])).toBe(true);
    expect(authorAgrees('Mary Shelley', [])).toBe(true);
    expect(sourceFits('ja', 'ja')).toBe(true);
    expect(sourceFits('all', 'ja')).toBe(true);
    expect(sourceFits('en', 'ja')).toBe(false);
    expect(sourceFits(null, 'ja')).toBe(false);
    expect(sourceFits(null, '')).toBe(true);
    expect(languageFits('English', 'en')).toBe(true);
    expect(languageFits('fr-CA', 'fr')).toBe(true);
    expect(languageFits('German', 'fr')).toBe(false);
    expect(languageFits(null, 'fr')).toBe(true);
  });
});
