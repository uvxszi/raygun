import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, DEFAULT_SETTINGS, bindingsMock, emitEvent } from '../test/bindingsMock';
import { PLAYER_DEFAULTS } from './playerOptions';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** Saves echo back the stored settings with the change applied, as Rust does. */
const storeSettings = () => {
  let stored = { ...DEFAULT_SETTINGS };
  commands.settingsGet.mockImplementation(() => Promise.resolve(ok(stored)));
  commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) => {
    stored = { ...stored, [change.key]: change.value };
    return Promise.resolve(ok(stored));
  });
  commands.settingsUpdateMany.mockImplementation((changes: { key: string; value: unknown }[]) => {
    for (const change of changes) stored = { ...stored, [change.key]: change.value };
    return Promise.resolve(ok(stored));
  });
  return {
    replace: (videoPlayer: typeof PLAYER_DEFAULTS) => {
      stored = { ...stored, videoPlayer };
    },
    saved: () => stored,
  };
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

const lastPlayer = () =>
  commands.settingsUpdate.mock.calls
    .map(([change]) => change as { key: string; value: typeof PLAYER_DEFAULTS })
    .filter((change) => change.key === 'videoPlayer')
    .at(-1)?.value;

describe('Settings → Video player', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('lists its screens and the sources list options', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/video');
    for (const name of [
      /^Controls/,
      /^Gestures/,
      /^Subtitles/,
      /^Audio/,
      /^Skip intro/,
      /^Picture-in-picture/,
      /^Advanced/,
    ]) {
      expect(await screen.findByRole('link', { name })).toBeInTheDocument();
    }
    await user.click(screen.getByRole('switch', { name: /Show sources that failed/ }));
    expect(lastPlayer()).toEqual({ ...PLAYER_DEFAULTS, showFailedSources: false });

    await user.click(screen.getByRole('link', { name: /^Skip intro/ }));
    expect(await screen.findByTestId('player-settings-skip')).toBeInTheDocument();
  });

  it('saves controls: orientation, speed presets and the hold speed', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/video/controls');
    await choose(user, /^Orientation/, 'Follow the video');
    expect(lastPlayer()).toMatchObject({ orientation: 'video' });

    await user.click(screen.getByRole('button', { name: /^Speed presets/ }));
    const sheet = await screen.findByRole('dialog', { name: 'Speed presets' });
    const input = within(sheet).getByRole('textbox', { name: 'Speed presets' });
    await user.clear(input);
    await user.type(input, '1, 5');
    expect(within(sheet).getByRole('alert')).toHaveTextContent(/0.25 to 4/);
    expect(within(sheet).getByRole('button', { name: 'Save' })).toBeDisabled();
    await user.clear(input);
    await user.type(input, '1.5, 1, 2x');
    await user.click(within(sheet).getByRole('button', { name: 'Save' }));
    expect(lastPlayer()).toMatchObject({ orientation: 'video', speedPresets: [100, 150, 200] });

    await choose(user, /^Hold for speed/, 'Off');
    expect(lastPlayer()).toMatchObject({ holdSpeed: 0 });
  });

  it('sets up double taps and swipes', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/video/gestures');
    await choose(user, /^Left/, 'Previous or next episode');
    await user.click(screen.getByRole('switch', { name: /Swap sides/ }));
    expect(lastPlayer()).toMatchObject({ doubleTapLeft: 'switchEpisode', swapSides: true });
    // The centre can't seek: it has no direction.
    await user.click(screen.getByRole('button', { name: /^Centre/ }));
    const sheet = await screen.findByRole('dialog', { name: 'Centre' });
    expect(within(sheet).queryByRole('radio', { name: 'Seek' })).not.toBeInTheDocument();
  });

  it('styles subtitles with a live preview, and resets the style', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/video/subtitles');
    const text = await screen.findByTestId('subtitle-preview-text');
    expect(text).toHaveStyle({ color: 'rgb(255, 255, 255)' });

    await choose(user, /^Text colour/, 'Yellow');
    await user.click(screen.getByRole('switch', { name: 'Bold' }));
    expect(lastPlayer()?.subtitleStyle).toMatchObject({ textColor: 0xffffeb3b, bold: true });
    expect(screen.getByTestId('subtitle-preview-text')).toHaveStyle({
      color: 'rgb(255, 235, 59)',
      fontWeight: '700',
    });

    await user.click(screen.getByRole('button', { name: 'Reset the style' }));
    expect(lastPlayer()?.subtitleStyle).toEqual(PLAYER_DEFAULTS.subtitleStyle);
  });

  it('keeps white and "None" picked as picks, over the file\'s own colours', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/video/subtitles');
    // White is the default, so picking it changes no colour: it marks it picked, and a file's
    // own colours then give way to it.
    await choose(user, /^Text colour/, 'White');
    expect(lastPlayer()?.subtitleStyle).toMatchObject({
      textColor: 0xffffffff,
      textColorChosen: true,
    });
    await choose(user, /^Background/, 'None');
    expect(lastPlayer()?.subtitleStyle).toMatchObject({
      backgroundColor: 0,
      backgroundColorChosen: true,
    });
  });

  it('picks the subtitle font among the caption families, each named in its own font', async () => {
    const user = userEvent.setup();
    commands.fontsList.mockClear();
    storeSettings();
    await AppShellWrapper.mount('/settings/video/subtitles');
    await user.click(await screen.findByRole('button', { name: /^Font/ }));
    const sheet = await screen.findByRole('radiogroup', { name: 'Font' });
    expect(
      within(sheet)
        .getAllByRole('radio')
        .map((radio) => radio.getAttribute('aria-label')),
    ).toEqual([
      'Default',
      'Sans serif',
      'Condensed',
      'Monospace',
      'Serif',
      'Serif monospace',
      'Casual',
      'Cursive',
      'Small capitals',
    ]);
    expect(within(sheet).getByText('Small capitals')).toHaveStyle({
      fontVariantCaps: 'small-caps',
    });
    // Android's player draws the system's families: the phone's own fonts aren't offered.
    expect(within(sheet).queryByText('On this device')).toBeNull();
    expect(commands.fontsList).not.toHaveBeenCalled();

    await user.click(within(sheet).getByRole('radio', { name: 'Casual' }));
    expect(lastPlayer()?.subtitleStyle).toMatchObject({ font: 'casual' });
    await vi.waitFor(() =>
      expect(screen.getByTestId('subtitle-preview-text').style.fontFamily).toMatch(/^casual,/),
    );
  });

  it('offers the desktop’s fonts for subtitles, kept to the device', async () => {
    const user = userEvent.setup();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.fontsList.mockResolvedValue({
      status: 'ok',
      data: [{ family: 'Comic Sans MS', faces: [] }],
    });
    const { saved } = storeSettings();
    await AppShellWrapper.mount('/settings/video/subtitles');
    await user.click(await screen.findByRole('button', { name: /^Font/ }));
    const sheet = await screen.findByRole('radiogroup', { name: 'Font' });
    await user.click(await within(sheet).findByRole('radio', { name: 'Comic Sans MS' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'fonts',
      value: expect.objectContaining({ subtitles: 'Comic Sans MS' }),
    });
    await vi.waitFor(() =>
      expect(screen.getByTestId('subtitle-preview-text').style.fontFamily).toMatch(
        /^"Comic Sans MS"/,
      ),
    );

    // One of the families again puts it away, with the family saved at once.
    await user.click(screen.getByRole('button', { name: /^Font/ }));
    await user.click(
      within(await screen.findByRole('radiogroup', { name: 'Font' })).getByRole('radio', {
        name: 'Serif',
      }),
    );
    await vi.waitFor(() => expect(saved().fonts?.subtitles).toBe(''));
    expect(saved().videoPlayer.subtitleStyle?.font).toBe('serif');
  });

  it("keeps lines to a share of the picture's width", async () => {
    storeSettings();
    await AppShellWrapper.mount('/settings/video/subtitles');
    const width = await screen.findByRole('slider', { name: 'Line width' });
    expect(width).toHaveValue('80');
    fireEvent.change(width, { target: { value: '60' } });
    await vi.waitFor(() => expect(lastPlayer()?.subtitleStyle.maxWidth).toBe(60));
    // The preview's line wraps within it too.
    await vi.waitFor(() =>
      expect(screen.getByTestId('subtitle-preview-text').parentElement).toHaveStyle({
        paddingLeft: '20%',
      }),
    );
  });

  it('picks a subtitle colour of your own, typed, with its opacity', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/video/subtitles');
    await user.click(await screen.findByRole('button', { name: /^Background/ }));
    const sheet = await screen.findByRole('dialog');
    // Each colour beside its swatch, then one of your own.
    expect(
      within(sheet)
        .getAllByRole('radio')
        .map((radio) => radio.textContent),
    ).toEqual([
      'None',
      'Black, half see-through',
      'Black, slightly see-through',
      'Black',
      'White, half see-through',
      'Custom',
    ]);
    await user.click(within(sheet).getByRole('radio', { name: 'Custom' }));
    const code = within(sheet).getByRole('textbox', { name: 'Colour code' });
    await user.clear(code);
    await user.type(code, '#3366ff{Enter}');
    // A background that was none starts half see-through.
    await vi.waitFor(() => expect(lastPlayer()?.subtitleStyle.backgroundColor).toBe(0x803366ff));
    // Custom's swatch shows the colour picked, not the hues it's picked from.
    expect(within(sheet).getByTestId('custom-swatch')).toHaveAttribute('data-color', '#3366ff');
    fireEvent.change(within(sheet).getByRole('slider', { name: 'Opacity' }), {
      target: { value: '100' },
    });
    await vi.waitFor(() => expect(lastPlayer()?.subtitleStyle.backgroundColor).toBe(0xff3366ff));
    expect(lastPlayer()?.subtitleStyle.backgroundColorChosen).toBe(true);
    expect(screen.getByRole('button', { name: /^Background/, hidden: true })).toHaveTextContent(
      'Your own colour · #3366FF',
    );
    // It follows the colour as it's picked, before it's kept.
    fireEvent.input(within(sheet).getByLabelText('Pick a colour'), {
      target: { value: '#ff8800' },
    });
    expect(within(sheet).getByTestId('custom-swatch')).toHaveAttribute('data-color', '#ff8800');
  });

  it('turns on AniSkip and picks another app', async () => {
    const user = userEvent.setup();
    storeSettings();
    await AppShellWrapper.mount('/settings/video/skip');
    const unless = await screen.findByRole('switch', { name: /Not when the video has chapters/ });
    expect(unless).toBeDisabled();
    await user.click(screen.getByRole('switch', { name: /Find openings and endings of anime/ }));
    expect(lastPlayer()).toMatchObject({ aniskip: true });
    expect(await screen.findByRole('switch', { name: /Not when the video/ })).toBeEnabled();

    await AppShellWrapper.mount('/settings/video/advanced');
    await choose(user, /^Other app/, 'VLC');
    expect(lastPlayer()).toMatchObject({ externalPackage: 'org.videolan.vlc' });
  });

  it('reads the settings again when the player changes one', async () => {
    const store = storeSettings();
    await AppShellWrapper.mount('/settings/video/controls');
    expect(await screen.findByRole('switch', { name: /Show the time left/ })).not.toBeChecked();

    store.replace({ ...PLAYER_DEFAULTS, showRemaining: true });
    emitEvent('settingsChangedEvent', 'videoPlayer');
    await vi.waitFor(() =>
      expect(screen.getByRole('switch', { name: /Show the time left/ })).toBeChecked(),
    );
  });
});
