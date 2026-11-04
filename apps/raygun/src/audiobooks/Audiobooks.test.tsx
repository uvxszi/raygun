import type { AudiobookResult, AudiobookView, PodcastEpisode } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const chapter = (id: string, title: string, played = false, positionMs = 0): PodcastEpisode => ({
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
});

const book = (inLibrary: boolean, chapters: PodcastEpisode[]): AudiobookView => ({
  id: 'b1',
  feedUrl: 'https://librivox.org/rss/253',
  title: 'Pride and Prejudice',
  author: 'Jane Austen',
  description: 'A novel.',
  artworkUrl: null,
  link: null,
  inLibrary,
  chapters,
});

const result = (title: string): AudiobookResult => ({
  title,
  author: 'Jane Austen',
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
          artworkUrl: null,
          chapters: 37,
          finished: 3,
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
        book(false, [
          chapter('c1', 'Chapters 1-3', true),
          chapter('c2', 'Chapters 4-5', false, 60_000),
          chapter('c3', 'Chapters 6-8'),
        ]),
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
    expect(commands.audiobookPlay).toHaveBeenLastCalledWith('b1', 'c1');
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
});
