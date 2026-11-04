import type { MangaCard, VideoTitleView } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const HERE = { id: 'dev.example/anime', name: 'Anime Here', extensionId: 'a', enabled: true };
const THERE = { id: 'dev.example/other', name: 'Anime There', extensionId: 'b', enabled: true };
const SITE = {
  id: 'dev.raygun.youtube/videos',
  name: 'YouTube',
  extensionId: 'youtube',
  enabled: true,
  searchPill: 'YouTube',
};

const episode = (id: string, number: number, watched: boolean) => ({
  id,
  remoteId: `overlord-2/${number}`,
  name: `Episode ${number}`,
  number,
  season: null,
  date: null,
  positionMs: 0,
  durationMs: null,
  watched,
  downloaded: false,
});

const OVERLORD_II: VideoTitleView = {
  // Rust's answer: episode 2 was skipped, episode 3 watched last.
  continueEpisodeId: 'u4',
  id: 'title-1',
  sourceId: HERE.id,
  sourceName: HERE.name,
  remoteId: 'overlord-2',
  title: 'Overlord II',
  coverUrl: null,
  mediaType: 'anime',
  year: 2018,
  description: null,
  genres: [],
  cast: [],
  directors: [],
  status: 'completed',
  url: null,
  inLibrary: false,
  hiddenNsfw: false,
  episodes: [
    episode('u1', 1, true),
    episode('u2', 2, false),
    episode('u3', 3, true),
    episode('u4', 4, false),
  ],
};

const card = (id: string, title: string): MangaCard => ({
  sourceId: THERE.id,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
});

describe('Seasons of a video title', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.videoSources.mockResolvedValue(ok([HERE, THERE, SITE]));
    commands.videoTitle.mockResolvedValue(ok(OVERLORD_II));
  });

  it('finds the same season in other sources, never another one', async () => {
    const user = userEvent.setup();
    commands.catalogGlobalSearch.mockImplementation(
      (_medium: string, _query: string, only: string[]) =>
        Promise.resolve(
          ok(
            only.includes(THERE.id)
              ? [
                  {
                    source: THERE,
                    items: [
                      card('o1', 'Overlord'),
                      card('o2', 'OVERLORD 2nd Season'),
                      card('o3', 'Overlord III'),
                      card('o4', 'Overlord II (Dub)'),
                    ],
                    hasNextPage: false,
                    error: null,
                  },
                ]
              : [],
          ),
        ),
    );
    await AppShellWrapper.mount(`/watch/video?source=${encodeURIComponent(HERE.id)}&id=overlord-2`);
    const page = await screen.findByTestId('video-title-page');
    await user.click(within(page).getByRole('button', { name: 'Find in other sources' }));
    const found = await screen.findByTestId('other-sources');
    expect(await within(found).findByText('OVERLORD 2nd Season')).toBeInTheDocument();
    expect(within(found).getByText('Overlord II (Dub)')).toBeInTheDocument();
    expect(within(found).queryByText('Overlord')).not.toBeInTheDocument();
    expect(within(found).queryByText('Overlord III')).not.toBeInTheDocument();
    // A video site's search isn't a catalogue's.
    expect(commands.catalogGlobalSearch).not.toHaveBeenCalledWith('video', 'Overlord II', [
      SITE.id,
    ]);
  });

  it('continues after the last episode watched', async () => {
    const user = userEvent.setup();
    commands.videoSourceWatch.mockResolvedValue(ok(null));
    await AppShellWrapper.mount(`/watch/video?source=${encodeURIComponent(HERE.id)}&id=overlord-2`);
    const page = await screen.findByTestId('video-title-page');
    // Episode 2 was skipped; episode 3 was watched last: 4 is next.
    await user.click(within(page).getByRole('button', { name: 'Start' }));
    expect(commands.videoSourceWatch).toHaveBeenCalledWith('title-1', 'overlord-2/4');
    // A name that only repeats the number isn't shown twice.
    expect(within(page).getByRole('listitem', { name: 'Episode 4' })).toBeInTheDocument();
  });
});
