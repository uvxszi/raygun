import type { ServerInfo } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SERVER: ServerInfo = {
  id: 'server.jellyfin.0a1b2c3d4e5f',
  kind: 'jellyfin',
  name: 'Home Media',
  address: 'https://media.example.org:8096',
  username: 'viewer',
  provides: ['video.source', 'music.metadata', 'music.stream', 'lyrics'],
  error: null,
};

const openSection = async () => {
  await AppShellWrapper.mount('/browse?tab=sources');
  return screen.findByRole('region', { name: 'Servers' });
};

describe('Servers', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('connects a server by its address and sign-in, and lists what it brings', async () => {
    const user = userEvent.setup();
    commands.serversAdd.mockResolvedValue(ok([SERVER]));
    const section = await openSection();
    expect(section).toHaveTextContent('No servers yet');

    await user.click(within(section).getByRole('button', { name: 'Add server' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add server' });
    const connect = within(sheet).getByRole('button', { name: 'Connect' });
    expect(within(sheet).getByRole('radio', { name: 'Jellyfin' })).toBeChecked();
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Address' }),
      'media.example.org:8096',
    );
    expect(connect).toBeDisabled();
    await user.type(within(sheet).getByRole('textbox', { name: 'User name' }), 'viewer');
    await user.type(within(sheet).getByLabelText('Password'), 'secret');
    await user.click(connect);
    expect(commands.serversAdd).toHaveBeenCalledWith({
      kind: 'jellyfin',
      address: 'media.example.org:8096',
      username: 'viewer',
      secret: 'secret',
      apiKey: false,
    });
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    const row = await within(section).findByTestId('server-row');
    expect(row).toHaveTextContent('Home Media');
    expect(row).toHaveTextContent('Jellyfin · media.example.org:8096 · viewer');
    const brings = within(row).getByRole('list', { name: 'What it brings' });
    expect(
      within(brings)
        .getAllByRole('listitem')
        .map((chip) => chip.textContent),
    ).toEqual(['Films and series', 'Music', 'Lyrics']);
  });

  it('takes an API key in place of a user, and catalogues without a sign-in', async () => {
    const user = userEvent.setup();
    const section = await openSection();
    await user.click(within(section).getByRole('button', { name: 'Add server' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add server' });

    await user.click(within(sheet).getByRole('radio', { name: 'Subsonic' }));
    await user.click(within(sheet).getByRole('checkbox', { name: 'Use an API key' }));
    expect(within(sheet).queryByRole('textbox', { name: 'User name' })).not.toBeInTheDocument();
    await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), 'music.example.org');
    await user.type(within(sheet).getByLabelText('API key'), 'k-123');
    await user.click(within(sheet).getByRole('button', { name: 'Connect' }));
    expect(commands.serversAdd).toHaveBeenLastCalledWith({
      kind: 'subsonic',
      address: 'music.example.org',
      username: '',
      secret: 'k-123',
      apiKey: true,
    });

    // An open catalogue needs only its address.
    await user.click(within(section).getByRole('button', { name: 'Add server' }));
    const again = await screen.findByRole('dialog', { name: 'Add server' });
    await user.click(within(again).getByRole('radio', { name: 'OPDS' }));
    expect(
      within(again).queryByRole('checkbox', { name: 'Use an API key' }),
    ).not.toBeInTheDocument();
    await user.type(
      within(again).getByRole('textbox', { name: 'Address' }),
      'books.example.org/opds',
    );
    expect(within(again).getByRole('button', { name: 'Connect' })).toBeEnabled();
  });

  it('says why a sign-in failed, in the sheet', async () => {
    const user = userEvent.setup();
    commands.serversAdd.mockResolvedValue({
      status: 'error',
      error: 'The user name or password is wrong',
    });
    const section = await openSection();
    await user.click(within(section).getByRole('button', { name: 'Add server' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add server' });
    await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), 'media.example.org');
    await user.type(within(sheet).getByRole('textbox', { name: 'User name' }), 'viewer');
    await user.click(within(sheet).getByRole('button', { name: 'Connect' }));
    expect(await within(sheet).findByRole('alert')).toHaveTextContent(
      'The user name or password is wrong',
    );
    expect(screen.getByRole('dialog', { name: 'Add server' })).toBeInTheDocument();
  });

  it('signs in again to a server that stopped working, and removes one', async () => {
    const user = userEvent.setup();
    const broken = {
      ...SERVER,
      error: "Raygun no longer has this server's sign-in: sign in again",
    };
    commands.serversList.mockResolvedValue(ok([broken]));
    const section = await openSection();
    const row = await within(section).findByTestId('server-row');
    expect(within(row).getByRole('alert')).toHaveTextContent('sign in again');

    const menu = within(row).getByRole('button', { name: 'Home Media options' });
    await user.click(menu);
    await user.click(screen.getByRole('menuitem', { name: 'Sign in again' }));
    const sheet = await screen.findByRole('dialog', { name: 'Sign in again' });
    expect(within(sheet).getByRole('textbox', { name: 'Address' })).toHaveValue(SERVER.address);
    expect(within(sheet).getByRole('textbox', { name: 'User name' })).toHaveValue('viewer');
    await user.keyboard('{Escape}');

    await user.click(menu);
    await user.click(screen.getByRole('menuitem', { name: 'Remove' }));
    expect(commands.serversRemove).not.toHaveBeenCalled();
    await user.click(screen.getByRole('menuitem', { name: 'Remove Home Media' }));
    expect(commands.serversRemove).toHaveBeenCalledWith(SERVER.id);
  });
});
