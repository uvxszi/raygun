import type { VideoLinkView, VideoTitleView } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SOURCE = {
  id: 'dev.raygun.internet-archive/films',
  name: 'Archive Films',
  extensionId: 'ia',
};

const link = (label: string, quality: number, preferred = false): VideoLinkView => ({
  url: `https://cdn.example/${label}.mp4`,
  label,
  quality,
  headers: {},
  format: null,
  subtitles: [],
  preferred,
});

const SERIES: VideoTitleView = {
  id: 'title-1',
  sourceId: SOURCE.id,
  sourceName: 'Archive Films',
  remoteId: 'serial',
  title: 'Flash Gordon',
  coverUrl: null,
  mediaType: 'series',
  year: 1936,
  description: 'A serial.',
  genres: ['Sci-fi'],
  cast: ['Buster Crabbe'],
  directors: ['Frederick Stephani'],
  status: 'completed',
  url: null,
  inLibrary: false,
  hiddenNsfw: false,
  episodes: [
    {
      id: 'u1',
      remoteId: 'serial/1',
      name: 'Planet of Peril',
      number: 1,
      season: 1,
      date: null,
      positionMs: 0,
      durationMs: null,
      watched: true,
      downloaded: false,
    },
    {
      id: 'u2',
      remoteId: 'serial/2',
      name: 'The Tunnel of Terror',
      number: 2,
      season: 1,
      date: null,
      positionMs: 60_000,
      durationMs: 1_200_000,
      watched: false,
      downloaded: false,
    },
    {
      id: 'u3',
      remoteId: 'serial/3',
      name: 'Captured',
      number: 1,
      season: 2,
      date: null,
      positionMs: 0,
      durationMs: null,
      watched: false,
      downloaded: false,
    },
  ],
};

const FILM: VideoTitleView = {
  ...SERIES,
  id: 'title-2',
  remoteId: 'chaplin',
  title: 'Charlie Chaplin Festival',
  mediaType: 'movie',
  year: 1938,
  episodes: [
    {
      id: 'f1',
      remoteId: 'chaplin/chaplin.mpeg',
      name: 'Film',
      number: null,
      season: null,
      date: null,
      positionMs: 0,
      durationMs: null,
      watched: false,
      downloaded: false,
    },
  ],
};

