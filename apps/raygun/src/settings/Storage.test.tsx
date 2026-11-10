import type { StorageUsage } from '@raygun/bindings';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { formatBytes } from '../lib/formatBytes';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { LetterAvatar } from '../shell/LetterAvatar';
import { bindingsMock } from '../test/bindingsMock';
import { CACHE_DEFAULTS } from './cacheDefaults';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const MB = 1024 * 1024;

const USAGE: StorageUsage[] = [
  { area: 'covers', bytes: 128 * MB, files: 900, limitBytes: 512 * MB, clearable: true },
  { area: 'pages', bytes: 0, files: 0, limitBytes: 512 * MB, clearable: true },
  { area: 'shared', bytes: 2 * MB, files: 3, limitBytes: null, clearable: true },
  { area: 'musicArt', bytes: 5 * MB, files: 40, limitBytes: null, clearable: false },
  { area: 'downloads', bytes: 3 * 1024 * MB, files: 1200, limitBytes: null, clearable: false },
];

describe('Storage and caches', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.storageUsage.mockResolvedValue(ok(USAGE));
  });

  it('shows what each cache holds and clears one', async () => {
    const user = userEvent.setup();
    commands.storageClear.mockResolvedValue(
      ok(USAGE.map((row) => (row.area === 'covers' ? { ...row, bytes: 0, files: 0 } : row))),
    );
    await AppShellWrapper.mount('/settings/data');
    const covers = await screen.findByRole('listitem', { name: 'Covers and icons' });
    expect(covers).toHaveTextContent('128 MB of 512 MB');
    expect(within(covers).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');
    // Nothing to clear in an empty cache; music artwork and downloads aren't cleared here.
    expect(
      within(screen.getByRole('listitem', { name: 'Reader pages' })).getByRole('button', {
        name: 'Clear Reader pages',
      }),
    ).toBeDisabled();
    expect(
      within(screen.getByRole('listitem', { name: 'Music artwork' })).queryByRole('button'),
    ).not.toBeInTheDocument();
    expect(screen.getByRole('listitem', { name: 'Downloads' })).toHaveTextContent(
      '3.0 GB · 1200 files',
    );

    await user.click(within(covers).getByRole('button', { name: 'Clear Covers and icons' }));
    expect(commands.storageClear).toHaveBeenCalledWith('covers');
    expect(await within(covers).findByText('0 B of 512 MB')).toBeInTheDocument();
  });

  it('changes the cache limits, the refresh age and clearing on start', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/data');
    await user.click(await screen.findByRole('button', { name: /^Covers and icons cache/ }));
    await user.click(screen.getByRole('radio', { name: /^1.0 GB/ }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'cache',
      value: { ...CACHE_DEFAULTS, coverCacheMb: 1024 },
    });
    await user.click(screen.getByRole('button', { name: /^Check covers again after/ }));
    await user.click(screen.getByRole('radio', { name: /^Never/ }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'cache',
      value: expect.objectContaining({ coverRefreshDays: 0 }),
    });
    await user.click(screen.getByRole('switch', { name: 'Clear the page cache on start' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'cache',
      value: expect.objectContaining({ clearPagesOnStart: true }),
    });
  });

  it("shows what's kept about songs in the order it goes, and changes how long", async () => {
    const user = userEvent.setup();
    commands.songMemoryUsage.mockResolvedValue(ok({ songs: 8412, coverSources: 312 }));
    await AppShellWrapper.mount('/settings/data');
    const pictures = await screen.findByText('1. Cover pictures');
    const sources = screen.getByText('2. Where covers came from');
    const rest = screen.getByText('3. Everything else');
    // In that order on the page.
    const follows = (before: Node, after: Node) =>
      Boolean(before.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING);
    expect(follows(pictures, sources) && follows(sources, rest)).toBe(true);
    expect(
      await screen.findByText(/^312 kept · for the 25,000 songs played or seen last/),
    ).toBeInTheDocument();
    expect(screen.getByText(/its lyrics · 8,412 songs$/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /^Keep where covers came from for/ }));
    await user.click(screen.getByRole('radio', { name: /^50,000 songs/ }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'cache',
      value: { ...CACHE_DEFAULTS, coverSourceSongs: 50_000 },
    });
    await user.click(screen.getByRole('button', { name: /^Forget everything else after/ }));
    await user.click(screen.getByRole('radio', { name: /^Never/ }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'cache',
      value: expect.objectContaining({ songFactsDays: 0 }),
    });
  });

  it('formats sizes', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(300 * MB)).toBe('300 MB');
    expect(formatBytes(4 * 1024 * MB)).toBe('4.0 GB');
  });

  it('shows an icon, or the letter when it fails to load', () => {
    render(<LetterAvatar name="NovelFull" logo="http://127.0.0.1:1/cover/ab" />);
    const icon = document.querySelector('img');
    expect(icon).toHaveAttribute('src', 'http://127.0.0.1:1/cover/ab');
    fireEvent.error(icon as HTMLImageElement);
    expect(document.querySelector('img')).toBeNull();
    expect(document.body).toHaveTextContent('N');
  });
});
