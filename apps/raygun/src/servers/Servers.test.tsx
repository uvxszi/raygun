import type { DetectedSource, ServerInfo, ServerKind } from '@raygun/bindings';
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

const found = (server: ServerKind, url: string, name: string): DetectedSource => ({
  kind: 'server',
  format: null,
  server,
  url,
  name,
  description: null,
  added: false,
  addon: null,
  entries: [],
  note: null,
});

/** Sources → Add, with what the address turned out to be. */
const lookUp = async (user: ReturnType<typeof userEvent.setup>, address: string) => {
  await AppShellWrapper.mount('/sources?type=servers');
  const page = await screen.findByTestId('sources-page');
  await user.click(within(page).getByRole('button', { name: 'Add a source' }));
  const sheet = await screen.findByRole('dialog', { name: 'Add a source' });
  await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), address);
  await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
  return { page, sheet };
};

describe('Servers', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('connects a server found at its address, and lists what it brings', async () => {
    const user = userEvent.setup();
    commands.sourcesDetect.mockResolvedValue(
      ok(found('jellyfin', 'https://media.example.org:8096', 'Home Media')),
    );
    commands.serversAdd.mockResolvedValue(ok([SERVER]));
    const { page, sheet } = await lookUp(user, 'media.example.org:8096');

    const card = await within(sheet).findByRole('region', {
      name: 'Looks like a Jellyfin server: Home Media',
    });
    const connect = within(card).getByRole('button', { name: 'Connect' });
    expect(connect).toBeDisabled();
    await user.type(within(card).getByRole('textbox', { name: 'User name' }), 'viewer');
    await user.type(within(card).getByLabelText('Password'), 'secret');
    await user.click(connect);
    expect(commands.serversAdd).toHaveBeenCalledWith({
      kind: 'jellyfin',
      address: 'https://media.example.org:8096',
      username: 'viewer',
      secret: 'secret',
      apiKey: false,
    });
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    const row = await within(page).findByTestId('server-row');
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
    commands.sourcesDetect.mockResolvedValue(
      ok(found('subsonic', 'https://music.example.org', 'music.example.org')),
    );
    const { page, sheet } = await lookUp(user, 'music.example.org');
    const card = await within(sheet).findByRole('region', {
      name: 'Looks like a Subsonic server: music.example.org',
    });
    await user.click(within(card).getByRole('checkbox', { name: 'Use an API key' }));
    expect(within(card).queryByRole('textbox', { name: 'User name' })).not.toBeInTheDocument();
    await user.type(within(card).getByLabelText('API key'), 'k-123');
    await user.click(within(card).getByRole('button', { name: 'Connect' }));
    expect(commands.serversAdd).toHaveBeenLastCalledWith({
      kind: 'subsonic',
      address: 'https://music.example.org',
      username: '',
      secret: 'k-123',
      apiKey: true,
    });

    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    // An open catalogue needs only its address.
    commands.sourcesDetect.mockResolvedValue(
      ok(found('opds', 'https://books.example.org/opds', 'Library')),
    );
    await user.click(within(page).getByRole('button', { name: 'Add a source' }));
    const again = await screen.findByRole('dialog', { name: 'Add a source' });
    await user.type(
      within(again).getByRole('textbox', { name: 'Address' }),
      'books.example.org/opds',
    );
    await user.click(within(again).getByRole('button', { name: 'Look up' }));
    const catalogue = await within(again).findByRole('region', {
      name: 'Looks like an OPDS catalogue: Library',
    });
    expect(
      within(catalogue).queryByRole('checkbox', { name: 'Use an API key' }),
    ).not.toBeInTheDocument();
    expect(within(catalogue).getByRole('button', { name: 'Connect' })).toBeEnabled();
  });

  it('says why a sign-in failed, in the sheet', async () => {
    const user = userEvent.setup();
    commands.sourcesDetect.mockResolvedValue(
      ok(found('jellyfin', 'https://media.example.org', 'Home Media')),
    );
    commands.serversAdd.mockResolvedValue({
      status: 'error',
      error: 'The user name or password is wrong',
    });
    const { sheet } = await lookUp(user, 'media.example.org');
    const card = await within(sheet).findByRole('region', {
      name: 'Looks like a Jellyfin server: Home Media',
    });
    await user.type(within(card).getByRole('textbox', { name: 'User name' }), 'viewer');
    await user.click(within(card).getByRole('button', { name: 'Connect' }));
    expect(await within(card).findByRole('alert')).toHaveTextContent(
      'The user name or password is wrong',
    );
    expect(screen.getByRole('dialog', { name: 'Add a source' })).toBeInTheDocument();
  });

  it('connects a server by hand when nothing is found at its address', async () => {
    const user = userEvent.setup();
    commands.sourcesDetect.mockResolvedValue({
      status: 'error',
      error: 'Raygun found no repository, addon, server or extension at media.example.org',
    });
    const { sheet } = await lookUp(user, 'media.example.org');
    await user.click(
      await within(sheet).findByRole('button', { name: 'Connect a server by hand' }),
    );
    const byHand = await screen.findByRole('dialog', { name: 'Add server' });
    expect(within(byHand).getByRole('radio', { name: 'Jellyfin' })).toBeChecked();
    await user.click(within(byHand).getByRole('radio', { name: 'Emby' }));
    await user.type(within(byHand).getByRole('textbox', { name: 'Address' }), 'media.example.org');
    await user.type(within(byHand).getByRole('textbox', { name: 'User name' }), 'viewer');
    await user.click(within(byHand).getByRole('button', { name: 'Connect' }));
    expect(commands.serversAdd).toHaveBeenLastCalledWith({
      kind: 'emby',
      address: 'media.example.org',
      username: 'viewer',
      secret: '',
      apiKey: false,
    });
  });

  it('signs in again to a server that stopped working, and removes one', async () => {
    const user = userEvent.setup();
    const broken = {
      ...SERVER,
      error: "Raygun no longer has this server's sign-in: sign in again",
    };
    commands.serversList.mockResolvedValue(ok([broken]));
    await AppShellWrapper.mount('/sources?type=servers');
    const section = await screen.findByRole('region', { name: 'Servers' });
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
