import type {
  PlaylistSummary,
  PodcastSummary,
  SavedTrack,
  Settings,
  Website,
} from '@raygun/bindings';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { HOLD_MS } from '../select/scopeContext';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';
import { sharedActions } from './allItems';
import type { AllItem } from './allItems';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const playlist = (id: string, name: string): PlaylistSummary => ({
  id,
  name,
  itemCount: 1,
  artworkUrls: [],
  coverUrl: null,
});

const SHOW: PodcastSummary = {
  id: 's1',
  feedUrl: 'https://example.com/feed.xml',
  title: 'Science Hour',
  author: 'Ann',
  artworkUrl: null,
  episodes: 3,
  unplayed: 1,
  latestPublishedAt: null,
};

const FRIEREN = libraryEntry({
  titleId: 'm1',
  sourceId: 'dev.example.comics/comics',
  remoteId: 'frieren',
  title: 'Frieren',
});

const SITE: Website = {
  id: 'w1',
  name: 'Wikipedia',
  url: 'https://wikipedia.org',
  host: 'wikipedia.org',
  icon: null,
  lastUrl: null,
  blockAds: true,
};

const song = (title: string): SavedTrack => ({
  trackKey: `artist|${title}`,
  title,
  artist: 'Artist',
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
  local: false,
  addedAt: null,
  playCount: 0,
});

const mountAll = async () => {
  await AppShellWrapper.mount('/library?media=all');
  const all = await screen.findByTestId('library-all');
  await within(all).findByText('Frieren');
  await within(all).findByText('Wikipedia');
  return all;
};

/** The tile of the item named `name`. */
const tile = (all: HTMLElement, name: string) =>
  within(all)
    .getAllByTestId('all-item')
    .find((element) => within(element).queryByText(name))!;

/** Ctrl-clicks (or, while picking, taps) the item named `name`. */
const pick = async (user: ReturnType<typeof userEvent.setup>, all: HTMLElement, name: string) => {
  await user.keyboard('{Control>}');
  await user.click(within(tile(all, name)).getByText(name));
  await user.keyboard('{/Control}');
};

const bar = () => screen.getByRole('toolbar', { name: 'Selected items' });

