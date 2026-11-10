import type {
  AudiobookSummary,
  AudiobookView,
  PodcastEpisode,
  PodcastSummary,
  PodcastView,
} from '@raygun/bindings';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { HOLD_MS } from '../select/scopeContext';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const episode = (id: string, title: string, podcastId = 'p1'): PodcastEpisode => ({
  id,
  podcastId,
  podcastTitle: podcastId === 'p1' ? 'The Example Show' : 'Another Show',
  title,
  description: null,
  publishedAt: Date.UTC(2026, 9, 1),
  durationMs: 3_600_000,
  positionMs: 0,
  played: false,
  artworkUrl: null,
  video: false,
  download: null,
  number: null,
});

const show = (id: string, title: string): PodcastSummary => ({
  id,
  feedUrl: `https://feeds.example/${id}.xml`,
  title,
  author: 'Ana',
  artworkUrl: null,
  episodes: 2,
  unplayed: 1,
  latestPublishedAt: Date.UTC(2026, 9, 1),
});

const SHOW: PodcastView = {
  id: 'p1',
  feedUrl: 'https://feeds.example/p1.xml',
  title: 'The Example Show',
  author: 'Ana',
  description: null,
  artworkUrl: null,
  link: null,
  subscribed: true,
  episodes: [episode('e1', 'Episode One'), episode('e2', 'Episode Two')],
  order: { sort: 'date', descending: true, filters: [] },
  customOrder: false,
  episodeCount: 2,
  downloaded: 0,
  speed: null,
};

const summary = (id: string, title: string): AudiobookSummary => ({
  id,
  title,
  author: 'Jane Austen',
  edition: null,
  artworkUrl: null,
  chapters: 3,
  finished: 0,
  totalMs: 0,
  listenedMs: 0,
});

const chapter = (id: string, title: string, number: number): PodcastEpisode => ({
  ...episode(id, title, 'b1'),
  podcastTitle: 'Emma',
  publishedAt: null,
  number,
});

const BOOK: AudiobookView = {
  id: 'b1',
  feedUrl: 'https://librivox.org/rss/1',
  title: 'Emma',
  author: 'Jane Austen',
  edition: null,
  description: null,
  artworkUrl: null,
  link: null,
  inLibrary: true,
  chapters: [chapter('c1', 'Chapter 1', 1), chapter('c2', 'Chapter 2', 2)],
  order: { sort: 'number', descending: false, filters: [] },
  customOrder: false,
  chapterCount: 2,
  finished: 0,
  downloaded: 0,
  downloading: 0,
  totalMs: 7_200_000,
  listenedMs: 0,
  resume: { chapterId: 'c1', number: 1, title: 'Chapter 1', positionMs: 0 },
  speed: null,
};

const selectedCount = () => screen.queryByRole('heading', { name: /selected$/ });

const showBar = () => screen.getByRole('toolbar', { name: 'Selected shows and episodes' });
const bookBar = () => screen.getByRole('toolbar', { name: 'Selected books and chapters' });

const hold = async (user: ReturnType<typeof userEvent.setup>, target: HTMLElement) => {
  await user.pointer({ keys: '[TouchA>]', target });
  await act(() => new Promise((resolve) => setTimeout(resolve, HOLD_MS + 50)));
  await user.pointer({ keys: '[/TouchA]', target });
};

