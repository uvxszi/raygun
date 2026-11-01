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
): MangaUpdateItem => ({
  titleId: `t-${title}`,
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
    commands.mangaUpdates.mockResolvedValue(
      ok([
        update('Frieren', chapter('c5', 'Chapter 5'), now),
        update('Berserk', chapter('c9', 'Chapter 9', { read: true }), now - DAY),
      ]),
    );
    await AppShellWrapper.mount('/manga/updates');
    const page = await screen.findByTestId('manga-updates-page');

    expect(await within(page).findByRole('heading', { name: 'Today' })).toBeInTheDocument();
    expect(within(page).getByRole('heading', { name: 'Yesterday' })).toBeInTheDocument();
    const rows = within(page).getAllByTestId('update-row');
    expect(rows[0]).toHaveTextContent('FrierenChapter 5');
    expect(rows[0]).toHaveAttribute('href', '/manga/read?title=t-Frieren&chapter=c5');
    expect(rows[1]).toHaveClass('opacity-50');

    await user.click(within(page).getByRole('button', { name: 'Update library' }));
    expect(commands.mangaUpdateLibrary).toHaveBeenCalled();
    expect(await within(page).findByRole('button', { name: 'Updating 0/0' })).toBeDisabled();
    emitEvent('mangaUpdateEvent', { ...IDLE_UPDATE, running: true, checked: 1, total: 4 });
    expect(await within(page).findByRole('button', { name: 'Updating 1/4' })).toBeInTheDocument();

    commands.mangaUpdates.mockClear();
    emitEvent('mangaUpdateEvent', {
      ...IDLE_UPDATE,
      checked: 4,
      total: 4,
      failed: 1,
      lastRunAt: now,
    });
    expect(await within(page).findByRole('button', { name: 'Update library' })).toBeEnabled();
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
    const page = await screen.findByTestId('manga-downloads-page');

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

  it('links updates and downloads from the manga library', async () => {
    commands.mangaLibrary.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/library?media=manga');
    const library = await screen.findByTestId('manga-library');
    expect(await within(library).findByRole('link', { name: 'Updates' })).toHaveAttribute(
      'href',
      '/manga/updates',
    );
    expect(within(library).getByRole('link', { name: 'Downloads' })).toHaveAttribute(
      'href',
      '/manga/downloads',
    );
    expect(within(library).getByRole('button', { name: 'Update library' })).toBeInTheDocument();
  });

  it('sets how often the library is checked', async () => {
    const user = userEvent.setup();
    commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) =>
      Promise.resolve(
        ok({ theme: 'system', language: 'en', mangaUpdateHours: change.value as number }),
      ),
    );
    await AppShellWrapper.mount('/settings');
    const card = await screen.findByTestId('manga-settings-card');
    const select = within(card).getByLabelText('Check the library for new chapters');
    expect(select).toHaveValue('12');
    await user.selectOptions(select, 'Daily');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'mangaUpdateHours', value: 24 });
  });
});