describe('Library → All', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.playlistsList.mockResolvedValue(
      ok([playlist('p1', 'Road trip'), playlist('p2', 'Focus')]),
    );
    commands.podcastSubscriptions.mockResolvedValue(ok([SHOW]));
    commands.mangaLibrary.mockResolvedValue(ok([FRIEREN]));
    commands.websiteList.mockResolvedValue(ok([SITE]));
  });

  it('opens the first tab for an address naming a tab it has not', async () => {
    await AppShellWrapper.mount('/library?media=audio');
    const all = await screen.findByTestId('library-all');
    expect(await within(all).findByText('Frieren')).toBeInTheDocument();
  });

  it('picks items like other lists, with what the picked kinds share', async () => {
    const user = userEvent.setup();
    const all = await mountAll();
    expect(screen.queryByRole('toolbar', { name: 'Selected items' })).not.toBeInTheDocument();

    await pick(user, all, 'Road trip');
    expect(tile(all, 'Road trip')).toHaveAttribute('data-selected', 'true');
    expect(screen.getByTestId('selection-bar')).toHaveTextContent('1 selected');
    // The page stayed: the click picked rather than opening the playlist.
    expect(screen.getByTestId('library-all')).toBeInTheDocument();
    const names = () =>
      within(bar())
        .getAllByRole('button')
        .map((button) => button.getAttribute('aria-label'));
    expect(names()).toEqual([
      'Play next',
      'Add to queue',
      'Download',
      'Add to playlist',
      'Pin to Quick access',
      'Remove from library',
    ]);

    // While picking, a tap adds: two playlists merge.
    await user.click(within(tile(all, 'Focus')).getByText('Focus'));
    expect(screen.getByTestId('selection-bar')).toHaveTextContent('2 selected');
    expect(within(bar()).getByRole('button', { name: 'Merge playlists' })).toBeInTheDocument();

    // A show joins: still played next or last, no longer added to a playlist.
    await user.click(within(tile(all, 'Science Hour')).getByText('Science Hour'));
    expect(names()).toEqual([
      'Play next',
      'Add to queue',
      'Download',
      'Pin to Quick access',
      'Remove from library',
    ]);

    // A comic joins: nothing to queue, but downloads still.
    await user.click(within(tile(all, 'Frieren')).getByText('Frieren'));
    expect(names()).toEqual(['Download', 'Pin to Quick access', 'Remove from library']);

    // Back to none: a tap opens again.
    await user.click(screen.getByRole('button', { name: 'Stop selecting' }));
    expect(screen.queryByTestId('selection-bar')).not.toBeInTheDocument();
  });

  it('plays the picked playlists and shows next, songs first', async () => {
    const user = userEvent.setup();
    commands.playlistGet.mockImplementation(async (id: string) =>
      ok({
        playlist: playlist(id, id),
        items: [{ id: `${id}-1`, track: song(`${id} song`) }],
      }),
    );
    commands.playbackQueueTracks.mockResolvedValue(ok(null));
    commands.podcastQueuePicks.mockResolvedValue(ok(3));
    const all = await mountAll();
    await pick(user, all, 'Road trip');
    await user.click(within(tile(all, 'Focus')).getByText('Focus'));
    await user.click(within(tile(all, 'Science Hour')).getByText('Science Hour'));

    await user.click(within(bar()).getByRole('button', { name: 'Play next' }));
    await waitFor(() =>
      expect(commands.playbackQueueTracks).toHaveBeenCalledWith(
        [
          expect.objectContaining({ title: 'p1 song' }),
          expect.objectContaining({ title: 'p2 song' }),
        ],
        true,
      ),
    );
    expect(commands.podcastQueuePicks).toHaveBeenCalledWith({ shows: ['s1'], episodes: [] }, true);
    // Shows go in first, so the songs (put next after them) still play first.
    expect(commands.podcastQueuePicks.mock.invocationCallOrder[0]).toBeLessThan(
      commands.playbackQueueTracks.mock.invocationCallOrder[0]!,
    );
    expect(await screen.findByText('5 added to the queue')).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByTestId('selection-bar')).not.toBeInTheDocument());
  });

  it('takes a mix out of the library after saying what goes with it', async () => {
    const user = userEvent.setup();
    const all = await mountAll();
    await pick(user, all, 'Road trip');
    await user.click(within(tile(all, 'Frieren')).getByText('Frieren'));
    await user.click(within(tile(all, 'Wikipedia')).getByText('Wikipedia'));
    // Picking, a site's tap picks rather than opening it.
    expect(commands.websiteOpen).not.toHaveBeenCalled();

    await user.click(within(bar()).getByRole('button', { name: 'Remove from library' }));
    const sheet = await screen.findByRole('dialog', { name: 'Remove from library' });
    expect(sheet).toHaveTextContent('Remove 3 items from your library?');
    expect(sheet).toHaveTextContent('The playlist is deleted');
    expect(sheet).toHaveTextContent('The website leaves your list');
    await user.click(
      within(sheet).getByRole('checkbox', {
        name: 'Also delete their downloaded chapters and episodes',
      }),
    );
    await user.click(within(sheet).getByRole('button', { name: 'Remove' }));

    await waitFor(() => expect(commands.playlistDelete).toHaveBeenCalledWith('p1'));
    expect(commands.libraryRemove).toHaveBeenCalledWith('manga', ['m1'], true, true);
    expect(commands.websiteRemove).toHaveBeenCalledWith('w1');
    expect(commands.podcastSetSubscribedMany).not.toHaveBeenCalled();
    expect(await screen.findByText('3 left your library')).toBeInTheDocument();
  });

  it('pins and unpins the picked items in quick access at once', async () => {
    const user = userEvent.setup();
    const all = await mountAll();
    await pick(user, all, 'Science Hour');
    await user.click(within(tile(all, 'Frieren')).getByText('Frieren'));
    await user.click(within(bar()).getByRole('button', { name: 'Pin to Quick access' }));

    await waitFor(async () => {
      const saved = (await commands.settingsGet()) as { data: Settings };
      expect(saved.data.rail?.pins?.map((pin) => [pin.kind, pin.search])).toEqual([
        ['podcast', { id: 's1' }],
        ['manga', { source: 'dev.example.comics/comics', id: 'frieren' }],
      ]);
    });
    expect(await screen.findByText('2 pinned to Quick access')).toBeInTheDocument();

    await pick(user, all, 'Science Hour');
    await user.click(within(tile(all, 'Frieren')).getByText('Frieren'));
    await user.click(within(bar()).getByRole('button', { name: 'Unpin from Quick access' }));
    await waitFor(async () => {
      const saved = (await commands.settingsGet()) as { data: Settings };
      expect(saved.data.rail?.pins).toEqual([]);
    });
  });

  it('asks how much of the picked titles to download', async () => {
    const user = userEvent.setup();
    commands.libraryDownloadNext.mockResolvedValue(ok(4));
    commands.podcastDownloadPicks.mockResolvedValue(ok(2));
    const all = await mountAll();
    await pick(user, all, 'Frieren');
    await user.click(within(tile(all, 'Science Hour')).getByText('Science Hour'));
    // A comic and a show: finished or not, in words for both.
    expect(within(bar()).getByRole('button', { name: 'Mark as finished' })).toBeInTheDocument();

    await user.click(within(bar()).getByRole('button', { name: 'Download' }));
    const sheet = await screen.findByRole('dialog', { name: 'Download' });
    await user.click(within(sheet).getByRole('button', { name: 'Next 5 chapters' }));
    await waitFor(() =>
      expect(commands.libraryDownloadNext).toHaveBeenCalledWith('manga', ['m1'], 5),
    );
    expect(commands.podcastDownloadPicks).toHaveBeenCalledWith({ shows: ['s1'], episodes: [] });
    expect(await screen.findByText('6 downloads queued')).toBeInTheDocument();
  });

  it("opens an item's own menu on a right click or a hold, with Select", async () => {
    const user = userEvent.setup();
    const all = await mountAll();

    await user.pointer({
      keys: '[MouseRight]',
      target: within(tile(all, 'Frieren')).getByText('Frieren'),
    });
    let menu = await screen.findByTestId('context-menu');
    expect(within(menu).getByRole('menuitem', { name: 'Remove from library' })).toBeInTheDocument();
    // The right click opened the menu and picked nothing.
    expect(screen.queryByTestId('selection-bar')).not.toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Select' }));
    expect(await screen.findByTestId('selection-bar')).toHaveTextContent('1 selected');
    expect(tile(all, 'Frieren')).toHaveAttribute('data-selected', 'true');

    // A finger held on a show opens the show's menu.
    const target = within(tile(all, 'Science Hour')).getByText('Science Hour');
    await user.pointer({ keys: '[TouchA>]', target });
    await act(() => new Promise((resolve) => setTimeout(resolve, HOLD_MS + 50)));
    await user.pointer({ keys: '[/TouchA]', target });
    menu = await screen.findByTestId('context-menu');
    expect(within(menu).getByRole('menuitem', { name: 'Unfollow' })).toBeInTheDocument();
    expect(within(menu).getByRole('menuitem', { name: 'Select' })).toBeInTheDocument();
    // The hold neither picked it nor opened its page.
    expect(tile(all, 'Science Hour')).not.toHaveAttribute('data-selected');
    expect(screen.getByTestId('library-all')).toBeInTheDocument();
  });

  it("opens a block's menu on a hold, and a site's on a right click", async () => {
    const user = userEvent.setup();
    const all = await mountAll();
    const favorites = within(all).getByRole('link', { name: /Favorites/ });
    await user.pointer({ keys: '[TouchA>]', target: favorites });
    await act(() => new Promise((resolve) => setTimeout(resolve, HOLD_MS + 50)));
    await user.pointer({ keys: '[/TouchA]', target: favorites });
    let menu = await screen.findByTestId('context-menu');
    expect(within(menu).getByRole('menuitem', { name: 'Shuffle' })).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByTestId('context-menu')).not.toBeInTheDocument());

    await user.pointer({
      keys: '[MouseRight]',
      target: within(tile(all, 'Wikipedia')).getByText('Wikipedia'),
    });
    menu = await screen.findByTestId('context-menu');
    expect(within(menu).getByRole('menuitem', { name: 'Edit website' })).toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Open' }));
    await waitFor(() => expect(commands.websiteOpen).toHaveBeenCalledWith('w1'));
  });
});