describe('Picking shows, episodes, books and chapters', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.podcastSubscriptions.mockResolvedValue(
      ok([show('p1', 'The Example Show'), show('p2', 'Another Show')]),
    );
    commands.podcastLatest.mockResolvedValue(
      ok([episode('e1', 'Episode One'), episode('e2', 'Episode Two', 'p2')]),
    );
  });

  it('acts on shows and new episodes picked together in Library → Audio → Podcasts', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?view=podcasts');
    const view = await screen.findByTestId('podcasts-view');
    await within(view).findByRole('checkbox', { name: 'Select Another Show' });

    // The circle sits on the show's art, its ⋮ beside it.
    const circle = within(view).getByRole('checkbox', { name: 'Select The Example Show' });
    expect(circle.className).toMatch(/\bleft-1\.5\b/);
    await user.click(circle);
    expect(selectedCount()).toHaveTextContent('1 selected');
    // A Ctrl-click on an episode adds it, without opening its notes.
    const two = within(view).getByRole('listitem', { name: 'Episode Two' });
    await user.keyboard('{Control>}');
    await user.click(within(two).getByText('Episode Two'));
    await user.keyboard('{/Control}');
    expect(selectedCount()).toHaveTextContent('2 selected');
    expect(two).toHaveAttribute('data-selected', 'true');

    const bar = within(view).getByRole('toolbar', { name: 'Selected shows and episodes' });
    commands.podcastQueuePicks.mockResolvedValueOnce(ok(2));
    await user.click(within(bar).getByRole('button', { name: 'Play next' }));
    await waitFor(() =>
      expect(commands.podcastQueuePicks).toHaveBeenCalledWith(
        { shows: ['p1'], episodes: ['e2'] },
        true,
      ),
    );
    expect(await screen.findByText('2 added to the queue')).toBeInTheDocument();
    await waitFor(() => expect(selectedCount()).not.toBeInTheDocument());

    // Shows alone: follow, pin, mark, download and delete downloads.
    await user.click(within(view).getByRole('checkbox', { name: 'Select The Example Show' }));
    await user.click(within(view).getByRole('checkbox', { name: 'Select Another Show' }));
    const shows = { shows: ['p1', 'p2'], episodes: [] };
    commands.podcastSetPlayedPicks.mockResolvedValueOnce(ok(3));
    await user.click(within(showBar()).getByRole('button', { name: 'Mark as played' }));
    await waitFor(() => expect(commands.podcastSetPlayedPicks).toHaveBeenCalledWith(shows, true));

    await user.click(within(view).getByRole('checkbox', { name: 'Select The Example Show' }));
    await user.click(within(view).getByRole('checkbox', { name: 'Select Another Show' }));
    commands.podcastDownloadPicks.mockResolvedValueOnce(ok(3));
    await user.click(within(showBar()).getByRole('button', { name: 'Download' }));
    await waitFor(() => expect(commands.podcastDownloadPicks).toHaveBeenCalledWith(shows));
    expect(await screen.findByText('3 downloads queued')).toBeInTheDocument();

    await user.click(within(view).getByRole('checkbox', { name: 'Select Another Show' }));
    await user.click(within(showBar()).getByRole('button', { name: 'Delete downloads' }));
    commands.podcastRemoveDownloadsPicks.mockResolvedValueOnce(ok(1));
    await user.click(within(showBar()).getByRole('button', { name: 'Delete' }));
    await waitFor(() =>
      expect(commands.podcastRemoveDownloadsPicks).toHaveBeenCalledWith({
        shows: ['p2'],
        episodes: [],
      }),
    );
    expect(await screen.findByText('1 download deleted')).toBeInTheDocument();

    await user.click(within(view).getByRole('checkbox', { name: 'Select The Example Show' }));
    commands.podcastPinMany.mockResolvedValueOnce(ok(1));
    await user.click(within(showBar()).getByRole('button', { name: 'Pin to Quick access' }));
    await waitFor(() => expect(commands.podcastPinMany).toHaveBeenCalledWith(['p1'], true));

    await user.click(within(view).getByRole('checkbox', { name: 'Select The Example Show' }));
    await user.click(within(view).getByRole('checkbox', { name: 'Select Another Show' }));
    commands.podcastSetSubscribedMany.mockResolvedValueOnce(ok(2));
    await user.click(within(showBar()).getByRole('button', { name: 'Unfollow' }));
    await waitFor(() =>
      expect(commands.podcastSetSubscribedMany).toHaveBeenCalledWith(['p1', 'p2'], false),
    );
    expect(await screen.findByText('Unfollowed 2 shows')).toBeInTheDocument();
  });

  it('picks a show from its ⋮', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?view=podcasts');
    const view = await screen.findByTestId('podcasts-view');
    await user.click(await within(view).findByRole('button', { name: 'Options for Another Show' }));
    await user.click(within(screen.getByRole('menu')).getByRole('menuitem', { name: 'Select' }));
    await waitFor(() => expect(selectedCount()).toHaveTextContent('1 selected'));
    expect(within(view).getByRole('checkbox', { name: 'Select Another Show' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await user.keyboard('{Escape}');
    await waitFor(() => expect(selectedCount()).not.toBeInTheDocument());
  });

  it('picks episodes on a show’s page with a hold and Shift', async () => {
    const user = userEvent.setup();
    commands.podcastGet.mockResolvedValue(ok(SHOW));
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');
    const one = within(page).getByRole('listitem', { name: 'Episode One' });
    const two = within(page).getByRole('listitem', { name: 'Episode Two' });
    await hold(user, within(one).getByText('Episode One'));
    expect(selectedCount()).toHaveTextContent('1 selected');
    expect(within(one).getByRole('checkbox', { name: 'Select Episode One' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await user.keyboard('{Shift>}');
    await user.click(within(two).getByText('Episode Two'));
    await user.keyboard('{/Shift}');
    expect(selectedCount()).toHaveTextContent('2 selected');

    const bar = within(page).getByRole('toolbar', { name: 'Selected shows and episodes' });
    expect(within(bar).queryByRole('button', { name: 'Unfollow' })).not.toBeInTheDocument();
    commands.podcastQueuePicks.mockResolvedValueOnce(ok(2));
    await user.click(within(bar).getByRole('button', { name: 'Add to queue' }));
    await waitFor(() =>
      expect(commands.podcastQueuePicks).toHaveBeenCalledWith(
        { shows: [], episodes: ['e1', 'e2'] },
        false,
      ),
    );
    await waitFor(() => expect(selectedCount()).not.toBeInTheDocument());
    // Picking over, a tap opens the episode's notes again.
    expect(within(one).queryByRole('checkbox')).not.toBeInTheDocument();
  });

  it('acts on books in Library → Audio → Audiobooks', async () => {
    const user = userEvent.setup();
    commands.audiobooks.mockResolvedValue(ok([summary('b1', 'Emma'), summary('b2', 'Persuasion')]));
    await AppShellWrapper.mount('/library?view=audiobooks');
    await screen.findByText('Persuasion');

    await user.click(screen.getByRole('checkbox', { name: 'Select Emma' }));
    const bar = screen.getByRole('toolbar', { name: 'Selected books and chapters' });
    commands.podcastSetSubscribedMany.mockResolvedValueOnce(ok(1));
    await user.click(within(bar).getByRole('button', { name: 'Remove from library' }));
    await waitFor(() =>
      expect(commands.podcastSetSubscribedMany).toHaveBeenCalledWith(['b1'], false),
    );
    expect(await screen.findByText('1 book taken out of the library')).toBeInTheDocument();

    await user.click(screen.getByRole('checkbox', { name: 'Select Persuasion' }));
    commands.podcastDownloadPicks.mockResolvedValueOnce(ok(3));
    await user.click(within(bookBar()).getByRole('button', { name: 'Download' }));
    await waitFor(() =>
      expect(commands.podcastDownloadPicks).toHaveBeenCalledWith({
        shows: ['b2'],
        episodes: [],
      }),
    );
  });

  it('picks chapters on a book’s page, where a tap then picks rather than plays', async () => {
    const user = userEvent.setup();
    commands.audiobook.mockResolvedValue(ok(BOOK));
    await AppShellWrapper.mount('/audiobook?id=b1');
    const page = await screen.findByTestId('audiobook-page');
    const second = within(page).getByRole('listitem', { name: 'Chapter 2' });
    await hold(user, within(second).getByText('Chapter 2'));
    expect(selectedCount()).toHaveTextContent('1 selected');
    // Picking, a tap on a chapter picks it rather than playing it.
    await user.click(
      within(within(page).getByRole('listitem', { name: 'Chapter 1' })).getByText('Chapter 1'),
    );
    expect(selectedCount()).toHaveTextContent('2 selected');
    expect(commands.audiobookPlay).not.toHaveBeenCalled();

    commands.podcastSetPlayedPicks.mockResolvedValueOnce(ok(2));
    await user.click(
      within(within(page).getByRole('toolbar', { name: 'Selected books and chapters' })).getByRole(
        'button',
        { name: 'Mark as finished' },
      ),
    );
    await waitFor(() =>
      expect(commands.podcastSetPlayedPicks).toHaveBeenCalledWith(
        { shows: [], episodes: ['c1', 'c2'] },
        true,
      ),
    );
  });
});
