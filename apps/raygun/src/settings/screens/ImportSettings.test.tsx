import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../../shell/AppShell.test-wrapper';
import { bindingsMock } from '../../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

describe('Settings → Import', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('keeps the Spotify app and the reader for screenshots', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/import');

    await user.click(await screen.findByRole('button', { name: /Client ID/ }));
    const idSheet = await screen.findByRole('dialog', { name: 'Client ID' });
    await user.type(within(idSheet).getByRole('textbox', { name: 'Client ID' }), 'abc123');
    await user.keyboard('{Enter}');
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'import',
      value: { reader: 'auto', spotifyClientId: 'abc123' },
    });

    await user.click(screen.getByRole('button', { name: /Client secret/ }));
    const secretSheet = await screen.findByRole('dialog', { name: 'Client secret' });
    await user.type(within(secretSheet).getByRole('textbox', { name: 'Client secret' }), 's3cret');
    await user.keyboard('{Enter}');
    expect(commands.importSetSpotifySecret).toHaveBeenCalledWith('s3cret');

    await user.click(screen.getByRole('button', { name: /Text reader/ }));
    await user.click(await screen.findByRole('radio', { name: /Windows' text reader/ }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'import',
      value: expect.objectContaining({ reader: 'windows' }),
    });
  });
});
