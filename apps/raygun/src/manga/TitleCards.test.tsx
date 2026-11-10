import type { VideoTitleView } from '@raygun/bindings';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { resetBackLayers } from '../shell/backLayers';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SOURCE = {
  id: 'dev.raygun.internet-archive/films',
  name: 'Archive Films',
  extensionId: 'ia',
  enabled: true,
};

const episode = (id: string, number: number, downloaded = false) => ({
  id,
  remoteId: `serial/${number}`,
  name: `Chapter ${number}`,
  number,
  season: 1,
  date: null,
  positionMs: 0,
  durationMs: null,
  watched: false,
  downloaded,
});

const SERIES: VideoTitleView = {
  continueEpisodeId: 'u1',
  id: 'title-1',
  sourceId: SOURCE.id,
  sourceName: 'Archive Films',
  remoteId: 'serial',
  title: 'Flash Gordon',
  coverUrl: null,
  mediaType: 'series',
  year: 1936,
  description: null,
  genres: [],
  cast: [],
  directors: [],
  status: 'completed',
  url: 'https://archive.example/serial',
  inLibrary: false,
  hiddenNsfw: false,
  episodes: [episode('u1', 1), episode('u2', 2, true), episode('u3', 3)],
};

const mountSource = async () => {
  commands.videoBrowse.mockResolvedValue(
    ok({
      items: [
        {
          sourceId: SOURCE.id,
          id: 'serial',
          title: 'Flash Gordon',
          coverUrl: null,
          inLibrary: false,
        },
      ],
      hasNextPage: false,
    }),
  );
  commands.videoTitle.mockResolvedValue(ok(SERIES));
  await AppShellWrapper.mount(`/watch/source?id=${encodeURIComponent(SOURCE.id)}`);
  return screen.findByRole('link', { name: 'Flash Gordon' });
};

const menuItems = () =>
  within(screen.getByRole('menu'))
    .getAllByRole('menuitem')
    .map((item) => item.textContent);

describe('Video cards', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    resetBackLayers(false);
    commands.videoSources.mockResolvedValue(ok([SOURCE]));
  });

  it('plays from the play button and opens the page from the rest of the card', async () => {
    const user = userEvent.setup();
    const card = await mountSource();

    await user.click(within(card).getByRole('button', { name: 'Play Flash Gordon' }));
    await waitFor(() => expect(commands.videoSourceWatch).toHaveBeenCalledWith('title-1', null));
    expect(screen.queryByTestId('video-title-page')).not.toBeInTheDocument();

    await user.click(card);
    expect(await screen.findByTestId('video-title-page')).toBeInTheDocument();
  });

  it('opens the full menu from ⋮, with working items, and none from a right click', async () => {
    const user = userEvent.setup();
    const card = await mountSource();
    const more = () => within(card).getByRole('button', { name: 'Options for Flash Gordon' });

    // A right click (a long press) opens nothing.
    fireEvent.contextMenu(card, { clientX: 40, clientY: 40 });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();

    await user.click(more());
    expect(menuItems()).toEqual(
      expect.arrayContaining([
        'Play',
        'Open page',
        'Add to library',
        'Mark as watched',
        'Find in other sources',
        'Download next episode',
        'Download all episodes',
        'Export…',
        'Copy link',
        'Copy title',
        'Not interested',
      ]),
    );
    await user.click(screen.getByRole('menuitem', { name: 'Add to library' }));
    await waitFor(() => expect(commands.videoSetInLibrary).toHaveBeenCalledWith('title-1', true));

    // ⋮ opens the same menu without opening the page.
    commands.videoDownloadEpisodes.mockResolvedValue(ok(2));
    await user.click(more());
    expect(screen.queryByTestId('video-title-page')).not.toBeInTheDocument();
    await user.click(screen.getByRole('menuitem', { name: 'Download all episodes' }));
    // Episodes already saved are skipped; the others are queued to find their own links.
    await waitFor(() =>
      expect(commands.videoDownloadEpisodes).toHaveBeenCalledWith(
        'title:title-1',
        expect.not.arrayContaining(['serial/2']),
      ),
    );
    expect(commands.videoSourceDownload).not.toHaveBeenCalled();

    await user.click(more());
    await user.click(screen.getByRole('menuitem', { name: 'Export…' }));
    expect(await screen.findByRole('dialog', { name: 'Export' })).toBeInTheDocument();
  });

  it('shows the page chosen with Open on a phone, after the menu leaves history', async () => {
    // Phones: each open layer has a history entry of its own (shell/backLayers).
    resetBackLayers(true);
    const user = userEvent.setup();
    const card = await mountSource();
    const before = window.history.length;

    await user.click(within(card).getByRole('button', { name: 'Options for Flash Gordon' }));
    // The menu's entry is on top of history.
    await waitFor(() => expect(window.history.length).toBe(before + 1));

    await user.click(screen.getByRole('menuitem', { name: 'Open page' }));
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    expect(await screen.findByTestId('video-title-page')).toBeInTheDocument();
    // It stays: the menu's entry went first, so dropping it can't take the page away.
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(screen.getByTestId('video-title-page')).toBeInTheDocument();
    resetBackLayers(false);
  });

  it('gives library titles ⋮ with the full menu, the source as a tag apart from the authors', async () => {
    const user = userEvent.setup();
    commands.videoLibrary.mockResolvedValue(
      ok([
        libraryEntry({
          titleId: 'title-1',
          sourceId: SOURCE.id,
          remoteId: 'serial',
          title: 'Flash Gordon',
          sourceName: 'Archive Films',
          authors: ['Universal'],
        }),
      ]),
    );
    await AppShellWrapper.mount('/library?media=video');
    await user.click(await screen.findByRole('button', { name: 'Options for Flash Gordon' }));
    expect(menuItems()).toEqual(
      expect.arrayContaining(['Select', 'Play', 'Open page', 'Remove from library']),
    );
    await user.click(screen.getByRole('menuitem', { name: 'Mark as watched' }));
    await waitFor(() =>
      expect(commands.libraryMarkRead).toHaveBeenCalledWith('video', ['title-1'], true),
    );
  });

  it('opens a Continue watching title from the card and resumes from its play button', async () => {
    const user = userEvent.setup();
    commands.videoContinueWatching.mockResolvedValue(
      ok([
        {
          key: 'stremio:series:tt1:1:2',
          title: 'Breaking Bad',
          subtitle: 'S1 E2',
          poster: null,
          positionMs: 60_000,
          durationMs: 1_000_000,
          updatedAt: 1,
          watched: false,
          sourceId: 'stremio',
          remoteId: 'series:tt1',
        },
      ]),
    );
    commands.videoResume.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/library?media=video');
    const row = await screen.findByRole('listitem', { name: 'Breaking Bad' });
    await user.click(within(row).getByRole('button', { name: 'Resume Breaking Bad' }));
    await waitFor(() =>
      expect(commands.videoResume).toHaveBeenCalledWith('stremio:series:tt1:1:2'),
    );
    expect(within(row).getByTestId('continue-card')).toHaveAttribute(
      'href',
      expect.stringContaining('/watch/title'),
    );
    await user.click(within(row).getByRole('button', { name: 'Options for Breaking Bad' }));
    expect(menuItems()).toEqual(
      expect.arrayContaining(['Resume', 'Open page', 'Remove from Continue watching']),
    );
  });
});
