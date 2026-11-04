import type { AudiobookResult, AudiobookView, PodcastEpisode } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const chapter = (
  id: string,
  title: string,
  played = false,
  positionMs = 0,
  number: number | null = null,
): PodcastEpisode => ({
  id,
  podcastId: 'b1',
  podcastTitle: 'Pride and Prejudice',
  title,
  description: null,
  publishedAt: null,
  durationMs: 1_200_000,
  positionMs,
  played,
  artworkUrl: null,
  video: false,
  download: null,
  number,
});

const book = (
  inLibrary: boolean,
  chapters: PodcastEpisode[],
  more: Partial<AudiobookView> = {},
): AudiobookView => ({
  id: 'b1',
  feedUrl: 'https://librivox.org/rss/253',
  title: 'Pride and Prejudice',
  author: 'Jane Austen',
  edition: null,
  description: 'A novel.',
  artworkUrl: null,
  link: null,
  inLibrary,
  chapters,
  order: { sort: 'number', descending: false, filters: [] },
  customOrder: false,
  chapterCount: chapters.length,
  finished: chapters.filter((one) => one.played).length,
  downloaded: 0,
  downloading: 0,
  totalMs: chapters.length * 1_200_000,
  listenedMs: 0,
  resume: chapters[0]
    ? { chapterId: chapters[0].id, number: 1, title: chapters[0].title, positionMs: 0 }
    : null,
  speed: null,
  ...more,
});

const result = (title: string): AudiobookResult => ({
  title,
  author: 'Jane Austen',
  edition: null,
  feedUrl: `https://librivox.org/rss/${title.length}`,
  artworkUrl: null,
  runtime: '13:06:44',
  language: 'eng',
  description: null,
  source: 'LibriVox',
});

