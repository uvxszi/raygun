import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** Saves echo back the stored settings with the change applied, as Rust does. */
const storeSettings = (initial = DEFAULT_SETTINGS) => {
  let stored = { ...initial };
  commands.settingsGet.mockImplementation(() => Promise.resolve(ok(stored)));
  commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) => {
    stored = { ...stored, [change.key]: change.value };
    return Promise.resolve(ok(stored));
  });
};

const choose = async (
  user: ReturnType<typeof userEvent.setup>,
  row: RegExp,
  option: string | RegExp,
) => {
  await user.click(await screen.findByRole('button', { name: row }));
  const sheet = await screen.findByRole('dialog');
  await user.click(within(sheet).getByRole('radio', { name: option }));
};

describe('Settings sections', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    storeSettings();
    delete document.documentElement.dataset.accent;
  });

  it('changes the accent colour of the whole app', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/appearance');
    await user.click(await screen.findByRole('radio', { name: 'Blue' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'accent', value: 'blue' });
    await vi.waitFor(() => expect(document.documentElement.dataset.accent).toBe('blue'));
    expect(screen.getByRole('radio', { name: 'Blue' })).toBeChecked();
  });

  it('opens on the chosen start screen', async () => {
    storeSettings({ ...DEFAULT_SETTINGS, startScreen: 'library' });
    await AppShellWrapper.mount('/');
    expect(await screen.findByTestId('library-page')).toBeInTheDocument();
  });

  it('opens Browse on the tab chosen in Settings → Browse', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/browse');
    await choose(user, /^Browse opens on/, 'Extensions');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'browseStartTab',
      value: 'extensions',
    });
    await user.click(
      within(screen.getByTestId('bottom-tabs')).getByRole('link', { name: 'Browse' }),
    );
    expect(await screen.findByTestId('browse-tab-extensions')).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  it('keeps links that name a Browse tab', async () => {
    storeSettings({ ...DEFAULT_SETTINGS, browseStartTab: 'extensions' });
    await AppShellWrapper.mount('/browse?tab=sources');
    expect(await screen.findByTestId('browse-tab-sources')).toHaveAttribute('aria-current', 'page');
  });

  it('opens Browse on the first shown tab when the chosen one is hidden', async () => {
    storeSettings({
      ...DEFAULT_SETTINGS,
      browseStartTab: 'discover',
      layout: {
        ...DEFAULT_SETTINGS.layout,
        browseTabs: [
          { id: 'sources', visible: true },
          { id: 'search', visible: true },
          { id: 'discover', visible: false },
          { id: 'extensions', visible: true },
        ],
      },
    });
    await AppShellWrapper.mount('/browse');
    expect(await screen.findByTestId('browse-tab-sources')).toHaveAttribute('aria-current', 'page');
    expect(screen.queryByTestId('browse-tab-discover')).not.toBeInTheDocument();
  });

  it('shows navigation labels on the selected tab only when asked', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/appearance');
    await choose(user, /^Navigation bar labels/, 'Only on the selected tab');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'navLabels', value: 'selected' });
    const tabs = screen.getByTestId('bottom-tabs');
    await vi.waitFor(() => expect(within(tabs).getByText('Library')).toHaveClass('hidden'));
  });

  it('sets covers per row and how titles sit on them', async () => {
    const user = userEvent.setup();
    commands.mangaLibrary.mockResolvedValue(
      ok([
        libraryEntry({
          titleId: 't1',
          sourceId: 'dev.example.comics/comics',
          remoteId: 'm1',
          title: 'Frieren',
          unread: 3,
          downloaded: 2,
        }),
      ]),
    );
    await AppShellWrapper.mount('/settings/library');
    // Covers are the screen's size, made smaller by a quarter (M46.48).
    const size = await screen.findByRole('spinbutton', { name: 'Cover size' });
    await user.clear(size);
    await user.type(size, '-25{Enter}');
    await choose(user, /^Covers in Browse/, 'Title under the cover');
    await choose(user, /^Layout/, 'List');
    await user.click(screen.getByRole('switch', { name: 'Downloaded badges' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'coverSize', value: -25 });
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'coverStyle',
      value: 'comfortable',
    });
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'downloadBadges', value: false });
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'libraryDisplay',
      value: expect.objectContaining({ mode: 'list' }),
    });

    await user.click(screen.getByTestId('nav-library-tab'));
    await user.click(await screen.findByTestId('library-media-manga'));
    const library = await screen.findByTestId('manga-library');
    const item = await within(library).findByTestId('library-item');
    expect(item.closest('ul')).toHaveAttribute('data-testid', 'library-list');
    expect(within(item).getByLabelText('3 unread chapters')).toBeInTheDocument();
    expect(within(item).queryByLabelText('2 chapters downloaded')).not.toBeInTheDocument();
  });

  it('saves the player settings', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/video');
    await user.click(await screen.findByRole('switch', { name: 'Resume videos' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'resumeVideos', value: false });

    await user.click(screen.getByRole('link', { name: /^Gestures/ }));
    await choose(user, /^Double-tap to skip/, '30 seconds');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'seekSeconds', value: 30 });

    // Subtitles start, by default, when the audio is in another language than yours.
    await AppShellWrapper.mount('/settings/video/subtitles');
    expect(await screen.findByRole('button', { name: /^Turn on subtitles/ })).toHaveTextContent(
      'When the audio is in another language',
    );
    expect(screen.getByRole('button', { name: /^Subtitle language/ })).toHaveTextContent(
      'Your languages',
    );
    await choose(user, /^Turn on subtitles/, 'Always');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'videoPlayer',
      value: expect.objectContaining({ subtitleMode: 'always' }),
    });
    await choose(user, /^Subtitle language/, 'Deutsch');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'subtitleLanguage', value: 'de' });
  });

  it("plays audio in your languages unless the video's own track is chosen", async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/video/audio');
    expect(await screen.findByRole('button', { name: /^Audio language/ })).toHaveTextContent(
      'Your languages',
    );
    await choose(user, /^Audio language/, "The video's own");
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'videoPlayer',
      value: expect.objectContaining({ audioLanguage: 'original' }),
    });
  });

  it('saves the novel reader settings', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/reader');
    await choose(user, /^Typeface/, 'Sans serif');
    await user.click(screen.getByRole('switch', { name: 'Turn pages instead of scrolling' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'novelReader',
      value: expect.objectContaining({ font: 'sans' }),
    });
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'novelReader',
      value: expect.objectContaining({ font: 'sans', paged: true }),
    });
  });
});
