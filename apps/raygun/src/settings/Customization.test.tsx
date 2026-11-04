import type { MangaChapterPages, SettingChange, Settings } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { coverGridStyle } from '../manga/coverGrid';
import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** Settings that Rust saves and sends back, as the real command does. */
const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
};

const changes = (key: string) =>
  commands.settingsUpdate.mock.calls
    .map(([change]) => change as SettingChange)
    .filter((change) => change.key === key)
    .map((change) => change.value);

/** A window at least `lg` wide (every media query matches), or a phone. */
const setWide = (wide: boolean) => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: wide,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

const chapterPages = (readerMode: MangaChapterPages['readerMode']): MangaChapterPages => ({
  titleId: 't1',
  title: 'Frieren',
  readerMode,
  chapter: {
    id: 'c1',
    remoteId: 'r1',
    name: 'Chapter 1',
    number: 1,
    uploadedAt: null,
    scanlator: null,
    read: false,
    lastPage: null,
    pageCount: null,
    bookmarked: false,
    downloaded: false,
  } as unknown as MangaChapterPages['chapter'],
  pages: ['http://img/1', 'http://img/2', 'http://img/3'],
  previousChapterId: null,
  nextChapterId: null,
});

describe('Customization', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    withSettings(DEFAULT_SETTINGS);
    document.documentElement.style.removeProperty('--accent-on-dark');
  });

  afterEach(() => {
    // jsdom has no matchMedia of its own.
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('takes any accent colour and puts it on the page', async () => {
    await AppShellWrapper.mount('/settings/appearance');
    const picker = await screen.findByLabelText('Any colour');
    fireEvent.change(picker, { target: { value: '#1e90ff' } });

    await vi.waitFor(() => expect(changes('accent')).toEqual(['custom']));
    expect(changes('accentColor')).toEqual(['#1e90ff']);
    await vi.waitFor(() =>
      expect(document.documentElement.style.getPropertyValue('--accent-on-dark')).toContain(
        '#1e90ff',
      ),
    );
    expect(document.documentElement.dataset.accent).toBe('custom');
  });

  it('sizes covers by the screen, then bigger or smaller', async () => {
    const user = userEvent.setup();
    commands.catalogGlobalSearch.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/settings/library');
    const size = await screen.findByRole('spinbutton', { name: 'Cover size' });
    await user.clear(size);
    await user.type(size, '20{Enter}');
    expect(changes('coverSize')).toEqual([20]);
    await user.click(screen.getByRole('button', { name: 'Reset Cover size' }));
    expect(changes('coverSize')).toEqual([20, 0]);
    expect(coverGridStyle(20).gridTemplateColumns).toBe(
      'repeat(auto-fill, minmax(min(100%, calc(var(--cover-width) * 1.2)), 1fr))',
    );
  });

  it('fits pages, zooms them and arranges the page bar', async () => {
    const user = userEvent.setup();
    commands.mangaChapter.mockResolvedValue(ok(chapterPages('pagedLtr')));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const paged = await within(reader).findByTestId('paged-reader');
    expect(paged).toHaveAttribute('data-scale', 'fitScreen');

    // The page bar zooms; Ctrl+0 goes back to the fit.
    const bar = within(reader).getByTestId('reader-bar');
    await user.click(within(bar).getByRole('button', { name: 'Zoom in' }));
    expect(within(bar).getByRole('button', { name: 'Back to the fit' })).toHaveTextContent('110%');
    const page = within(reader).getByTestId('manga-page');
    expect(page.style.width).toBe('110%');
    await user.keyboard('{Control>}0{/Control}');
    expect(within(bar).getByRole('button', { name: 'Back to the fit' })).toHaveTextContent('100%');

    // Ctrl+scroll zooms too; plain scrolling doesn't.
    fireEvent.wheel(window, { deltaY: -100, ctrlKey: true });
    expect(within(bar).getByRole('button', { name: 'Back to the fit' })).toHaveTextContent('110%');
    fireEvent.wheel(window, { deltaY: -100 });
    expect(within(bar).getByRole('button', { name: 'Back to the fit' })).toHaveTextContent('110%');

    // The Display sheet: fit width, and a page bar without zoom buttons.
    await user.click(within(reader).getByRole('button', { name: 'Display' }));
    const sheet = await screen.findByRole('dialog', { name: 'Display' });
    await user.click(within(sheet).getByRole('button', { name: /^Page fit/ }));
    await user.click(screen.getByRole('radio', { name: /^Fit width/ }));
    expect(changes('mangaReader').at(-1)).toMatchObject({ pagedScale: 'fitWidth' });
    await vi.waitFor(() => expect(paged).toHaveAttribute('data-scale', 'fitWidth'));
    await user.click(within(sheet).getByRole('switch', { name: 'Zoom buttons in the page bar' }));
    expect(changes('mangaReader').at(-1)).toMatchObject({ barZoom: false });
    await vi.waitFor(() =>
      expect(within(bar).queryByRole('button', { name: 'Zoom in' })).not.toBeInTheDocument(),
    );
  });

  it("names a catalog's rows in Search when nothing is typed", async () => {
    commands.stremioHome.mockResolvedValue(
      ok([
        {
          addonId: 'cinemeta',
          addonName: 'Cinemeta',
          kind: 'movie',
          catalogId: 'top',
          name: 'Popular',
          items: [],
          genres: [],
          hasMore: false,
          error: null,
        },
        {
          addonId: 'cinemeta',
          addonName: 'Cinemeta',
          kind: 'movie',
          catalogId: 'year',
          name: 'New',
          items: [],
          genres: [],
          hasMore: false,
          error: null,
        },
      ]),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=video');
    expect(await screen.findByRole('region', { name: 'Popular · Movies' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'New · Movies' })).toBeInTheDocument();
    emitEvent('watchChangedEvent', 'x');
  });
  it('opens track actions beside their button, and closes them by clicking away or again', async () => {
    setWide(true);
    const user = userEvent.setup();
    commands.playlistsList.mockResolvedValue(ok([]));
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro', 'Raygun'), makeItem('Second')], 0);
    await AppShellWrapper.miniPlayer.title;
    // The wide bar has two ways in: its artwork and title, and the expand button.
    await user.click(
      within(AppShellWrapper.miniPlayer.element).getAllByRole('button', {
        name: 'Open player',
      })[0]!,
    );
    const sheet = await AppShellWrapper.playerSheet.element;
    const button = within(sheet).getByRole('button', { name: 'Add to playlist' });

    await user.click(button);
    const popover = await screen.findByTestId('popover');
    expect(popover).toHaveAccessibleName('Intro');
    expect(popover).not.toHaveAttribute('aria-modal');
    expect(within(popover).getByText('Add to playlist')).toBeInTheDocument();

    // The same button closes it…
    await user.click(button);
    expect(screen.queryByTestId('popover')).not.toBeInTheDocument();
    // …and so does a click anywhere else.
    await user.click(button);
    expect(await screen.findByTestId('popover')).toBeInTheDocument();
    await user.click(within(sheet).getByTestId('sheet-title'));
    expect(screen.queryByTestId('popover')).not.toBeInTheDocument();
  });

  it('lays the full player side by side on wide windows, or stacked when chosen', async () => {
    setWide(true);
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro', 'Raygun'), makeItem('Second')], 0);
    await AppShellWrapper.miniPlayer.title;
    // The wide bar has two ways in: its artwork and title, and the expand button.
    await user.click(
      within(AppShellWrapper.miniPlayer.element).getAllByRole('button', {
        name: 'Open player',
      })[0]!,
    );
    const sheet = await AppShellWrapper.playerSheet.element;
    expect(sheet).toHaveAttribute('data-layout', 'sideBySide');

    // Lyrics and the queue share the right column, as tabs.
    await user.click(within(sheet).getByRole('tab', { name: 'Up next' }));
    expect(within(sheet).getByTestId('up-next')).toHaveTextContent('Second');

    // The layout button's popover, and a choice in it, which pops up beside its row.
    await user.click(within(sheet).getByRole('button', { name: 'Player layout' }));
    const layout = await screen.findByRole('dialog', { name: 'Player layout' });
    await user.click(within(layout).getByRole('button', { name: /^Layout/ }));
    await user.click(screen.getByRole('radio', { name: /^Stacked/ }));
    expect(changes('playerView').at(-1)).toMatchObject({ layout: 'stacked' });
    await vi.waitFor(() => expect(sheet).toHaveAttribute('data-layout', 'stacked'));
  });

  it('sets how much things move and how scrollbars look', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/appearance');
    await vi.waitFor(() => expect(document.documentElement.dataset.motion).toBe('lively'));
    expect(document.documentElement.dataset.scrollbars).toBe('thin');

    await user.click(await screen.findByRole('button', { name: /^Animations/ }));
    await user.click(screen.getByRole('radio', { name: /^Off/ }));
    await vi.waitFor(() => expect(document.documentElement.dataset.motion).toBe('off'));
    await user.click(screen.getByRole('button', { name: /^Scrollbars/ }));
    await user.click(screen.getByRole('radio', { name: /^Hidden/ }));
    await vi.waitFor(() => expect(document.documentElement.dataset.scrollbars).toBe('hidden'));
  });
});
