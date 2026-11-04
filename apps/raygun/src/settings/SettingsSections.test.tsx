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
          sourceId: 'dev.raygun.mangadex/mangadex',
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
    await AppShellWrapper.mount('/settings/player');
    await user.click(await screen.findByRole('switch', { name: 'Resume videos' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'resumeVideos', value: false });

    await user.click(screen.getByRole('link', { name: /^Gestures/ }));
    await choose(user, /^Double-tap to skip/, '30 seconds');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'seekSeconds', value: 30 });

    await AppShellWrapper.mount('/settings/player/subtitles');
    await user.click(await screen.findByRole('switch', { name: 'Turn on subtitles' }));
    await choose(user, /^Subtitle language/, 'Deutsch');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'autoSubtitles', value: true });
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'subtitleLanguage', value: 'de' });
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
