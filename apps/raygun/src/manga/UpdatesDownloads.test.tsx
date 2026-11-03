import type { MangaChapterInfo, MangaTitleView, MangaUpdateItem } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { IDLE_UPDATE, bindingsMock, emitEvent } from '../test/bindingsMock';
import { groupByDay } from './groupByDay';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.raygun.mangadex/mangadex';
const DAY = 24 * 60 * 60 * 1000;

const chapter = (
  id: string,
  name: string,
  extra: Partial<MangaChapterInfo> = {},
): MangaChapterInfo => ({
  id,
  name,
  number: null,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
  ...extra,
});

const update = (
  title: string,
  chapterInfo: MangaChapterInfo,
  foundAt: number,
  medium: 'manga' | 'novel' = 'manga',
): MangaUpdateItem => ({
  titleId: `t-${title}`,
  medium,
  sourceId: SOURCE,
  remoteId: `m-${title}`,
  title,
  coverUrl: null,
  chapter: chapterInfo,
  foundAt,
});

describe('Manga updates and downloads', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('groups updates by the day they were found', () => {
    const noon = new Date(2026, 9, 31, 12).getTime();
    const groups = groupByDay([
      update('A', chapter('a2', 'Chapter 2'), noon + 1000),
      update('A', chapter('a1', 'Chapter 1'), noon),
      update('B', chapter('b9', 'Chapter 9'), noon - DAY),
    ]);
    expect(groups.map((group) => group.items.map((item) => item.chapter.id))).toEqual([
      ['a2', 'a1'],
      ['b9'],
    ]);
  });

  it('lists updates by day, opens a chapter and runs a library update', async () => {
    const user = userEvent.setup();
    const now = Date.now();
    // The calendar's yesterday: a day when clocks change has 23 or 25 hours.
    const yesterday = new Date(now);
    yesterday.setDate(yesterday.getDate() - 1);
    commands.mangaUpdates.mockResolvedValue(
      ok([
        update('Frieren', chapter('c5', 'Chapter 5'), now),
        update('Berserk', chapter('c9', 'Chapter 9', { read: true }), yesterday.getTime()),
      ]),
    );
    await AppShellWrapper.mount('/updates?tab=chapters');
    const page = await screen.findByTestId('updates-page');

    expect(await within(page).findByRole('heading', { name: 'Today' })).toBeInTheDocument();
    expect(within(page).getByRole('heading', { name: 'Yesterday' })).toBeInTheDocument();
    const rows = within(page).getAllByTestId('update-row');
    expect(rows[0]).toHaveTextContent('FrierenChapter 5');
    expect(rows[0]).toHaveAttribute('href', '/manga/read?title=t-Frieren&chapter=c5');
    expect(rows[1]).toHaveClass('opacity-50');

    // One check covers library titles and followed artists.
    await user.click(within(page).getByRole('button', { name: 'Check for updates' }));
    expect(commands.mangaUpdateLibrary).toHaveBeenCalled();
    expect(commands.artistReleasesCheck).toHaveBeenCalled();
    await vi.waitFor(() =>
      expect(within(page).getByRole('button', { name: 'Check for updates' })).toBeDisabled(),
    );
    expect(within(page).getByRole('status')).toHaveTextContent('Updating 0/0');
    emitEvent('mangaUpdateEvent', { ...IDLE_UPDATE, running: true, checked: 1, total: 4 });
    expect(await within(page).findByText('Updating 1/4')).toBeInTheDocument();

    commands.mangaUpdates.mockClear();
    emitEvent('mangaUpdateEvent', {
      ...IDLE_UPDATE,
      checked: 4,
      total: 4,
      failed: 1,
      lastRunAt: now,
    });
    expect(await within(page).findByRole('button', { name: 'Check for updates' })).toBeEnabled();
    expect(within(page).getByText(/1 title failed/)).toBeInTheDocument();
    await vi.waitFor(() => expect(commands.mangaUpdates).toHaveBeenCalled());
  });

  it('shows the download queue, pauses it and cancels a chapter', async () => {
    const user = userEvent.setup();
    commands.mangaDownloads.mockResolvedValue(
      ok({
        paused: false,
        downloaded: 3,
        items: [
          {
            chapterId: 'c1',
            titleId: 't1',
            title: 'Frieren',
            chapter: 'Chapter 1',
            download: { state: 'downloading', pagesDone: 4, pagesTotal: 16, error: null },
          },
          {
            chapterId: 'c2',
            titleId: 't1',
            title: 'Frieren',
            chapter: 'Chapter 2',
            download: { state: 'failed', pagesDone: 0, pagesTotal: null, error: 'HTTP 404' },
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/manga/downloads');
    const page = await screen.findByTestId('downloads-page');

    const rows = await within(page).findAllByTestId('download-row');
    const [running, failed] = [0, 1].map((index) => {
      const row = rows[index];
      if (!row) throw new Error(`download row ${index} is missing`);
      return row;
    }) as [HTMLElement, HTMLElement];
    expect(running).toHaveTextContent('4 of 16 pages');
    expect(within(running).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '4');
    expect(failed).toHaveTextContent('HTTP 404');
    expect(within(page).getByText('3 chapters on this device')).toBeInTheDocument();

    await user.click(within(page).getByRole('button', { name: 'Pause' }));
    expect(commands.mangaSetDownloadsPaused).toHaveBeenCalledWith(true);
    await user.click(
      within(failed).getByRole('button', { name: 'Retry downloading Frieren · Chapter 2' }),
    );
    expect(commands.mangaDownload).toHaveBeenCalledWith(['c2']);
    await user.click(within(running).getByRole('button', { name: 'Cancel Frieren · Chapter 1' }));
    expect(commands.mangaRemoveDownloads).toHaveBeenCalledWith(['c1']);

    commands.mangaDownloads.mockClear();
    emitEvent('mangaDownloadsEvent', 'c1');
    await vi.waitFor(() => expect(commands.mangaDownloads).toHaveBeenCalled());
  });

  it('retries every failed chapter and cancels the whole queue after asking', async () => {
    const user = userEvent.setup();
    const failedRow = (id: string) => ({
      chapterId: id,
      titleId: 't1',
      title: 'Frieren',
      chapter: `Chapter ${id}`,
      download: { state: 'failed' as const, pagesDone: 0, pagesTotal: null, error: 'HTTP 500' },
    });
    commands.mangaDownloads.mockResolvedValue(
      ok({ paused: false, downloaded: 0, items: [failedRow('c1'), failedRow('c2')] }),
    );
    await AppShellWrapper.mount('/downloads?tab=chapters');
    const queue = await screen.findByTestId('chapter-downloads');

    // A failure is calm, its error under More info.
    const rows = await within(queue).findAllByTestId('download-row');
    const first = rows[0] as HTMLElement;
    expect(within(first).getByTestId('source-issue')).toHaveTextContent("Didn't download");
    expect(within(first).getByText('HTTP 500')).not.toBeVisible();

    await user.click(within(queue).getByRole('button', { name: 'Retry 2 failed' }));
    expect(commands.mangaDownload).toHaveBeenCalledWith(['c1', 'c2']);

    await user.click(within(queue).getByRole('button', { name: 'Cancel all' }));
    expect(commands.mangaRemoveDownloads).not.toHaveBeenCalled();
    await user.click(within(queue).getByRole('button', { name: 'Cancel all 2' }));
    expect(commands.mangaRemoveDownloads).toHaveBeenCalledWith(['c1', 'c2']);
  });

  it('opens new chapters of books in the book reader', async () => {
    commands.mangaUpdates.mockResolvedValue(
      ok([update('Alice', chapter('n1', 'Chapter I'), Date.now(), 'novel')]),
    );
    await AppShellWrapper.mount('/updates?tab=chapters');
    const row = await screen.findByTestId('update-row');
    expect(row).toHaveAttribute('href', '/novel/read?title=t-Alice&chapter=n1');
  });

  it('lists chapters, episodes and releases together by day under All', async () => {
    const user = userEvent.setup();
    const now = Date.now();
    commands.mangaUpdates.mockResolvedValue(
      ok([update('Frieren', chapter('c5', 'Chapter 5'), now - 2000)]),
    );
    commands.videoUpdates.mockResolvedValue(
      ok([
        {
          titleId: 'tv',
          sourceId: 'stremio',
          remoteId: 'series:tt1',
          title: 'Breaking Bad',
          coverUrl: null,
          episodeId: 'e1',
          episodeRemoteId: 'tt1:1:1',
          name: 'Pilot',
          season: 1,
          number: 1,
          date: null,
          foundAt: now - 1000,
          watched: false,
          downloaded: false,
        },
      ]),
    );
    commands.artistReleaseNews.mockResolvedValue(
      ok([
        {
          artist: 'Daft Punk',
          foundAt: now,
          album: {
            provider: {
              id: 'dz',
              extensionId: 'dz',
              kind: 'music.metadata',
              name: 'Deezer',
              searchPill: null,
            },
            id: 'a1',
            title: 'Random Access Memories',
            artist: 'Daft Punk',
            year: 2026,
            artworkUrl: null,
            trackCount: null,
          },
        },
      ]),
    );
    await AppShellWrapper.mount('/updates');
    const all = await screen.findByTestId('updates-all');
    const today = await within(all).findByRole('heading', { name: 'Today' });
    const list = today.nextElementSibling as HTMLElement;
    // Newest first: the release, the episode, then the chapter.
    expect(
      within(list)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual([
      expect.stringContaining('Random Access Memories'),
      expect.stringContaining('Breaking Bad'),
      expect.stringContaining('Frieren'),
    ]);
    expect(within(list).getByTestId('release-row')).toHaveAttribute(
      'href',
      '/music/album?provider=dz&id=a1',
    );
    // An episode plays from here.
    await user.click(
      within(list).getByRole('button', { name: 'Play Breaking Bad · S1 E1 · Pilot' }),
    );
    expect(commands.stremioWatch).toHaveBeenCalledWith('series', 'tt1', 'tt1:1:1');
  });

  it('says once that nothing is new, rather than once per kind', async () => {
    await AppShellWrapper.mount('/updates');
    const page = await screen.findByTestId('updates-page');
    expect(await within(page).findByText('Nothing new yet')).toBeInTheDocument();
    expect(within(page).queryByText('No new chapters yet')).not.toBeInTheDocument();
  });

  it('shows only the kinds of download that have something under All', async () => {
    commands.videoDownloads.mockResolvedValue(
      ok({
        paused: false,
        items: [
          {
            id: 'v1',
            key: 'url:x',
            titleId: null,
            title: 'Sintel',
            subtitle: null,
            poster: null,
            state: 'done',
            doneBytes: 1024 ** 3,
            totalBytes: null,
            doneParts: 0,
            totalParts: null,
            error: null,
          },
        ],
      }),
    );
    commands.storageUsage.mockResolvedValue(
      ok([
        { area: 'downloads', bytes: 2 * 1024 ** 3, files: 3, limitBytes: null, clearable: false },
      ]),
    );
    await AppShellWrapper.mount('/downloads');
    const all = await screen.findByTestId('downloads-all');
    expect(await within(all).findByRole('region', { name: 'Videos' })).toBeInTheDocument();
    expect(within(all).queryByRole('region', { name: 'Chapters' })).not.toBeInTheDocument();
    expect(within(all).queryByRole('region', { name: 'Songs' })).not.toBeInTheDocument();
    expect(await within(all).findByTestId('downloads-storage')).toHaveTextContent(
      'Downloads take 2.0 GB on this device',
    );
  });

  it('says once that nothing is downloaded', async () => {
    await AppShellWrapper.mount('/downloads');
    const all = await screen.findByTestId('downloads-all');
    expect(within(all).getByText('Nothing downloaded yet')).toBeInTheDocument();
    expect(within(all).queryByText('Nothing is downloading.')).not.toBeInTheDocument();
  });

  it('downloads chapters from the title page and shows their states', async () => {
    const user = userEvent.setup();
    const title: MangaTitleView = {
      id: 't1',
      hiddenNsfw: false,
      sourceId: SOURCE,
      sourceName: 'MangaDex',
      remoteId: 'm1',
      title: 'Frieren',
      coverUrl: null,
      authors: [],
      artists: [],
      description: null,
      genres: [],
      status: 'ongoing',
      url: null,
      inLibrary: true,
      chapters: [
        chapter('c5', 'Chapter 5'),
        chapter('c4', 'Chapter 4'),
        chapter('c3', 'Chapter 3', {
          download: { state: 'downloading', pagesDone: 5, pagesTotal: 10, error: null },
        }),
        chapter('c2', 'Chapter 2', {
          download: { state: 'done', pagesDone: 12, pagesTotal: 12, error: null },
        }),
        chapter('c1', 'Chapter 1', { read: true }),
      ],
      continueChapterId: 'c3',
    };
    commands.mangaTitle.mockResolvedValue(ok(title));
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');

    await user.click(await within(page).findByRole('button', { name: 'Download Chapter 5' }));
    expect(commands.mangaDownload).toHaveBeenLastCalledWith(['c5']);
    expect(
      within(page).getByRole('button', { name: 'Downloading Chapter 3, 50%. Cancel' }),
    ).toBeInTheDocument();
    await user.click(
      within(page).getByRole('button', { name: 'Delete the download of Chapter 2' }),
    );
    expect(commands.mangaRemoveDownloads).toHaveBeenLastCalledWith(['c2']);

    // Unread and not yet downloaded, oldest first.
    await user.click(within(page).getByRole('button', { name: 'Download 2 unread' }));
    expect(commands.mangaDownload).toHaveBeenLastCalledWith(['c4', 'c5']);
  });

  it('offers updates, downloads and a library update from the manga library menu', async () => {
    const user = userEvent.setup();
    commands.mangaLibrary.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/library?media=manga');
    const library = await screen.findByTestId('manga-library');
    await user.click(within(library).getByRole('button', { name: 'More options' }));
    const menu = within(library).getByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: 'Updates' })).toBeInTheDocument();
    expect(within(menu).getByRole('menuitem', { name: 'Downloads' })).toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Update library' }));
    expect(commands.mangaUpdateLibrary).toHaveBeenCalled();

    await user.click(within(library).getByRole('button', { name: 'More options' }));
    await user.click(within(library).getByRole('menuitem', { name: 'Updates' }));
    expect(await screen.findByTestId('updates-page')).toBeInTheDocument();
  });

  it('sets how often the library is checked', async () => {
    const user = userEvent.setup();
    commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) =>
      Promise.resolve(
        ok({ theme: 'system', language: 'en', mangaUpdateHours: change.value as number }),
      ),
    );
    await AppShellWrapper.mount('/settings/library');
    const row = await screen.findByRole('button', { name: /Check the library for new chapters/ });
    expect(row).toHaveTextContent('Every 12 hours');
    await user.click(row);
    const sheet = await screen.findByRole('dialog', {
      name: 'Check the library for new chapters and episodes',
    });
    await user.click(within(sheet).getByRole('radio', { name: 'Daily' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'mangaUpdateHours', value: 24 });
  });
});
