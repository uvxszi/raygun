import type { PodcastEpisode, PodcastView } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { sourceExtension } from '../test/sourceExtension';

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
  number: null,
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
  order: { sort: 'date', descending: true, filters: [] },
  customOrder: false,
  episodeCount: 2,
  downloaded: 0,
  speed: null,
};

describe('Podcasts', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('points podcast search to the extensions when no directory is installed', async () => {
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=podcasts');
    const search = await screen.findByTestId('podcast-search');
    await vi.waitFor(() =>
      expect(within(search).getByTestId('no-sources')).toHaveTextContent(
        'No podcast directories yet.',
      ),
    );
    expect(within(search).getByRole('link', { name: 'Find extensions' })).toBeInTheDocument();
    expect(within(search).queryByTestId('source-issue')).not.toBeInTheDocument();
  });

  it('keeps only followed shows in the library and finds new ones in Browse', async () => {
    const user = userEvent.setup();
    commands.extensionsList.mockResolvedValue(ok([sourceExtension(['podcast.directory'])]));
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
    await user.click(await within(search).findByRole('button', { name: /^The Example Show/ }));

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

    await user.click(within(two).getByRole('button', { name: 'Resume Episode Two' }));
    expect(commands.podcastPlay).toHaveBeenCalledWith('e2');
    await user.click(within(one).getByRole('button', { name: 'Mark as played' }));
    expect(commands.podcastSetPlayed).toHaveBeenCalledWith('e1', true);
    await user.click(within(one).getByRole('button', { name: 'Download episode' }));
    expect(commands.podcastDownload).toHaveBeenCalledWith('e1');

    await user.click(within(one).getByRole('button', { name: /^Episode One/ }));
    expect(within(one).getByText('All about one.')).toBeInTheDocument();
  });

  it('offers an episode’s options from its ⋮', async () => {
    const user = userEvent.setup();
    commands.podcastGet.mockResolvedValue(ok(SHOW));
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');
    const two = within(page).getByRole('listitem', { name: 'Episode Two' });

    await user.click(within(two).getByRole('button', { name: 'Options for Episode Two' }));
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: 'Resume' })).toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Play next' }));
    expect(commands.podcastQueue).toHaveBeenCalledWith('e2', true);

    await user.click(within(two).getByRole('button', { name: 'Options for Episode Two' }));
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Add to queue' }),
    );
    expect(commands.podcastQueue).toHaveBeenLastCalledWith('e2', false);

    await user.click(within(two).getByRole('button', { name: 'Options for Episode Two' }));
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

  it('asks before deleting downloads, one or all of them', async () => {
    const user = userEvent.setup();
    const done = { state: 'done' as const, receivedBytes: 10, totalBytes: 10, error: null };
    commands.podcastGet.mockResolvedValue(
      ok({
        ...SHOW,
        episodes: [
          episode({ download: done }),
          episode({ id: 'e2', title: 'Episode Two', download: done }),
        ],
        downloaded: 2,
      }),
    );
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');
    const one = await within(page).findByRole('listitem', { name: 'Episode One' });

    // The row's button: the first tap says what it does, the second does it.
    await user.click(within(one).getByRole('button', { name: 'Delete download' }));
    expect(commands.podcastRemoveDownload).not.toHaveBeenCalled();
    await user.click(within(one).getByRole('button', { name: 'Delete' }));
    expect(commands.podcastRemoveDownload).toHaveBeenCalledWith('e1');

    // The episode's menu asks in its place.
    const two = within(page).getByRole('listitem', { name: 'Episode Two' });
    await user.click(within(two).getByRole('button', { name: 'Options for Episode Two' }));
    const menu = await screen.findByRole('menu');
    await user.click(within(menu).getByRole('menuitem', { name: 'Delete download' }));
    expect(commands.podcastRemoveDownload).not.toHaveBeenCalledWith('e2');
    const question = within(menu).getByRole('alertdialog', {
      name: 'Delete the download of “Episode Two”?',
    });
    await user.click(within(question).getByRole('button', { name: 'Delete download' }));
    expect(commands.podcastRemoveDownload).toHaveBeenCalledWith('e2');

    // The show's ⋮ says how many go.
    await user.click(within(page).getByRole('button', { name: 'More options' }));
    await user.click(screen.getByRole('menuitem', { name: 'Delete downloads' }));
    expect(commands.podcastRemoveDownloads).not.toHaveBeenCalled();
    await user.click(screen.getByRole('menuitem', { name: 'Delete 2 downloads?' }));
    expect(commands.podcastRemoveDownloads).toHaveBeenCalledWith('p1');
  });

  it('sorts and filters a show’s episodes its own way, kept in Rust', async () => {
    const user = userEvent.setup();
    commands.podcastGet.mockResolvedValue(ok(SHOW));
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');
    await within(page).findByRole('listitem', { name: 'Episode One' });

    await user.click(screen.getByRole('button', { name: 'Sort and filter' }));
    const sheet = await screen.findByRole('dialog', { name: 'Sort and filter' });
    expect(within(sheet).getByRole('button', { name: /Release date/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await user.click(within(sheet).getByRole('button', { name: /Release date/ }));
    expect(commands.podcastSetEpisodeOrder).toHaveBeenCalledWith('p1', {
      sort: 'date',
      descending: false,
      filters: [],
    });
    await user.click(within(sheet).getByRole('checkbox', { name: 'Unplayed' }));
    expect(commands.podcastSetEpisodeOrder).toHaveBeenLastCalledWith('p1', {
      sort: 'date',
      descending: true,
      filters: ['unplayed'],
    });

    // Rust answers with the episodes it kept and how many there are in all.
    commands.podcastGet.mockResolvedValue(
      ok({
        ...SHOW,
        episodes: [SHOW.episodes[1]!],
        order: { sort: 'date', descending: true, filters: ['started'] },
        customOrder: true,
      }),
    );
    emitEvent('podcastsChangedEvent', 'p1');
    expect(
      await within(page).findByRole('heading', { name: '1 of 2 episodes' }),
    ).toBeInTheDocument();
    expect(within(page).queryByRole('listitem', { name: 'Episode One' })).not.toBeInTheDocument();
  });

  it('keeps a speed for the show, marks every episode, and opens links in its notes', async () => {
    const user = userEvent.setup();
    commands.podcastGet.mockResolvedValue(
      ok({ ...SHOW, description: 'Notes and more at https://example.com/show.' }),
    );
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');

    await user.click(within(page).getByRole('button', { name: 'Speed for this show' }));
    const speeds = await screen.findByRole('radiogroup', { name: 'Playback speed' });
    await user.click(within(speeds).getByRole('radio', { name: '1.5×' }));
    expect(commands.podcastSetSpeed).toHaveBeenCalledWith('p1', 1.5);

    await user.click(within(page).getByRole('link', { name: 'https://example.com/show' }));
    expect(commands.podcastOpenPage).toHaveBeenCalledWith('https://example.com/show');

    await user.click(
      within(screen.getByTestId('top-bar')).getByRole('button', { name: 'More options' }),
    );
    await user.click(await screen.findByRole('menuitem', { name: 'Mark all as played' }));
    expect(commands.podcastSetAllPlayed).toHaveBeenCalledWith('p1', true);
  });

  it('shows downloads waiting and under way, and stops them', async () => {
    const user = userEvent.setup();
    commands.podcastGet.mockResolvedValue(
      ok({
        ...SHOW,
        episodes: [
          episode({
            download: { state: 'downloading', receivedBytes: 25, totalBytes: 100, error: null },
          }),
          episode({
            id: 'e2',
            title: 'Episode Two',
            download: { state: 'queued', receivedBytes: 0, totalBytes: null, error: null },
          }),
        ],
      }),
    );
    await AppShellWrapper.mount('/podcast?id=p1');
    const page = await screen.findByTestId('podcast-page');
    const one = await within(page).findByRole('listitem', { name: 'Episode One' });
    expect(within(one).getByText(/Downloading 25%/)).toBeInTheDocument();
    const two = within(page).getByRole('listitem', { name: 'Episode Two' });
    expect(within(two).getByText(/Waiting to download/)).toBeInTheDocument();
    await user.click(within(two).getByRole('button', { name: 'Cancel download' }));
    expect(commands.podcastCancelDownload).toHaveBeenCalledWith('e2');
  });
});
