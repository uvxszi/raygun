import type {
  MangaLibraryEntry,
  StremioMeta,
  VideoDownloadItem,
  VideoTitleView,
  VideoUpdateItem,
} from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SOURCE_ID = 'dev.raygun.internet-archive/films';

const entry = (
  title: string,
  sourceId: string,
  remoteId: string,
  extra: Partial<MangaLibraryEntry> = {},
): MangaLibraryEntry => ({
  titleId: `t-${remoteId}`,
  sourceId,
  remoteId,
  title,
  coverUrl: null,
  chapters: 10,
  unread: 4,
  categoryIds: [],
  genres: ['Drama'],
  status: 'ongoing',
  addedAt: 1,
  lastReadAt: null,
  latestChapterAt: null,
  started: true,
  downloaded: 1,
  ...extra,
});

const FILM: VideoTitleView = {
  id: 'title-film',
  sourceId: SOURCE_ID,
  sourceName: 'Archive Films',
  remoteId: 'chaplin',
  title: 'Charlie Chaplin Festival',
  coverUrl: null,
  mediaType: 'movie',
  year: 1938,
  description: null,
  genres: [],
  cast: [],
  directors: [],
  status: 'completed',
  url: null,
  inLibrary: false,
  hiddenNsfw: false,
  episodes: [
    {
      id: 'f1',
      remoteId: 'chaplin/film.mp4',
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

const SERIES: StremioMeta = {
  kind: 'series',
  id: 'tt0903747',
  name: 'Breaking Bad',
  poster: null,
  background: null,
  logo: null,
  description: null,
  genres: [],
  releaseInfo: '2008–2013',
  runtime: null,
  rating: null,
  cast: [],
  director: [],
  trailerYtIds: [],
  videos: [1, 2].map((number) => ({
    id: `tt0903747:1:${number}`,
    title: `Episode ${number}`,
    season: 1,
    episode: number,
    released: null,
    thumbnail: null,
    overview: null,
    positionMs: 0,
    durationMs: null,
    watched: false,
    downloaded: number === 1,
  })),
  positionMs: 0,
  durationMs: null,
  watched: false,
  downloaded: false,
  addonName: 'Cinemeta',
  inLibrary: true,
  titleId: 'title-bb',
};

const download = (
  id: string,
  state: VideoDownloadItem['state'],
  extra: Partial<VideoDownloadItem> = {},
): VideoDownloadItem => ({
  id,
  key: `stremio:series:${id}`,
  titleId: 'title-bb',
  title: 'Breaking Bad',
  subtitle: `S1 E${id}`,
  poster: null,
  state,
  doneBytes: 0,
  totalBytes: null,
  doneParts: 0,
  totalParts: null,
  error: null,
  ...extra,
});

const episodeUpdate = (episode: number, extra: Partial<VideoUpdateItem> = {}): VideoUpdateItem => ({
  titleId: 'title-bb',
  sourceId: 'stremio',
  remoteId: 'series:tt0903747',
  title: 'Breaking Bad',
  coverUrl: null,
  episodeId: `u${episode}`,
  episodeRemoteId: `tt0903747:1:${episode}`,
  name: `Episode ${episode}`,
  season: 1,
  number: episode,
  date: null,
  foundAt: Date.now(),
  watched: false,
  downloaded: false,
  ...extra,
});

describe('Video library', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('shows saved titles in Library → Videos, opening each on its own page', async () => {
    const user = userEvent.setup();
    commands.videoLibrary.mockResolvedValue(
      ok([
        entry('Breaking Bad', 'stremio', 'series:tt0903747'),
        entry('Flash Gordon', SOURCE_ID, 'serial', { unread: 0, downloaded: 0 }),
      ]),
    );
    await AppShellWrapper.mount('/library?media=video');
    const library = await screen.findByTestId('video-library');

    const cards = await within(library).findAllByTestId('manga-card');
    expect(cards[0]).toHaveAttribute('href', '/watch/title?type=series&id=tt0903747');
    expect(cards[1]).toHaveAttribute(
      'href',
      `/watch/video?source=${encodeURIComponent(SOURCE_ID)}&id=serial`,
    );
    expect(within(cards[0]!).getByLabelText('4 unwatched episodes')).toBeInTheDocument();

    await user.click(within(library).getByRole('button', { name: 'More options' }));
    const menu = within(library).getByRole('menu');
    for (const name of ['Updates', 'Downloads', 'History', 'Edit categories']) {
      expect(within(menu).getByRole('menuitem', { name })).toBeInTheDocument();
    }
    await user.click(within(menu).getByRole('menuitem', { name: 'Update library' }));
    expect(commands.mangaUpdateLibrary).toHaveBeenCalled();

    // Videos read as videos in the filters.
    await user.click(within(library).getByRole('button', { name: 'Filter and sort' }));
    const sheet = await screen.findByRole('dialog', { name: 'Filters' });
    expect(within(sheet).getByRole('button', { name: 'Unwatched' })).toBeInTheDocument();
    expect(within(sheet).getByRole('option', { name: 'Last watched' })).toBeInTheDocument();
  });

  it('saves a source title and downloads a link from the server picker', async () => {
    const user = userEvent.setup();
    commands.videoTitle.mockResolvedValue(ok(FILM));
    const preferred = {
      url: 'https://cdn.example/film.mp4',
      label: '720p',
      quality: 720,
      headers: {},
      format: null,
      subtitles: [],
      preferred: true,
    };
    commands.videoServers.mockResolvedValue(
      ok([{ id: 'archive', name: 'Archive', links: [preferred] }]),
    );
    await AppShellWrapper.mount(`/watch/video?source=${encodeURIComponent(SOURCE_ID)}&id=chaplin`);
    const page = await screen.findByTestId('video-title-page');

    await user.click(within(page).getByRole('button', { name: 'Add to library' }));
    expect(commands.videoSetInLibrary).toHaveBeenCalledWith('title-film', true);

    // The film's own download control asks for a link, then saves it.
    await user.click(
      within(page).getByRole('button', { name: 'Download Charlie Chaplin Festival' }),
    );
    const picker = await screen.findByRole('dialog', { name: 'Choose what to download' });
    await user.click(await within(picker).findByRole('button', { name: /^720p/ }));
    expect(commands.videoSourceDownload).toHaveBeenCalledWith({
      titleId: 'title-film',
      episodeId: 'f1',
      link: preferred,
    });
    expect(commands.videoSourcePlay).not.toHaveBeenCalled();
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    // From the play picker, each link has its own download button.
    await user.click(within(page).getByRole('button', { name: 'Play' }));
    const servers = await screen.findByRole('dialog', { name: 'Servers' });
    await user.click(await within(servers).findByRole('button', { name: 'Download 720p' }));
    expect(commands.videoSourceDownload).toHaveBeenCalledTimes(2);
  });

  it('takes a Stremio series out of the library and downloads a stream', async () => {
    const user = userEvent.setup();
    commands.stremioMeta.mockResolvedValue(ok(SERIES));
    const direct = {
      kind: 'direct' as const,
      name: 'HLS 720p',
      description: null,
      target: 'https://cdn.example/e2.m3u8',
      headers: {},
      subtitles: [],
    };
    commands.stremioStreams.mockResolvedValue(
      ok([{ addonId: 'local', addonName: 'Local', streams: [direct], error: null }]),
    );
    commands.videoDownloads.mockResolvedValue(
      ok({
        paused: false,
        items: [download('tt0903747:1:1', 'done', { key: 'stremio:series:tt0903747:1:1' })],
      }),
    );
    await AppShellWrapper.mount('/watch/title?type=series&id=tt0903747');
    const page = await screen.findByTestId('watch-title-page');

    const saved = within(page).getByRole('button', { name: 'In library' });
    expect(saved).toHaveAttribute('aria-pressed', 'true');
    // A library title can be put in categories.
    expect(within(page).getByRole('button', { name: 'Add to categories' })).toBeInTheDocument();
    expect(
      await within(page).findByRole('button', {
        name: 'Delete the download of S1 E1 · Episode 1',
      }),
    ).toBeInTheDocument();
    // A downloaded episode plays from the device, with no stream to pick.
    await user.click(within(page).getByRole('button', { name: /^1\. Episode 1/ }));
    expect(commands.videoDownloadPlay).toHaveBeenCalledWith('tt0903747:1:1');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    await user.click(within(page).getByRole('button', { name: 'Download S1 E2 · Episode 2' }));
    const picker = await screen.findByRole('dialog', { name: 'Choose what to download' });
    await user.click(await within(picker).findByRole('button', { name: /^HLS 720p/ }));
    expect(commands.stremioDownload).toHaveBeenCalledWith(
      {
        kind: 'series',
        videoId: 'tt0903747:1:2',
        title: 'Breaking Bad',
        subtitle: 'S1 E2 · Episode 2',
        poster: null,
        stream: direct,
      },
      'en',
      'title-bb',
    );

    await user.click(saved);
    expect(commands.stremioSetInLibrary).toHaveBeenCalledWith('series', 'tt0903747', false);
  });

  it('lists video downloads with progress, pause, retry, delete and play', async () => {
    const user = userEvent.setup();
    commands.videoDownloads.mockResolvedValue(
      ok({
        paused: false,
        items: [
          download('2', 'downloading', {
            doneParts: 30,
            totalParts: 120,
            doneBytes: 50 * 1024 ** 2,
          }),
          download('3', 'failed', { error: 'The server answered 403 Forbidden' }),
          download('1', 'done', { doneBytes: 800 * 1024 ** 2 }),
        ],
      }),
    );
    await AppShellWrapper.mount('/downloads?tab=videos');
    const page = await screen.findByTestId('downloads-page');
    expect(within(page).getByRole('link', { name: 'Videos' })).toHaveAttribute(
      'aria-current',
      'page',
    );

    const running = await within(page).findByRole('listitem', { name: 'Breaking Bad · S1 E2' });
    expect(within(running).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');
    expect(running).toHaveTextContent('25% · 50 MB');
    const failed = within(page).getByRole('listitem', { name: 'Breaking Bad · S1 E3' });
    expect(failed).toHaveTextContent('403 Forbidden');
    const saved = within(page).getByRole('listitem', { name: 'Breaking Bad · S1 E1' });
    expect(saved).toHaveTextContent('On this device · 800 MB');
    expect(within(page).getByText('1 video on this device · 800 MB')).toBeInTheDocument();

    await user.click(within(page).getByRole('button', { name: 'Pause' }));
    expect(commands.videoDownloadsSetPaused).toHaveBeenCalledWith(true);
    await user.click(
      within(failed).getByRole('button', { name: 'Retry downloading Breaking Bad · S1 E3' }),
    );
    expect(commands.videoDownloadRetry).toHaveBeenCalledWith('3');
    await user.click(
      within(running).getByRole('button', { name: 'Cancel the download of Breaking Bad · S1 E2' }),
    );
    expect(commands.videoDownloadRemove).toHaveBeenCalledWith('2');
    await user.click(within(saved).getByRole('button', { name: 'Play Breaking Bad · S1 E1' }));
    expect(commands.videoDownloadPlay).toHaveBeenCalledWith('1');

    commands.videoDownloads.mockClear();
    emitEvent('videoDownloadsEvent', '2');
    await vi.waitFor(() => expect(commands.videoDownloads).toHaveBeenCalled());

    // Chapters keep their own tab.
    await user.click(within(page).getByRole('link', { name: 'Chapters' }));
    expect(await within(page).findByTestId('chapter-downloads')).toBeInTheDocument();
  });

  it('lists new and upcoming episodes in Updates', async () => {
    commands.videoUpdates.mockResolvedValue(
      ok([episodeUpdate(5, { downloaded: true }), episodeUpdate(4, { watched: true })]),
    );
    commands.videoUpcoming.mockResolvedValue(
      ok([episodeUpdate(6, { date: Date.now() + 24 * 60 * 60 * 1000 })]),
    );
    await AppShellWrapper.mount('/updates?tab=episodes');
    const page = await screen.findByTestId('updates-page');

    const upcoming = await within(page).findByRole('region', { name: 'Upcoming' });
    const card = within(upcoming).getByRole('listitem', {
      name: 'Breaking Bad · S1 E6 · Episode 6',
    });
    expect(card).toHaveTextContent('Tomorrow');
    expect(within(card).getByRole('link')).toHaveAttribute(
      'href',
      '/watch/title?type=series&id=tt0903747',
    );

    const rows = within(page).getAllByTestId('episode-update-row');
    expect(rows[0]).toHaveTextContent('S1 E5 · Episode 5');
    expect(within(rows[0]!).getByRole('img', { name: 'Downloaded' })).toBeInTheDocument();
    expect(rows[1]).toHaveClass('opacity-50');
    expect(within(page).getByRole('heading', { name: 'Today' })).toBeInTheDocument();
  });
});
