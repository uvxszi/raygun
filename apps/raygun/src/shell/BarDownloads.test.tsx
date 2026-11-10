import type {
  SettingChange,
  Settings,
  SongDownload,
  VideoDownloadItem,
  WebsiteDownload,
} from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from './AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const song = (id: string, state: SongDownload['state'], at: number): SongDownload => ({
  id,
  trackKey: `a|song ${id}`,
  title: `Song ${id}`,
  artist: 'Artist',
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  state,
  kind: null,
  provider: null,
  format: state === 'done' ? 'mp3' : null,
  doneBytes: 0,
  totalBytes: null,
  fraction: null,
  bytesPerSecond: null,
  etaMs: null,
  error: null,
  errorDetail: null,
  replacing: false,
  problem: null,
  unlisted: false,
  queuedAt: at,
  finishedAt: state === 'done' ? at : null,
});

const video = (id: string, state: VideoDownloadItem['state'], at: number): VideoDownloadItem => ({
  id,
  key: `source:s:${id}`,
  titleId: 't1',
  title: 'Big Buck Bunny',
  subtitle: `Part ${id}`,
  poster: null,
  state,
  doneBytes: 0,
  totalBytes: null,
  doneParts: 0,
  totalParts: null,
  error: null,
  sourceId: 's',
  remoteId: 'bbb',
  episodeId: `e${id}`,
  watched: false,
  at,
});

const file = (state: WebsiteDownload['state']): WebsiteDownload => ({
  id: 'f1',
  url: 'https://archive.org/download/book.pdf',
  name: 'book.pdf',
  mime: 'application/pdf',
  site: 'archive.org',
  state,
  doneBytes: 0,
  totalBytes: null,
  bytesPerSecond: null,
  resumable: true,
  folder: 'Download/Raygun',
  shown: 'Download/Raygun/book.pdf',
  error: null,
  createdAt: 50,
});

const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
};

const openPopover = async (user: ReturnType<typeof userEvent.setup>) => {
  const bar = await screen.findByTestId('app-bar');
  await user.click(within(bar).getByRole('button', { name: 'Downloads' }));
  return screen.findByRole('dialog', { name: 'Downloads' });
};

describe("The desktop bar's Downloads", () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
  });

  it('lists the latest downloads, under way first, each with its own menu', async () => {
    const user = userEvent.setup();
    withSettings({ ...DEFAULT_SETTINGS, downloadsPopover: 5 });
    commands.songDownloads.mockResolvedValue(
      ok([song('1', 'done', 10), song('2', 'done', 40), song('3', 'failed', 30)]),
    );
    commands.videoDownloads.mockResolvedValue(
      ok({ paused: false, items: [video('1', 'downloading', 5), video('2', 'done', 20)] }),
    );
    commands.websiteDownloads.mockResolvedValue([file('done')]);
    await AppShellWrapper.mount('/');
    const popover = await openPopover(user);
    const rows = await within(popover).findAllByTestId('bar-download');
    // Five of six: the one under way, then the newest.
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining('Part 1'),
      expect.stringContaining('book.pdf'),
      expect.stringContaining('Song 2'),
      expect.stringContaining('Song 3'),
      expect.stringContaining('Part 2'),
    ]);
    expect(within(popover).getByRole('list', { name: 'Downloads' })).toHaveClass('overflow-y-auto');
    // Open Downloads stays under the list.
    expect(within(popover).getByRole('link', { name: 'Open Downloads' })).toBeInTheDocument();

    // A video under way pauses the queue or stops, asked first.
    const [running] = rows as [HTMLElement];
    await user.click(within(running).getByRole('button', { name: 'Options for Big Buck Bunny' }));
    let menu = await screen.findByRole('menu');
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent),
    ).toEqual(['Go to title', 'Pause downloads', 'Stop download']);
    await user.click(within(menu).getByRole('menuitem', { name: 'Pause downloads' }));
    expect(commands.videoDownloadsSetPaused).toHaveBeenCalledWith(true);

    const again = await openPopover(user);
    const [first] = await within(again).findAllByTestId('bar-download');
    await user.click(
      within(first as HTMLElement).getByRole('button', { name: 'Options for Big Buck Bunny' }),
    );
    menu = await screen.findByRole('menu');
    await user.click(within(menu).getByRole('menuitem', { name: 'Stop download' }));
    expect(commands.videoDownloadRemove).not.toHaveBeenCalled();
    const ask = await screen.findByRole('alertdialog', {
      name: 'Stop downloading “Big Buck Bunny · Part 1”?',
    });
    await user.click(within(ask).getByRole('button', { name: 'Stop download' }));
    expect(commands.videoDownloadRemove).toHaveBeenCalledWith('1');
  });

  it('gives a file from a website its folder, and a failed song another try', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(ok([song('3', 'failed', 30)]));
    commands.websiteDownloads.mockResolvedValue([file('done')]);
    await AppShellWrapper.mount('/');
    let popover = await openPopover(user);
    await user.click(await within(popover).findByRole('button', { name: 'Options for book.pdf' }));
    let menu = await screen.findByRole('menu');
    expect(
      within(menu)
        .getAllByRole('menuitem')
        .map((item) => item.textContent),
    ).toEqual(['Open', 'Show in folder', 'Delete']);
    await user.click(within(menu).getByRole('menuitem', { name: 'Show in folder' }));
    expect(commands.websiteDownloadReveal).toHaveBeenCalledWith('f1');

    popover = await openPopover(user);
    await user.click(await within(popover).findByRole('button', { name: 'Options for Song 3' }));
    menu = await screen.findByRole('menu');
    await user.click(within(menu).getByRole('menuitem', { name: 'Try again' }));
    expect(commands.songDownloadRetry).toHaveBeenCalledWith('3');
  });

  it('shows as many as Settings → Downloads says', async () => {
    withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/settings/downloads');
    const field = await screen.findByRole('spinbutton', {
      name: /Latest downloads in the bar/,
    });
    expect(field).toHaveValue(20);
    fireEvent.focus(field);
    fireEvent.change(field, { target: { value: '35' } });
    fireEvent.blur(field);
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'downloadsPopover', value: 35 }),
    );
  });
});
