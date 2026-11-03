import type { PodcastEpisode, PodcastView } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const episode = (overrides: Partial<PodcastEpisode> = {}): PodcastEpisode => ({
  id: 'e1',
  podcastId: 'p1',
  podcastTitle: 'The Example Show',
  title: 'Episode One',
  description: 'All about one.',
  publishedAt: Date.UTC(2026, 9, 1),
  durationMs: 3_600_000,
  positionMs: 0,
  played: false,
  artworkUrl: null,
  video: false,
  download: null,
  ...overrides,
});

const SHOW: PodcastView = {
  id: 'p1',
  feedUrl: 'https://feeds.example/show.xml',
  title: 'The Example Show',
  author: 'Ana',
  description: 'Weekly talk.',
  artworkUrl: null,
  link: null,
  subscribed: false,
  episodes: [episode(), episode({ id: 'e2', title: 'Episode Two', positionMs: 1_800_000 })],
};

describe('Podcasts', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('keeps only followed shows in the library and finds new ones in Browse', async () => {
    const user = userEvent.setup();
    commands.podcastTop.mockResolvedValue(
      ok([
        {
          title: 'Top Show',
          author: 'Big Network',
          feedUrl: 'https://feeds.example/top.xml',
          artworkUrl: null,
          description: null,
          subscribed: false,
        },
      ]),
    );
    commands.podcastSearch.mockResolvedValue(
      ok([
        {
          title: 'The Example Show',
          author: 'Ana',
          feedUrl: SHOW.feedUrl,
          artworkUrl: null,
          description: null,
          subscribed: false,
        },
      ]),
    );
    commands.podcastOpenFeed.mockResolvedValue(ok(SHOW));
    await AppShellWrapper.mount('/library?media=music');

    await user.click(await screen.findByTestId('library-view-podcasts'));
    const view = await screen.findByTestId('podcasts-view');
    // The library holds what you follow; discovering shows is Search's job.
    expect(await within(view).findByText('No podcasts yet')).toBeInTheDocument();
    expect(commands.podcastTop).not.toHaveBeenCalled();
    await user.click(within(view).getByRole('link', { name: 'Find in Browse' }));

    const browse = await screen.findByTestId('browse-page');
    const search = within(browse).getByTestId('search-page');
    expect(await within(search).findByText('Top Show')).toBeInTheDocument();
    await user.type(within(browse).getByRole('searchbox', { name: 'Search' }), 'example');
    await vi.waitFor(() => expect(commands.podcastSearch).toHaveBeenCalledWith('example'));
    await user.click(await within(search).findByRole('button', { name: /The Example Show/ }));

    expect(commands.podcastOpenFeed).toHaveBeenCalledWith(SHOW.feedUrl);
    const page = await screen.findByTestId('podcast-page');
    expect(within(page).getByRole('heading', { name: 'The Example Show' })).toBeInTheDocument();
  });

  it('subscribes, plays, marks played and downloads episodes', async () => {
    const user = userEvent.setup();
    commands.podcastGet.mockResolvedValue(ok(SHOW));
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');

    expect(within(page).getByRole('heading', { name: '2 episodes' })).toBeInTheDocument();
    const two = within(page).getByRole('listitem', { name: 'Episode Two' });
    expect(within(two).getByText(/30 min left/)).toBeInTheDocument();
    const one = within(page).getByRole('listitem', { name: 'Episode One' });
    expect(within(one).getByText(/1 h 0 min/)).toBeInTheDocument();

    await user.click(within(page).getByRole('button', { name: 'Subscribe' }));
    expect(commands.podcastSetSubscribed).toHaveBeenCalledWith('p1', true);

    await user.click(within(two).getByRole('button', { name: 'Play Episode Two' }));
    expect(commands.podcastPlay).toHaveBeenCalledWith('e2');
    await user.click(within(one).getByRole('button', { name: 'Mark as played' }));
    expect(commands.podcastSetPlayed).toHaveBeenCalledWith('e1', true);
    await user.click(within(one).getByRole('button', { name: 'Download episode' }));
    expect(commands.podcastDownload).toHaveBeenCalledWith('e1');

    await user.click(within(one).getByRole('button', { name: /^Episode One/ }));
    expect(within(one).getByText('All about one.')).toBeInTheDocument();
  });

  it('offers an episode’s options on right click', async () => {
    const user = userEvent.setup();
    commands.podcastGet.mockResolvedValue(ok(SHOW));
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');
    const two = within(page).getByRole('listitem', { name: 'Episode Two' });

    fireEvent.contextMenu(two, { clientX: 40, clientY: 40 });
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: 'Resume' })).toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Play next' }));
    expect(commands.podcastQueue).toHaveBeenCalledWith('e2', true);

    fireEvent.contextMenu(two, { clientX: 40, clientY: 40 });
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Add to queue' }),
    );
    expect(commands.podcastQueue).toHaveBeenLastCalledWith('e2', false);

    fireEvent.contextMenu(two, { clientX: 40, clientY: 40 });
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Mark as played' }),
    );
    expect(commands.podcastSetPlayed).toHaveBeenCalledWith('e2', true);
  });

  it('follows downloads reported by Rust', async () => {
    commands.podcastGet.mockResolvedValue(ok(SHOW));
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');
    await within(page).findByRole('listitem', { name: 'Episode One' });

    commands.podcastGet.mockResolvedValue(
      ok({
        ...SHOW,
        episodes: [
          episode({
            download: { state: 'done', receivedBytes: 10, totalBytes: 10, error: null },
          }),
        ],
      }),
    );
    emitEvent('podcastsChangedEvent', 'p1');
    const one = await within(page).findByRole('listitem', { name: 'Episode One' });
    expect(await within(one).findByText(/Downloaded/)).toBeInTheDocument();
    expect(within(one).getByRole('button', { name: 'Delete download' })).toBeInTheDocument();
  });
});
