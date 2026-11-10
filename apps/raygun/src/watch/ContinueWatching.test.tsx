import type { ContinueWatching, DownloadState, VideoDownloadItem } from '@raygun/bindings';
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { resetBackLayers } from '../shell/backLayers';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** A Stremio series' episode, half watched. */
const EPISODE: ContinueWatching = {
  key: 'stremio:series:tt1:1:2',
  title: 'Breaking Bad',
  subtitle: 'S1 E2 · Cat',
  poster: null,
  positionMs: 60_000,
  durationMs: 1_000_000,
  updatedAt: 3,
  watched: false,
  sourceId: 'stremio',
  remoteId: 'series:tt1',
  group: 'stremio:series:tt1',
  episodeId: 'tt1:1:2',
  series: true,
};

/** A Stremio movie. */
const MOVIE: ContinueWatching = {
  key: 'stremio:movie:tt9',
  title: 'Sintel',
  subtitle: '2010',
  poster: null,
  positionMs: 30_000,
  durationMs: 900_000,
  updatedAt: 2,
  watched: false,
  sourceId: 'stremio',
  remoteId: 'movie:tt9',
  group: 'stremio:movie:tt9',
  episodeId: 'tt9',
  series: false,
};

/** A video source's stored episode. */
const SERIAL: ContinueWatching = {
  key: 'source:films:serial/2',
  title: 'Flash Gordon',
  subtitle: 'Chapter 2',
  poster: null,
  positionMs: 120_000,
  durationMs: 1_200_000,
  updatedAt: 1,
  watched: false,
  sourceId: 'films',
  remoteId: 'serial',
  group: 'title:title-1',
  episodeId: 'serial/2',
  unitId: 'u2',
  series: true,
};

/** A link played on its own: no title, no page. */
const LINK: ContinueWatching = {
  key: 'url:https://cdn.example/film.mp4',
  title: 'film.mp4',
  subtitle: null,
  poster: null,
  positionMs: 5_000,
  durationMs: 100_000,
  updatedAt: 0,
  watched: false,
};

const mountRow = async (items: ContinueWatching[]) => {
  commands.videoContinueWatching.mockResolvedValue(ok(items));
  await AppShellWrapper.mount('/library?media=video');
  return screen.findByRole('region', { name: 'Continue watching' });
};

const card = (row: HTMLElement, name: string) => within(row).getByRole('listitem', { name });

const menuItems = () =>
  within(screen.getByRole('menu'))
    .getAllByRole('menuitem')
    .map((item) => item.textContent);

const openMenu = async (user: ReturnType<typeof userEvent.setup>, row: HTMLElement, name: string) =>
  user.click(within(card(row, name)).getByRole('button', { name: `Options for ${name}` }));

const count = () =>
  within(screen.getByRole('toolbar', { name: 'Selected videos' })).getByText(/selected/);

