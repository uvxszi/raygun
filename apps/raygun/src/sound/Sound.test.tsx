import type { SettingChange, Settings, SongEdit, SoundPrefs } from '@raygun/bindings';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';
import { EQ_PRESETS } from './presets';
import { parseTrim } from './format';
import { withSoundDefaults } from './soundPrefs';
import { useSongEditStore } from './songEditStore';

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

const lastSound = () => {
  const calls = commands.settingsUpdate.mock.calls.filter(([change]) => change.key === 'sound');
  return (calls.at(-1)?.[0] as { value: SoundPrefs } | undefined)?.value;
};

describe('Sound settings', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    withSettings(DEFAULT_SETTINGS);
  });

  it('picks a preset, moves a band, and turns on the effects', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/music');
    const equalizer = await screen.findByTestId('equalizer');

    await user.click(within(equalizer).getByRole('switch', { name: 'Equalizer on' }));
    await waitFor(() => expect(lastSound()).toMatchObject({ eqEnabled: true }));
    await user.click(within(equalizer).getByRole('button', { name: 'Rock' }));
    await waitFor(() =>
      expect(lastSound()).toMatchObject({
        eqEnabled: true,
        eqPreset: 'rock',
        eqBands: EQ_PRESETS.find((preset) => preset.id === 'rock')?.bands,
      }),
    );
    expect(within(equalizer).getByRole('button', { name: 'Rock' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );

    fireEvent.change(within(equalizer).getByRole('slider', { name: '1kHz band' }), {
      target: { value: '-60' },
    });
    await waitFor(() =>
      expect(lastSound()).toMatchObject({
        eqPreset: 'custom',
        eqBands: [48, 32, 0, -16, 0, -60, 32, 48, 64, 80],
      }),
    );
    expect(within(equalizer).getByText('Custom')).toBeInTheDocument();

    await user.click(screen.getByRole('switch', { name: /Mono/ }));
    await waitFor(() => expect(lastSound()).toMatchObject({ mono: true }));
    await user.click(screen.getByRole('switch', { name: /Even out loudness/ }));
    await waitFor(() => expect(lastSound()).toMatchObject({ normalize: true, mono: true }));
  });

  it('changes nothing in the equalizer while it is off; only its switch turns it on', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/music');
    const equalizer = await screen.findByTestId('equalizer');
    const band = () => within(equalizer).getByRole('slider', { name: '1kHz band' });
    const preamp = () => within(equalizer).getByRole('slider', { name: 'Preamp' });
    const rock = () => within(equalizer).getByRole('button', { name: 'Rock' });

    // Off: dimmed, and nothing in it can be changed.
    expect(within(equalizer).getByTestId('equalizer-controls')).toHaveAttribute(
      'aria-disabled',
      'true',
    );
    // "Flat" is a preset and the reset button.
    for (const control of [
      band(),
      preamp(),
      rock(),
      ...within(equalizer).getAllByRole('button', { name: 'Flat' }),
    ]) {
      expect(control).toBeDisabled();
    }
    // A drag that reaches the slider anyway (or a double click) neither moves it nor turns the
    // equalizer on.
    fireEvent.change(band(), { target: { value: '-60' } });
    fireEvent.doubleClick(band());
    fireEvent.change(preamp(), { target: { value: '30' } });
    fireEvent.click(rock());
    await new Promise((resolve) => setTimeout(resolve, 300));
    expect(lastSound()).toBeUndefined();
    expect(within(equalizer).getByRole('switch', { name: 'Equalizer on' })).not.toBeChecked();

    // The switch turns it on; then a band changes, and only the band.
    await user.click(within(equalizer).getByRole('switch', { name: 'Equalizer on' }));
    await waitFor(() => expect(band()).toBeEnabled());
    expect(rock()).toBeEnabled();
    fireEvent.change(band(), { target: { value: '-60' } });
    await waitFor(() =>
      expect(lastSound()).toMatchObject({
        eqEnabled: true,
        eqPreset: 'custom',
        eqBands: [0, 0, 0, 0, 0, -60, 0, 0, 0, 0],
      }),
    );

    // Off again: the curve stays as it was, and can't be changed until it's back on.
    await user.click(within(equalizer).getByRole('switch', { name: 'Equalizer on' }));
    await waitFor(() => expect(lastSound()).toMatchObject({ eqEnabled: false }));
    expect(band()).toBeDisabled();
    expect(band()).toHaveValue('-60');
    fireEvent.change(band(), { target: { value: '20' } });
    await new Promise((resolve) => setTimeout(resolve, 300));
    expect(lastSound()).toMatchObject({
      eqEnabled: false,
      eqBands: [0, 0, 0, 0, 0, -60, 0, 0, 0, 0],
    });
  });

  it('starts with a 2 second crossfade and sets it in seconds', async () => {
    expect(withSoundDefaults(null).crossfadeMs).toBe(2000);
    // A stored row without one gets the default; one saved with it off stays off.
    expect(withSoundDefaults({ mono: true } as SoundPrefs).crossfadeMs).toBe(2000);
    expect(withSoundDefaults({ crossfadeMs: 0 } as SoundPrefs).crossfadeMs).toBe(0);
    await AppShellWrapper.mount('/settings/music');
    const crossfade = await screen.findByRole('slider', { name: /^Crossfade/ });
    expect(crossfade).toHaveValue('2');
    fireEvent.change(crossfade, { target: { value: '6' } });
    fireEvent.pointerUp(crossfade);
    await waitFor(() => expect(lastSound()).toMatchObject({ crossfadeMs: 6000 }));
  });
});

