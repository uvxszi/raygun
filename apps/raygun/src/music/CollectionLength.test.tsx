import type { AudiobookView, PodcastEpisode, SavedTrack } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import i18n from 'i18next';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { describeLength, formatHoursMinutes, summarizeLengths } from './collectionLength';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const song = (title: string, artist: string, durationMs: number | null): SavedTrack => ({
  trackKey: `${artist.toLowerCase()}|${title.toLowerCase()}`,
  title,
  artist,
  album: null,
  durationMs,
  artworkUrl: null,
  source: null,
  local: false,
  addedAt: 1_000,
  playCount: 0,
});

const chapter = (id: string, durationMs: number | null): PodcastEpisode => ({
  id,
  podcastId: 'b1',
  podcastTitle: 'Emma',
  title: `Chapter ${id}`,
  description: null,
  publishedAt: null,
  durationMs,
  positionMs: 0,
  played: false,
  artworkUrl: null,
  video: false,
  download: null,
  number: null,
});

const MINUTE = 60_000;

describe('collection lengths', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('formats hours and minutes, or minutes alone under an hour', () => {
    expect(formatHoursMinutes(157 * MINUTE, 'en')).toBe('2 hr 37 min');
    expect(formatHoursMinutes(42 * MINUTE, 'en')).toBe('42 min');
    expect(formatHoursMinutes(180 * MINUTE, 'en')).toBe('3 hr');
    expect(formatHoursMinutes(10_000, 'en')).toBe('1 min');
  });

  it('is exact when every length is known, estimated or a lower bound otherwise', () => {
    const t = i18n.getFixedT('en');
    expect(describeLength(summarizeLengths([60 * MINUTE, 97 * MINUTE]), t, 'en')).toBe(
      '2 hr 37 min',
    );
    // Three of four known, 50 min on average: about 3 h 20 min.
    expect(
      describeLength(summarizeLengths([50 * MINUTE, 50 * MINUTE, 50 * MINUTE, null]), t, 'en'),
    ).toBe('about 3 hr 20 min');
    // One of four known: at least what that one plays.
    expect(describeLength(summarizeLengths([42 * MINUTE], 4), t, 'en')).toBe('42 min+');
    expect(describeLength(summarizeLengths([null, null]), t, 'en')).toBeNull();
  });

  it('shows Favorites’ length and updates it as missing lengths arrive', async () => {
    commands.favoritesList.mockResolvedValue(
      ok([
        song('Alpha', 'Mia', 20 * MINUTE),
        song('Bravo', 'Zed', 20 * MINUTE),
        song('Charlie', 'Ola', null),
      ]),
    );
    commands.trackDurations.mockResolvedValue(ok([{ trackKey: 'ola|charlie', durationMs: null }]));
    await AppShellWrapper.mount('/library?view=favorites');
    const view = await screen.findByTestId('favorites-view');
    expect(await within(view).findByText('3 tracks · about 1 hr')).toBeInTheDocument();
    emitEvent('trackDurationsEvent', [{ trackKey: 'ola|charlie', durationMs: 37 * MINUTE }]);
    expect(await within(view).findByText('3 tracks · 1 hr 17 min')).toBeInTheDocument();
  });

  it('lists playlists with the length Rust summed', async () => {
    commands.playlistsList.mockResolvedValue(
      ok([
        {
          id: 'p1',
          name: 'Road trip',
          itemCount: 42,
          artworkUrls: [],
          coverUrl: null,
          length: { knownSecs: 157 * 60, knownCount: 42 },
        },
        {
          id: 'p2',
          name: 'Unknown',
          itemCount: 3,
          artworkUrls: [],
          coverUrl: null,
          length: { knownSecs: 0, knownCount: 0 },
        },
      ]),
    );
    await AppShellWrapper.mount('/library?view=playlists');
    const links = await screen.findAllByTestId('playlist-link');
    expect(links[0]).toHaveTextContent('42 tracks · 2 hr 37 min');
    expect(links[1]).toHaveTextContent('3 tracks');
    expect(links[1]).not.toHaveTextContent('min');
  });

  it('shows an audiobook’s whole listening time', async () => {
    const chapters = [chapter('1', 40 * MINUTE), chapter('2', 50 * MINUTE)];
    const view: AudiobookView = {
      id: 'b1',
      feedUrl: 'https://librivox.org/rss/1',
      title: 'Emma',
      author: 'Jane Austen',
      edition: null,
      description: null,
      artworkUrl: null,
      link: null,
      inLibrary: false,
      chapters,
      order: { sort: 'number', descending: false, filters: [] },
      customOrder: false,
      chapterCount: 2,
      finished: 0,
      downloaded: 0,
      downloading: 0,
      totalMs: 90 * MINUTE,
      listenedMs: 0,
      resume: null,
      speed: null,
    };
    commands.audiobook.mockResolvedValue(ok(view));
    await AppShellWrapper.mount('/audiobook?id=b1');
    const page = await screen.findByTestId('audiobook-page');
    expect(await within(page).findByText('2 chapters · 1 hr 30 min')).toBeInTheDocument();
  });
});