describe('Continue watching', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    resetBackLayers(false);
  });

  it('picks cards by a long press, a tap while picking, their circle and Ctrl/Shift-clicks', async () => {
    const user = userEvent.setup();
    const row = await mountRow([EPISODE, MOVIE, SERIAL]);
    const circle = (name: string) =>
      within(card(row, name)).getByRole('checkbox', { name: `Select ${name}` });

    // A finger held on a card picks it; the click its release makes opens nothing.
    fireEvent.pointerDown(card(row, 'Breaking Bad'), {
      pointerType: 'touch',
      clientX: 5,
      clientY: 5,
    });
    await act(() => new Promise((resolve) => setTimeout(resolve, 520)));
    fireEvent.pointerUp(card(row, 'Breaking Bad'));
    fireEvent.click(within(card(row, 'Breaking Bad')).getByTestId('continue-card'));
    expect(circle('Breaking Bad')).toHaveAttribute('aria-checked', 'true');
    expect(count()).toHaveTextContent('1 selected');
    // The heading made way for the selection's bar, and ⋮ hides while picking.
    expect(within(row).queryByRole('heading', { name: 'Continue watching' })).toBeNull();
    expect(within(row).queryByRole('button', { name: 'Options for Sintel' })).toBeNull();

    // While picking, a tap (on the card or its play button) picks instead of opening or playing.
    await user.click(within(card(row, 'Flash Gordon')).getByTestId('continue-card'));
    await user.click(within(card(row, 'Sintel')).getByRole('button', { name: 'Resume Sintel' }));
    expect(count()).toHaveTextContent('3 selected');
    expect(commands.videoResume).not.toHaveBeenCalled();
    expect(screen.queryByTestId('video-title-page')).not.toBeInTheDocument();
    await user.click(circle('Sintel'));
    expect(circle('Sintel')).toHaveAttribute('aria-checked', 'false');
    expect(count()).toHaveTextContent('2 selected');

    // Escape (and Back) drops the selection.
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('toolbar', { name: 'Selected videos' })).not.toBeInTheDocument();
    expect(within(row).getByRole('heading', { name: 'Continue watching' })).toBeInTheDocument();

    // Ctrl-click picks one; Shift-click picks the range from it.
    await user.keyboard('[ControlLeft>]');
    await user.click(within(card(row, 'Breaking Bad')).getByTestId('continue-card'));
    await user.keyboard('[/ControlLeft]');
    expect(count()).toHaveTextContent('1 selected');
    await user.keyboard('[ShiftLeft>]');
    await user.click(within(card(row, 'Flash Gordon')).getByTestId('continue-card'));
    await user.keyboard('[/ShiftLeft]');
    expect(count()).toHaveTextContent('3 selected');
    expect(circle('Sintel')).toHaveAttribute('aria-checked', 'true');
    expect(screen.queryByTestId('video-title-page')).not.toBeInTheDocument();
  });

  it('removes the picked cards together after asking, saying how many', async () => {
    const user = userEvent.setup();
    const row = await mountRow([EPISODE, MOVIE, SERIAL]);

    await user.click(within(card(row, 'Sintel')).getByRole('checkbox', { name: 'Select Sintel' }));
    const bar = screen.getByRole('toolbar', { name: 'Selected videos' });
    await user.click(within(bar).getByRole('button', { name: 'Select all' }));
    expect(count()).toHaveTextContent('3 selected');
    expect(within(bar).getByRole('button', { name: 'Select none' })).toBeInTheDocument();

    // Asked first; Cancel keeps them.
    await user.click(within(bar).getByRole('button', { name: 'Remove' }));
    const ask = screen.getByRole('alertdialog', {
      name: 'Remove 3 videos from Continue watching?',
    });
    await user.click(within(ask).getByRole('button', { name: 'Cancel' }));
    expect(commands.videoRemoveMany).not.toHaveBeenCalled();

    await user.click(
      within(screen.getByRole('toolbar', { name: 'Selected videos' })).getByRole('button', {
        name: 'Remove',
      }),
    );
    commands.videoContinueWatching.mockResolvedValue(ok([]));
    await user.click(
      within(
        screen.getByRole('alertdialog', { name: 'Remove 3 videos from Continue watching?' }),
      ).getByRole('button', { name: 'Remove' }),
    );
    expect(commands.videoRemoveMany).toHaveBeenCalledWith([EPISODE.key, MOVIE.key, SERIAL.key]);
    expect(await screen.findByText('Removed 3 videos from Continue watching')).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.queryByRole('region', { name: 'Continue watching' })).not.toBeInTheDocument(),
    );
  });

  it("has no X on a card, and ⋮ with an episode's whole menu", async () => {
    const user = userEvent.setup();
    const row = await mountRow([EPISODE]);
    expect(within(card(row, 'Breaking Bad')).queryByRole('button', { name: /Remove/ })).toBeNull();

    await openMenu(user, row, 'Breaking Bad');
    expect(menuItems()).toEqual([
      'Resume',
      'Play from the start',
      'Next episode',
      'Open page',
      'Choose a source…',
      'Add to library',
      'Mark as watched',
      'Download',
      'Pin to Quick access',
      'Remove from Continue watching',
    ]);
    await user.click(screen.getByRole('menuitem', { name: 'Play from the start' }));
    await waitFor(() => expect(commands.videoPlayFromStart).toHaveBeenCalledWith(EPISODE.key));

    await openMenu(user, row, 'Breaking Bad');
    await user.click(screen.getByRole('menuitem', { name: 'Next episode' }));
    await waitFor(() => expect(commands.videoPlayNext).toHaveBeenCalledWith(EPISODE.key));
    commands.videoPlayNext.mockResolvedValue(ok(false));
    await openMenu(user, row, 'Breaking Bad');
    await user.click(screen.getByRole('menuitem', { name: 'Next episode' }));
    expect(await screen.findByText('Breaking Bad has no next episode yet')).toBeInTheDocument();

    await openMenu(user, row, 'Breaking Bad');
    await user.click(screen.getByRole('menuitem', { name: 'Download' }));
    await waitFor(() =>
      expect(commands.videoDownloadEpisodes).toHaveBeenCalledWith('stremio:series:tt1', [
        'tt1:1:2',
      ]),
    );

    await openMenu(user, row, 'Breaking Bad');
    await user.click(screen.getByRole('menuitem', { name: 'Add to library' }));
    await waitFor(() =>
      expect(commands.stremioSetInLibrary).toHaveBeenCalledWith('series', 'tt1', true),
    );

    await openMenu(user, row, 'Breaking Bad');
    await user.click(screen.getByRole('menuitem', { name: 'Choose a source…' }));
    expect(await screen.findByRole('dialog', { name: 'Streams' })).toBeInTheDocument();
    expect(commands.stremioStreams).toHaveBeenCalledWith('series', 'tt1', 'tt1:1:2');
    await user.click(screen.getByRole('button', { name: 'Close' }));

    await openMenu(user, row, 'Breaking Bad');
    commands.videoContinueWatching.mockResolvedValue(ok([]));
    await user.click(screen.getByRole('menuitem', { name: 'Mark as watched' }));
    await waitFor(() => expect(commands.videoMarkWatched).toHaveBeenCalledWith(EPISODE.key));
    expect(
      await screen.findByText('Breaking Bad · S1 E2 · Cat is marked watched'),
    ).toBeInTheDocument();
    // It leaves the row.
    await waitFor(() =>
      expect(screen.queryByRole('region', { name: 'Continue watching' })).not.toBeInTheDocument(),
    );
  });

  it('asks before removing one from its menu', async () => {
    const user = userEvent.setup();
    const row = await mountRow([EPISODE, MOVIE]);
    await openMenu(user, row, 'Sintel');
    await user.click(screen.getByRole('menuitem', { name: 'Remove from Continue watching' }));
    expect(commands.videoRemove).not.toHaveBeenCalled();
    const ask = screen.getByRole('alertdialog', {
      name: 'Remove Sintel · 2010 from Continue watching?',
    });
    await user.click(within(ask).getByRole('button', { name: 'Remove from Continue watching' }));
    expect(commands.videoRemove).toHaveBeenCalledWith(MOVIE.key);
  });

  it("gives a video source's episode its servers, and a movie or a link no next episode", async () => {
    const user = userEvent.setup();
    const row = await mountRow([SERIAL, MOVIE, LINK]);

    await openMenu(user, row, 'Flash Gordon');
    await user.click(screen.getByRole('menuitem', { name: 'Choose a source…' }));
    expect(await screen.findByRole('dialog', { name: 'Servers' })).toBeInTheDocument();
    expect(commands.videoServers).toHaveBeenCalledWith('films', 'serial/2');
    await user.click(screen.getByRole('button', { name: 'Close' }));

    await openMenu(user, row, 'Flash Gordon');
    await user.click(screen.getByRole('menuitem', { name: 'Add to library' }));
    await waitFor(() => expect(commands.videoSetInLibrary).toHaveBeenCalledWith('title-1', true));
    await openMenu(user, row, 'Flash Gordon');
    await user.click(screen.getByRole('menuitem', { name: 'Download' }));
    await waitFor(() =>
      expect(commands.videoDownloadEpisodes).toHaveBeenCalledWith('title:title-1', ['serial/2']),
    );

    await openMenu(user, row, 'Sintel');
    expect(menuItems()).not.toContain('Next episode');
    expect(menuItems()).toContain('Choose a source…');
    await user.keyboard('{Escape}');

    await openMenu(user, row, 'film.mp4');
    expect(menuItems()).toEqual([
      'Resume',
      'Play from the start',
      'Mark as watched',
      'Remove from Continue watching',
    ]);
  });

  describe('its download', () => {
    /** The episode's download, in `state`. */
    const saved = (state: DownloadState, extra: Partial<VideoDownloadItem> = {}) =>
      ({
        id: 'dl-1',
        key: EPISODE.key,
        titleId: null,
        title: EPISODE.title,
        subtitle: EPISODE.subtitle,
        poster: null,
        state,
        doneBytes: 40,
        totalBytes: 100,
        doneParts: 0,
        totalParts: null,
        error: null,
        sourceId: 'stremio',
        remoteId: 'series:tt1',
        episodeId: 'tt1:1:2',
        watched: false,
        at: 1,
        ...extra,
      }) satisfies VideoDownloadItem;

    const withDownload = (item: VideoDownloadItem) =>
      commands.videoDownloads.mockResolvedValue(ok({ paused: false, items: [item] }));

    /** The menu's lines between Mark as watched and Pin: the download's own. */
    const downloadLines = () => {
      const items = menuItems();
      return items.slice(
        items.indexOf('Mark as watched') + 1,
        items.indexOf('Pin to Quick access'),
      );
    };

    it('offers Delete download once it is downloaded, asked first', async () => {
      const user = userEvent.setup();
      withDownload(saved('done'));
      const row = await mountRow([EPISODE]);
      await waitFor(() => expect(commands.videoDownloads).toHaveBeenCalled());

      await openMenu(user, row, 'Breaking Bad');
      await waitFor(() => expect(downloadLines()).toEqual(['Delete download']));
      await user.click(screen.getByRole('menuitem', { name: 'Delete download' }));
      expect(commands.videoDownloadRemove).not.toHaveBeenCalled();
      const ask = screen.getByRole('alertdialog', {
        name: 'Delete the download of “Breaking Bad · S1 E2 · Cat”?',
      });
      await user.click(within(ask).getByRole('button', { name: 'Delete download' }));
      await waitFor(() => expect(commands.videoDownloadRemove).toHaveBeenCalledWith('dl-1'));
      expect(
        await screen.findByText('Deleted the download of Breaking Bad · S1 E2 · Cat'),
      ).toBeInTheDocument();
      expect(commands.videoDownloadEpisodes).not.toHaveBeenCalled();
    });

    it('shows a download on its way, with Cancel asked first', async () => {
      const user = userEvent.setup();
      withDownload(saved('downloading'));
      const row = await mountRow([EPISODE]);
      await waitFor(() => expect(commands.videoDownloads).toHaveBeenCalled());

      await openMenu(user, row, 'Breaking Bad');
      await waitFor(() =>
        expect(downloadLines()).toEqual(['Downloading · 40%', 'Cancel download']),
      );
      expect(screen.getByRole('menuitem', { name: 'Downloading · 40%' })).toBeDisabled();
      await user.click(screen.getByRole('menuitem', { name: 'Cancel download' }));
      const ask = screen.getByRole('alertdialog', {
        name: 'Stop downloading “Breaking Bad · S1 E2 · Cat”?',
      });
      // Asked first: backing out keeps it.
      await user.click(within(ask).getByRole('button', { name: 'Cancel' }));
      expect(commands.videoDownloadRemove).not.toHaveBeenCalled();

      await openMenu(user, row, 'Breaking Bad');
      await user.click(screen.getByRole('menuitem', { name: 'Cancel download' }));
      await user.click(
        within(
          screen.getByRole('alertdialog', {
            name: 'Stop downloading “Breaking Bad · S1 E2 · Cat”?',
          }),
        ).getByRole('button', { name: 'Cancel download' }),
      );
      await waitFor(() => expect(commands.videoDownloadRemove).toHaveBeenCalledWith('dl-1'));
      expect(
        await screen.findByText('Stopped downloading Breaking Bad · S1 E2 · Cat'),
      ).toBeInTheDocument();
    });

    it('says a queued one waits, and downloads a failed one again', async () => {
      const user = userEvent.setup();
      withDownload(saved('queued'));
      const row = await mountRow([EPISODE]);
      await waitFor(() => expect(commands.videoDownloads).toHaveBeenCalled());
      await openMenu(user, row, 'Breaking Bad');
      await waitFor(() =>
        expect(downloadLines()).toEqual(['Waiting to download', 'Cancel download']),
      );
      await user.keyboard('{Escape}');

      withDownload(saved('failed', { error: 'HTTP 404' }));
      emitEvent('videoDownloadsEvent', 'dl-1');
      await openMenu(user, row, 'Breaking Bad');
      await waitFor(() => expect(downloadLines()).toEqual(['Download again']));
      await user.click(screen.getByRole('menuitem', { name: 'Download again' }));
      await waitFor(() => expect(commands.videoDownloadRetry).toHaveBeenCalledWith('dl-1'));
      expect(commands.videoDownloadEpisodes).not.toHaveBeenCalled();
    });
  });
});
