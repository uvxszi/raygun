import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type { PlayerPrefs } from '../../settings/playerOptions';
import { AppShellWrapper } from '../../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const lastPlayer = () =>
  commands.settingsUpdate.mock.calls
    .map(([change]) => change as { key: string; value: PlayerPrefs })
    .filter((change) => change.key === 'videoPlayer')
    .at(-1)?.value;

describe('Settings → Video player → Effects', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    let stored = { ...DEFAULT_SETTINGS };
    commands.settingsGet.mockImplementation(() => Promise.resolve(ok(stored)));
    commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) => {
      stored = { ...stored, [change.key]: change.value };
      return Promise.resolve(ok(stored));
    });
  });

  it('is listed with the player screens, effects off until changed', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/video');
    await user.click(await screen.findByRole('link', { name: /^Effects/ }));
    expect(await screen.findByTestId('player-settings-effects')).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: /Night mode/ })).not.toBeChecked();
    expect(screen.getByRole('slider', { name: 'Deflicker' })).toHaveValue('0');
  });

  it('saves the defaults: a strength and a smart audio switch', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/video/effects');
    const deflicker = await screen.findByRole('slider', { name: 'Deflicker' });
    fireEvent.change(deflicker, { target: { value: '40' } });
    await vi.waitFor(() =>
      expect(lastPlayer()?.pictureEffects).toMatchObject({ deflicker: 40, denoise: 0 }),
    );
    await user.click(screen.getByRole('switch', { name: /Even loudness/ }));
    await vi.waitFor(() =>
      expect(lastPlayer()).toMatchObject({
        smartAudio: { dialogue: false, night: false, normalize: true, leveling: false },
        pictureEffects: expect.objectContaining({ deflicker: 40 }),
      }),
    );

    await user.click(screen.getByRole('button', { name: /^Look/ }));
    const sheet = await screen.findByRole('dialog');
    await user.click(within(sheet).getByRole('radio', { name: 'Sepia' }));
    await vi.waitFor(() => expect(lastPlayer()?.pictureEffects.look).toBe('sepia'));
  });
});