describe('Audiobooks', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('lists books in Library → Audio with how far each is', async () => {
    commands.audiobooks.mockResolvedValue(
      ok([
        {
          id: 'b1',
          title: 'Pride and Prejudice',
          author: 'Jane Austen',
          edition: null,
          artworkUrl: null,
          chapters: 37,
          finished: 3,
          totalMs: 0,
          listenedMs: 0,
        },
      ]),
    );
    await AppShellWrapper.mount('/library?view=audiobooks');
    const card = await screen.findByTestId('audiobook-card');
    expect(card).toHaveTextContent('Pride and Prejudice');
    expect(card).toHaveTextContent('Chapter 4 of 37');
  });

  it('says where to find books when there are none: Search → Audio → Audiobooks', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?view=audiobooks');
    expect(await screen.findByText('No audiobooks yet')).toBeInTheDocument();
    await user.click(screen.getByRole('link', { name: 'Find audiobooks' }));
    expect(await screen.findByTestId('audiobook-search')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Audio' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('button', { name: 'Audiobooks' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('resumes a book at the first chapter not finished, and adds it to the library', async () => {
    const user = userEvent.setup();
    commands.audiobook.mockResolvedValue(
      ok(
        book(
          false,
          [
            chapter('c1', 'Chapters 1-3', true, 0, 1),
            chapter('c2', 'Chapters 4-5', false, 60_000, 2),
            chapter('c3', 'Chapters 6-8', false, 0, 3),
          ],
          {
            resume: { chapterId: 'c2', number: 2, title: 'Chapters 4-5', positionMs: 60_000 },
            listenedMs: 1_260_000,
          },
        ),
      ),
    );
    commands.podcastSetSubscribed.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/audiobook?id=b1');
    const page = await screen.findByTestId('audiobook-page');
    expect(within(page).getByRole('heading', { level: 1 })).toHaveTextContent(
      'Pride and Prejudice',
    );
    await user.click(within(page).getByRole('button', { name: 'Resume chapter 2' }));
    expect(commands.audiobookPlay).toHaveBeenCalledWith('b1', null);
    await user.click(within(page).getByRole('button', { name: 'From the start' }));
    // The book's very start: the first chapter's place is forgotten, then it plays.
    expect(commands.podcastForgetPosition).toHaveBeenCalledWith('c1');
    await vi.waitFor(() => expect(commands.audiobookPlay).toHaveBeenLastCalledWith('b1', 'c1'));
    await user.click(within(page).getByRole('button', { name: 'Add to library' }));
    expect(commands.podcastSetSubscribed).toHaveBeenCalledWith('b1', true);
  });

  it('finds books and opens one', async () => {
    const user = userEvent.setup();
    commands.audiobookDirectory.mockImplementation(async (list: string) =>
      ok({
        books: list === 'search' ? [result('Emma')] : [result('Persuasion')],
        hasMore: false,
      }),
    );
    commands.audiobookOpen.mockResolvedValue(ok('b1'));
    commands.audiobook.mockResolvedValue(ok(book(false, [chapter('c1', 'Chapter 1')])));
    // Old links land on Search's Audiobooks pill: popular books until something is typed.
    await AppShellWrapper.mount('/audiobooks');
    const browse = await screen.findByTestId('audiobook-search');
    expect(
      await within(within(browse).getByRole('region', { name: 'Popular' })).findByText(
        'Persuasion',
      ),
    ).toBeInTheDocument();
    await user.type(screen.getByRole('searchbox', { name: 'Search' }), 'emma{Enter}');
    const results = await screen.findByRole('region', { name: 'Books found' });
    await user.click(await within(results).findByText('Emma'));
    expect(commands.audiobookOpen).toHaveBeenCalledWith('https://librivox.org/rss/4');
    expect(await screen.findByTestId('audiobook-page')).toBeInTheDocument();
  });

  it('browses a genre from every source and shows each book’s recording and source', async () => {
    const user = userEvent.setup();
    commands.audiobookDirectory.mockImplementation(async (list: string, query: string) =>
      ok({
        books:
          list === 'subject' && query === 'Science fiction'
            ? [{ ...result('The Time Machine'), edition: 'Version 3', source: 'Lit2Go' }]
            : [],
        hasMore: false,
      }),
    );
    await AppShellWrapper.mount('/audiobooks');
    const browse = await screen.findByTestId('audiobook-search');
    await user.click(within(browse).getByRole('button', { name: 'Science fiction' }));
    expect(commands.audiobookDirectory).toHaveBeenCalledWith('subject', 'Science fiction', 1);
    const genre = await within(browse).findByRole('region', { name: 'Science fiction' });
    const card = await within(genre).findByTestId('book-card');
    expect(card).toHaveTextContent('The Time Machine');
    expect(card).toHaveTextContent('Version 3 · 13 h 7 min · Lit2Go');
  });

  it('numbers chapters, plays a finished book again and downloads it whole', async () => {
    const user = userEvent.setup();
    commands.audiobook.mockResolvedValue(
      ok(
        book(
          true,
          [chapter('c1', 'Chapter 1', true, 0, 1), chapter('c2', 'Chapter 2', true, 0, 2)],
          { resume: null, listenedMs: 2_400_000, edition: 'Version 2' },
        ),
      ),
    );
    await AppShellWrapper.mount('/audiobook?id=b1');
    const page = await screen.findByTestId('audiobook-page');
    expect(within(page).getByText(/Version 2 · 2 chapters/)).toBeInTheDocument();
    const second = within(page).getByRole('listitem', { name: 'Chapter 2' });
    expect(within(second).getByText('2')).toBeInTheDocument();
    expect(
      within(second).getByRole('button', { name: 'Mark as not finished' }),
    ).toBeInTheDocument();

    await user.click(within(page).getByRole('button', { name: 'Play again' }));
    expect(commands.audiobookPlay).toHaveBeenCalledWith('b1', 'c1');
    await user.click(within(page).getByRole('button', { name: 'Download book' }));
    expect(commands.audiobookDownload).toHaveBeenCalledWith('b1');
  });

  it('lists books and episodes to go on with on Home', async () => {
    const user = userEvent.setup();
    commands.podcastContinueListening.mockResolvedValue(
      ok([
        {
          kind: 'audiobook',
          podcastId: 'b1',
          episodeId: 'c4',
          title: 'Chapter 4',
          show: 'Pride and Prejudice',
          author: 'Jane Austen',
          artworkUrl: null,
          positionMs: 60_000,
          durationMs: 660_000,
          chapter: 4,
          chapters: 37,
          listenedAt: 2,
        },
        {
          kind: 'podcast',
          podcastId: 'p1',
          episodeId: 'e7',
          title: 'Episode Seven',
          show: 'The Example Show',
          author: null,
          artworkUrl: null,
          positionMs: 0,
          durationMs: null,
          chapter: null,
          chapters: null,
          listenedAt: 1,
        },
      ]),
    );
    await AppShellWrapper.mount('/');
    const row = await screen.findByTestId('continue-listening');
    const [bookCard, episodeCard] = await within(row).findAllByTestId('listening-card');
    expect(bookCard).toHaveTextContent('Pride and Prejudice');
    expect(bookCard).toHaveTextContent('Chapter 4 of 37 · 10 min left');
    await user.click(bookCard!);
    expect(commands.audiobookPlay).toHaveBeenCalledWith('b1', null);
    await user.click(episodeCard!);
    expect(commands.podcastPlay).toHaveBeenCalledWith('e7');
    await user.click(
      within(row).getByRole('button', {
        name: 'Remove Episode Seven from Continue listening',
      }),
    );
    expect(commands.podcastForgetPosition).toHaveBeenCalledWith('e7');
  });
});
