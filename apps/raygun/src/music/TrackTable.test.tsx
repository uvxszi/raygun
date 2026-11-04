import type { SavedTrack, SettingChange, Settings } from '@raygun/bindings';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const song = (title: string, artist: string, addedAt: number, playCount = 0): SavedTrack => ({
  trackKey: `${artist.toLowerCase()}|${title.toLowerCase()}`,
  title,
  artist,
  album: `${title} album`,
  durationMs: 180_000,
  artworkUrl: null,
  source: null,
  local: false,
  addedAt,
  playCount,
});

/** Settings that Rust saves and sends back, as the real command does. */
const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
};

/** A window wide enough for the table's columns. */
const setWide = () => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: true,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

const titles = () =>
  screen
    .getAllByTestId('track-table-row')
    // The title is the row's button with words (the grip, Play and menus have none).
    .map((row) =>
      within(row)
        .getAllByRole('button')
        .map((button) => button.textContent ?? '')
        .find((text) => text.trim() !== ''),
    );

describe('Song tables', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    withSettings(DEFAULT_SETTINGS);
    commands.favoritesList.mockResolvedValue(
      ok([
        song('Bravo', 'Zed', 3_000, 5),
        song('Alpha', 'Mia', 2_000, 1),
        song('Charlie', 'Ann', 1_000, 9),
      ]),
    );
    commands.collectionPlay.mockResolvedValue(ok(null));
    commands.playbackQueueTracks.mockResolvedValue(ok(null));
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('keeps no hidden Save button in the rows of a touch screen, where nothing hovers', async () => {
    // A phone: a finger, and a narrow window.
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: query.includes('coarse'),
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    await AppShellWrapper.mount('/library?view=favorites');
    const rows = await screen.findAllByTestId('track-table-row');
    for (const row of rows) {
      expect(
        within(row).queryByRole('button', { name: 'Save to Favorites or a playlist' }),
      ).not.toBeInTheDocument();
    }
    // The duration and the row's menu stay.
    expect(within(rows[0] as HTMLElement).getByText('3:00')).toBeInTheDocument();
  });

  it('resizes a column live from the divider on its left, and with the arrow keys', async () => {
    setWide();
    await AppShellWrapper.mount('/library?view=favorites');
    await screen.findByTestId('track-table');
    const divider = screen.getByRole('separator', { name: 'Drag to resize Album' });
    const header = screen.getAllByRole('row')[0] as HTMLElement;

    // Dragging the divider 50 px left widens Album by 50 px, and the table follows at once.
    fireEvent.pointerDown(divider, { clientX: 500 });
    fireEvent.pointerMove(window, { clientX: 450 });
    expect(header.style.gridTemplateColumns).toContain('170px');
    fireEvent.pointerUp(window, { clientX: 450 });
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenLastCalledWith(
        expect.objectContaining({ key: 'trackTables' }),
      ),
    );
    const saved = (
      commands.settingsUpdate.mock.lastCall?.[0] as SettingChange & {
        value: Settings['trackTables'];
      }
    ).value.tables?.favorites?.columns;
    expect(saved?.find((column) => column.id === 'album')?.width).toBe(170);

    // From the keyboard: Left makes the column wider.
    divider.focus();
    fireEvent.keyDown(divider, { key: 'ArrowLeft' });
    await vi.waitFor(() => {
      const after = (
        commands.settingsUpdate.mock.lastCall?.[0] as SettingChange & {
          value: Settings['trackTables'];
        }
      ).value.tables?.favorites?.columns;
      expect(after?.find((column) => column.id === 'album')?.width).toBe(136);
    });
  });

  it('fills lengths the sources left out, as Raygun learns them (M46.26)', async () => {
    setWide();
    commands.favoritesList.mockResolvedValue(
      ok([{ ...song('Bravo', 'Zed', 3_000), durationMs: null }, song('Alpha', 'Mia', 2_000)]),
    );
    commands.trackDurations.mockResolvedValue(ok([{ trackKey: 'zed|bravo', durationMs: null }]));
    await AppShellWrapper.mount('/library?view=favorites');
    await screen.findByTestId('track-table');
    await vi.waitFor(() =>
      expect(commands.trackDurations).toHaveBeenCalledWith([
        { title: 'Bravo', artist: 'Zed', album: 'Bravo album', durationMs: null },
      ]),
    );
    const bravo = () => screen.getAllByTestId('track-table-row')[0];
    expect(bravo()).not.toHaveTextContent('3:35');
    // The lookup found it in the background.
    emitEvent('trackDurationsEvent', [{ trackKey: 'zed|bravo', durationMs: 215_000 }]);
    await vi.waitFor(() => expect(bravo()).toHaveTextContent('3:35'));
  });

  it('sorts, filters, shows columns and plays in the order shown', async () => {
    setWide();
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?view=favorites');
    await screen.findByTestId('track-table');
    expect(titles()).toEqual(['Bravo', 'Alpha', 'Charlie']);

    // The Title header sorts A to Z; a second click Z to A.
    await user.click(screen.getByRole('button', { name: 'Title' }));
    await vi.waitFor(() => expect(titles()).toEqual(['Alpha', 'Bravo', 'Charlie']));
    expect(screen.getAllByRole('columnheader')[1]).toHaveAttribute('aria-sort', 'ascending');

    // Sort by in the menu: Most played; then Compact.
    await user.click(screen.getByRole('button', { name: /^Sort and view/ }));
    const menu = await screen.findByRole('dialog', { name: 'Sort by' });
    await user.click(within(menu).getByRole('radio', { name: 'Most played' }));
    await vi.waitFor(() => expect(titles()).toEqual(['Charlie', 'Bravo', 'Alpha']));
    await user.click(within(menu).getByRole('radio', { name: 'Compact' }));
    await user.keyboard('{Escape}');

    // Hide the Album column from the header's menu.
    expect(screen.getByRole('columnheader', { name: 'Album' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Columns' }));
    await user.click(await screen.findByRole('checkbox', { name: 'Album' }));
    await vi.waitFor(() =>
      expect(screen.queryByRole('columnheader', { name: 'Album' })).not.toBeInTheDocument(),
    );
    await user.keyboard('{Escape}');

    // Playing a row plays the table as shown.
    const charlie = screen.getAllByTestId('track-table-row')[0] as HTMLElement;
    fireEvent.doubleClick(charlie);
    // It plays once Rust knows where the play comes from.
    await waitFor(() =>
      expect(commands.collectionPlay).toHaveBeenLastCalledWith({
        collection: { kind: 'favorites' },
        startIndex: 0,
        shuffle: false,
        order: ['ann|charlie', 'zed|bravo', 'mia|alpha'],
      }),
    );

    // Filter.
    await user.click(screen.getByRole('button', { name: 'Find in these songs' }));
    await user.keyboard('mia');
    expect(titles()).toEqual(['Alpha']);
  });

  it('moves rows only in edit mode, turned on and off by Edit and Done (M46.49)', async () => {
    setWide();
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?view=favorites');
    await screen.findByTestId('track-table');
    expect(screen.queryByRole('button', { name: 'Drag to move Bravo' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Edit the order' }));
    expect(screen.getByRole('button', { name: 'Drag to move Bravo' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Done' })).toHaveAttribute('aria-pressed', 'true');

    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(screen.queryByRole('button', { name: 'Drag to move Bravo' })).not.toBeInTheDocument();
  });

  it('acts on several songs at once', async () => {
    setWide();
    const user = userEvent.setup();
    await AppShellWrapper.mount('/library?view=favorites');
    await screen.findByTestId('track-table');
    const rows = screen.getAllByTestId('track-table-row');

    await user.click(rows[0] as HTMLElement);
    await user.keyboard('{Shift>}');
    await user.click(rows[2] as HTMLElement);
    await user.keyboard('{/Shift}');
    const bar = screen.getByRole('toolbar', { name: 'Selected songs' });
    expect(bar).toHaveTextContent('3 songs selected');

    await user.click(within(bar).getByRole('button', { name: /Play next/ }));
    expect(commands.playbackQueueTracks).toHaveBeenCalledWith(
      [
        expect.objectContaining({ title: 'Bravo' }),
        expect.objectContaining({ title: 'Alpha' }),
        expect.objectContaining({ title: 'Charlie' }),
      ],
      true,
    );
  });
});