describe('Video sources in Watch', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.videoSources.mockResolvedValue(ok([SOURCE]));
  });

  it('lists the source in Browse and opens a series with seasons', async () => {
    const user = userEvent.setup();
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
        hasNextPage: true,
      }),
    );
    commands.videoTitle.mockResolvedValue(ok(SERIES));
    await AppShellWrapper.mount('/browse');

    // Video sources are listed in Browse under Movies & series, with a Latest shortcut.
    const section = await screen.findByRole('region', { name: 'Movies & series' });
    expect(within(section).getByRole('link', { name: 'Latest from Archive Films' })).toBeVisible();
    await user.click(within(section).getByRole('link', { name: 'Archive Films' }));
    const source = await screen.findByTestId('manga-source-page');
    expect(commands.videoBrowse).toHaveBeenCalledWith(SOURCE.id, 'popular', null, 1);
    await user.click(await within(source).findByRole('link', { name: /Flash Gordon/ }));

    const page = await screen.findByTestId('video-title-page');
    expect(commands.videoTitle).toHaveBeenCalledWith(SOURCE.id, 'serial', false);
    expect(within(page).getByText('Series · 1936 · Archive Films')).toBeInTheDocument();
    // Season 1 opens first: it has an unwatched episode.
    expect(within(page).getByRole('tab', { name: 'Season 1' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    expect(within(page).getAllByRole('listitem', { name: /^S1 E/ })).toHaveLength(2);

    await user.click(
      within(page).getByRole('button', { name: 'Mark S1 E2 · The Tunnel of Terror watched' }),
    );
    expect(commands.videoSetEpisodeWatched).toHaveBeenCalledWith('title-1', 'u2', true);

    await user.click(within(page).getByRole('tab', { name: 'Season 2' }));
    expect(within(page).getByRole('listitem', { name: 'S2 E1 · Captured' })).toBeInTheDocument();
  });

  it('opens a link in another app or copies it', async () => {
    const user = userEvent.setup();
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    commands.videoTitle.mockResolvedValue(ok(FILM));
    commands.videoServers.mockResolvedValue(
      ok([{ id: 'fast', name: 'Fast host', links: [link('720p', 720, true)] }]),
    );
    await AppShellWrapper.mount(`/watch/video?source=${encodeURIComponent(SOURCE.id)}&id=chaplin`);
    const page = await screen.findByTestId('video-title-page');
    await user.click(within(page).getByRole('button', { name: 'Choose a source' }));
    const picker = await screen.findByRole('dialog', { name: 'Servers' });

    await user.click(await within(picker).findByRole('button', { name: 'More for 720p' }));
    await user.click(within(picker).getByRole('menuitem', { name: 'Copy link' }));
    expect(writeText).toHaveBeenCalledWith(link('720p', 720, true).url);

    await user.click(within(picker).getByRole('button', { name: 'More for 720p' }));
    await user.click(within(picker).getByRole('menuitem', { name: 'Open in another app' }));
    expect(commands.videoSourcePlay).toHaveBeenCalledWith({
      titleId: 'title-2',
      episodeId: 'f1',
      link: link('720p', 720, true),
      external: true,
    });
  });

  it('resolves servers one by one and plays the preferred link', async () => {
    const user = userEvent.setup();
    commands.videoTitle.mockResolvedValue(ok(FILM));
    commands.videoServers.mockResolvedValue(
      ok([
        { id: 'broken', name: 'Broken host', links: null },
        { id: 'slow', name: 'Slow host', links: null },
        { id: 'empty', name: 'Empty host', links: [] },
        { id: 'fast', name: 'Fast host', links: [link('720p', 720, true), link('360p', 360)] },
      ]),
    );
    let finishSlow: (value: unknown) => void = () => {};
    commands.videoLinks.mockImplementation((_source: string, server: string) =>
      server === 'broken'
        ? Promise.resolve({ status: 'error', error: 'host is down' })
        : new Promise((resolve) => {
            finishSlow = resolve;
          }),
    );
    await AppShellWrapper.mount(`/watch/video?source=${encodeURIComponent(SOURCE.id)}&id=chaplin`);
    const page = await screen.findByTestId('video-title-page');
    expect(within(page).getByText('Movie · 1938 · Archive Films')).toBeInTheDocument();

    // Play watches through a session; choosing a link is the top bar's Choose a source.
    await user.click(within(page).getByRole('button', { name: 'Play' }));
    expect(commands.videoSourceWatch).toHaveBeenCalledWith('title-2', 'chaplin/chaplin.mpeg');
    await user.click(within(page).getByRole('button', { name: 'Choose a source' }));
    const picker = await screen.findByRole('dialog', { name: 'Servers' });
    expect(commands.videoServers).toHaveBeenCalledWith(SOURCE.id, 'chaplin/chaplin.mpeg');
    expect(commands.videoLinks).toHaveBeenCalledWith(SOURCE.id, 'broken');
    expect(commands.videoLinks).toHaveBeenCalledWith(SOURCE.id, 'slow');
    expect(commands.videoLinks).not.toHaveBeenCalledWith(SOURCE.id, 'fast');

    const fast = within(picker).getByRole('listitem', { name: 'Fast host' });
    expect(fast).toHaveAttribute('data-state', 'ready');
    expect(within(picker).getByRole('listitem', { name: 'Empty host' })).toHaveAttribute(
      'data-state',
      'empty',
    );
    expect(within(picker).getByText('No videos on this server.')).toBeInTheDocument();
    const broken = within(picker).getByRole('listitem', { name: 'Broken host' });
    expect(await within(broken).findByText(/host is down/)).toBeInTheDocument();
    expect(broken).toHaveAttribute('data-state', 'failed');
    const slow = within(picker).getByRole('listitem', { name: 'Slow host' });
    expect(slow).toHaveAttribute('data-state', 'resolving');

    finishSlow(ok([link('1080p', 1080, true)]));
    expect(await within(slow).findByRole('button', { name: /^1080p/ })).toBeVisible();
    expect(slow).toHaveAttribute('data-state', 'ready');

    await user.click(within(fast).getByRole('button', { name: /720p.*Preferred/ }));
    expect(commands.videoSourcePlay).toHaveBeenCalledWith({
      titleId: 'title-2',
      episodeId: 'f1',
      link: link('720p', 720, true),
      external: false,
    });
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('turns autoplay off, sets the next-episode prompt and names the player in the app language', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/player');
    await user.click(
      await screen.findByRole('switch', { name: /Play the next episode automatically/ }),
    );
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'autoplayNext', value: false });

    await user.click(screen.getByRole('button', { name: /Next-episode prompt/ }));
    const sheet = await screen.findByRole('dialog', { name: 'Next-episode prompt' });
    await user.click(within(sheet).getByRole('radio', { name: 'At the end' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'nextPromptSeconds', value: 0 });

    expect(commands.videoSetPlayerLabels).toHaveBeenCalledWith(
      expect.objectContaining({ episodes: 'Episodes', nextIn: 'Next episode in {n} s' }),
    );
  });

  it('sets the preferred video quality', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/player');
    await user.click(await screen.findByRole('button', { name: /Preferred video quality/ }));
    const sheet = await screen.findByRole('dialog', { name: 'Preferred video quality' });
    await user.click(within(sheet).getByRole('radio', { name: 'Up to 720p' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'preferredVideoQuality',
      value: 720,
    });
  });
});