describe("A song's own sound", () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    let stored: SongEdit = {};
    commands.songEditGet.mockImplementation(async () => stored);
    commands.songEditSet.mockImplementation(async (_query, edit: SongEdit) => {
      stored = edit;
      return ok(edit);
    });
  });

  it('reads times the way people type them', () => {
    expect(parseTrim('1:23')).toBe(83_000);
    expect(parseTrim('1:23.4')).toBe(83_400);
    expect(parseTrim('83.25')).toBe(83_250);
    expect(parseTrim('1:02:03')).toBe(3_723_000);
    expect(parseTrim('soon')).toBeNull();
  });

  it('trims the start and end and changes the volume, then undoes it all', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    useSongEditStore
      .getState()
      .open({ title: 'SWIM', artist: 'BTS', album: null, durationMs: null });
    const editor = await screen.findByTestId('song-edit');

    const start = within(editor).getByRole('textbox', { name: 'Start at' });
    await user.type(start, '0:12.5{Enter}');
    await waitFor(() =>
      expect(commands.songEditSet).toHaveBeenLastCalledWith(
        { title: 'SWIM', artist: 'BTS', album: null, durationMs: null },
        expect.objectContaining({ startMs: 12_500, endMs: null }),
      ),
    );
    const end = within(editor).getByRole('textbox', { name: 'End at' });
    await user.type(end, '2:30{Enter}');
    await waitFor(() =>
      expect(commands.songEditSet).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({ startMs: 12_500, endMs: 150_000 }),
      ),
    );

    const volume = within(editor).getByRole('slider', { name: 'This song’s volume' });
    fireEvent.change(volume, { target: { value: '-35' } });
    fireEvent.pointerUp(volume);
    await waitFor(() =>
      expect(commands.songEditSet).toHaveBeenLastCalledWith(
        expect.anything(),
        expect.objectContaining({ gain: -35 }),
      ),
    );

    await user.click(within(editor).getByRole('button', { name: /Undo every change/ }));
    await waitFor(() =>
      expect(commands.songEditSet).toHaveBeenLastCalledWith(expect.anything(), {
        startMs: 0,
        endMs: null,
        gain: 0,
        lyricsOffsetMs: 0,
        lyrics: null,
      }),
    );
  });
});
