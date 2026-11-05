import type { DataUsage } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const GB = 1024 * 1024 * 1024;

const usage: DataUsage[] = [
  {
    category: 'downloads',
    counts: [
      { item: 'songs', count: 120, bytes: null },
      { item: 'waiting', count: 3, bytes: null },
    ],
    bytes: 2 * GB,
    scope: 'device',
  },
  {
    category: 'playlists',
    counts: [
      { item: 'playlists', count: 2, bytes: null },
      { item: 'favorites', count: 10, bytes: null },
    ],
    bytes: 0,
    scope: 'synced',
  },
  { category: 'searches', counts: [], bytes: 0, scope: 'device' },
];

describe('Data categories', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.dataUsage.mockResolvedValue(ok(usage));
  });

  it('lists what each kind holds, its size and where clearing reaches', async () => {
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('data-card');

    const rows = await within(card).findAllByTestId('data-row');
    expect(rows.map((row) => row.getAttribute('aria-label'))).toEqual([
      'Playlists and favorites',
      'Search history',
      'Downloads',
    ]);
    const playlists = within(card).getByRole('listitem', { name: 'Playlists and favorites' });
    expect(playlists).toHaveTextContent('2 playlists · 10 favorites');
    expect(playlists).toHaveTextContent('Linked devices clear it too');
    const downloads = within(card).getByRole('listitem', { name: 'Downloads' });
    expect(downloads).toHaveTextContent('120 songs · 3 waiting or failed · 2.0 GB');
    expect(downloads).toHaveTextContent('This device only');
    // Nothing to clear.
    expect(within(card).getByRole('button', { name: 'Clear Search history' })).toBeDisabled();
  });

  it('clears a synced kind after saying linked devices remove it too', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('data-card');

    await user.click(
      await within(card).findByRole('button', { name: 'Clear Playlists and favorites' }),
    );
    const sheet = await screen.findByTestId('confirm-sheet');
    expect(
      screen.getByRole('heading', { name: 'Clear Playlists and favorites?' }),
    ).toBeInTheDocument();
    expect(sheet).toHaveTextContent('Linked devices remove it too the next time they sync.');
    expect(commands.dataClear).not.toHaveBeenCalled();
    await user.click(within(sheet).getByRole('button', { name: 'Clear' }));
    expect(commands.dataClear).toHaveBeenCalledWith('playlists', null);
  });

  it('asks whether downloads lose their files or only the entries without one', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('data-card');

    await user.click(await within(card).findByRole('button', { name: 'Clear Downloads' }));
    const choices = await screen.findByTestId('clear-downloads');
    expect(choices).toHaveTextContent('Copies saved outside Raygun stay.');
    await user.click(within(choices).getByRole('button', { name: /Clear waiting and failed/ }));

    const sheet = await screen.findByTestId('confirm-sheet');
    expect(
      screen.getByRole('heading', { name: 'Clear downloads waiting or failed?' }),
    ).toBeInTheDocument();
    expect(sheet).toHaveTextContent('Only on this device');
    await user.click(within(sheet).getByRole('button', { name: 'Clear' }));
    expect(commands.dataClear).toHaveBeenCalledWith('downloads', 'records');
  });
});