describe("Library → All's shared actions", () => {
  const show: AllItem = { kind: 'podcast', show: SHOW };
  const comic: AllItem = { kind: 'title', medium: 'manga', entry: FRIEREN };
  const video: AllItem = { kind: 'title', medium: 'video', entry: { ...FRIEREN, titleId: 'v1' } };
  const local: AllItem = { kind: 'title', medium: 'novel', entry: { ...FRIEREN, local: true } };
  const list: AllItem = { kind: 'playlist', playlist: playlist('p1', 'Road trip') };
  const site: AllItem = { kind: 'website', site: SITE };

  it('says finished or not in the words of the kinds picked', () => {
    expect(sharedActions([comic]).done?.done).toBe('library.select.markRead');
    expect(sharedActions([video]).done?.done).toBe('library.select.markWatched');
    expect(sharedActions([show]).done?.done).toBe('podcasts.markPlayed');
    expect(sharedActions([comic, video, show]).done?.done).toBe('library.all.markDone');
    expect(sharedActions([comic, list]).done).toBeNull();
  });

  it('downloads only what can be, and favorites only titles', () => {
    expect(sharedActions([comic, show, list]).download).toBe(true);
    expect(sharedActions([comic, local]).download).toBe(false);
    expect(sharedActions([comic, site]).download).toBe(false);
    expect(sharedActions([comic, video]).favorite).toBe(true);
    expect(sharedActions([comic, show]).favorite).toBe(false);
    expect(sharedActions([list]).playlists).toBe(true);
    expect(sharedActions([list, show]).playlists).toBe(false);
  });
});
