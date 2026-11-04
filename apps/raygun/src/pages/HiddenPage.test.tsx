import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const HIDDEN = {
  artists: [
    { name: 'Older Band', hiddenAt: 10 },
    { name: 'Tate McRae', hiddenAt: 20 },
  ],
  items: [
    {
      kind: 'song',
      key: 'jamie miller|babydoll',
      name: 'Babydoll',
      subtitle: 'Jamie Miller',
      artworkUrl: null,
      hiddenAt: 5,
    },
  ],
};

describe('Hidden artists and Not interested', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, hidden: HIDDEN }));
  });

  it('are counted in Settings → Browse, and listed newest first to show again', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/browse');
    const link = await screen.findByRole('link', { name: /Hidden/ });
    expect(link).toHaveTextContent('2 artists · 1 other thing');
    await user.click(link);

    const page = await screen.findByTestId('hidden-page');
    const artists = within(page).getByRole('region', { name: 'Artists' });
    expect(
      within(artists)
        .getAllByRole('listitem')
        .map((row) => row.textContent),
    ).toEqual(['Tate McRaeShow again', 'Older BandShow again']);
    const items = within(page).getByRole('region', { name: 'Not interested' });
    expect(within(items).getByText('Song · Jamie Miller')).toBeInTheDocument();

    await user.click(within(artists).getByRole('button', { name: 'Show Tate McRae again' }));
    expect(commands.hiddenArtistSet).toHaveBeenCalledWith('Tate McRae', false);
    await vi.waitFor(() => expect(within(artists).queryByText('Tate McRae')).toBeNull());
    expect(await screen.findByTestId('toast')).toHaveTextContent('Tate McRae is shown again');

    await user.click(within(items).getByRole('button', { name: 'Show Babydoll again' }));
    expect(commands.hiddenItemSet).toHaveBeenCalledWith(
      expect.objectContaining({ key: 'jamie miller|babydoll' }),
      false,
    );
    expect(await within(items).findByText('Nothing marked Not interested')).toBeInTheDocument();
  });

  it('shows a whole list again at once', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/hidden');
    const artists = await screen.findByRole('region', { name: 'Artists' });
    await user.click(within(artists).getByRole('button', { name: 'Show all again' }));
    expect(commands.hiddenClear).toHaveBeenCalledWith(true, false);
    expect(await within(artists).findByText('No hidden artists')).toBeInTheDocument();
  });
});
