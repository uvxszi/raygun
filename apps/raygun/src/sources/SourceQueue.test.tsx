import type { SourceQueueItem, SourceQueueState, SourceQueueStatus } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const item = (
  id: number,
  address: string,
  state: SourceQueueState,
  overrides: Partial<SourceQueueItem> = {},
): SourceQueueItem => ({
  id,
  address,
  state,
  kind: null,
  format: null,
  server: null,
  name: null,
  url: null,
  configureUrl: null,
  error: null,
  batch: 1,
  ...overrides,
});

const BUSY: SourceQueueState[] = ['waiting', 'checking', 'adding'];

/** Rust's queue holding `items`, counted as Rust counts them. */
const listOf = (items: SourceQueueItem[]): SourceQueueStatus => {
  const count = (state: SourceQueueState) => items.filter((entry) => entry.state === state).length;
  const now = items.find((entry) => entry.state === 'checking' || entry.state === 'adding');
  return {
    batch: 1,
    items,
    total: items.length,
    added: count('added'),
    alreadyAdded: count('alreadyAdded'),
    needsSignIn: count('needsSignIn'),
    needsSetup: count('needsSetup'),
    failed: count('failed'),
    cancelled: count('cancelled'),
    running: items.some((entry) => BUSY.includes(entry.state)),
    paused: false,
    loading: false,
    current: now ? (now.name ?? now.address) : null,
  };
};

const REPO = 'https://example.org/index.json';
const ADDON = 'https://films.example.org/manifest.json';
const SERVER = 'media.example.org';

const openSeveral = async (user: ReturnType<typeof userEvent.setup>) => {
  await AppShellWrapper.mount('/sources');
  const page = await screen.findByTestId('sources-page');
  await user.click(within(page).getByRole('button', { name: 'Add a source' }));
  const sheet = await screen.findByRole('dialog', { name: 'Add a source' });
  await user.click(within(sheet).getByRole('tab', { name: 'Several' }));
  return { page, sheet, field: within(sheet).getByRole('textbox', { name: 'Addresses' }) };
};

/** A row of the list by its name, inside `scope`. */
const row = (scope: HTMLElement, name: string) =>
  within(within(scope).getByRole('list', { name: 'Addresses being added' })).getByRole('listitem', {
    name,
  });

describe('Sources → Add several', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('ends every paste with its line, so the next paste starts a new one', async () => {
    const user = userEvent.setup();
    const { field } = await openSeveral(user);
    await user.click(field);
    await user.paste(REPO);
    await user.paste(ADDON);
    expect(field).toHaveValue(`${REPO}\n${ADDON}\n`);
    // A block that ends with its line break gets no blank line after it.
    await user.paste(`${SERVER}\nother.example.org\n`);
    expect(field).toHaveValue(`${REPO}\n${ADDON}\n${SERVER}\nother.example.org\n`);
    const end = (field as HTMLTextAreaElement).value.length;
    expect((field as HTMLTextAreaElement).selectionStart).toBe(end);
    expect((field as HTMLTextAreaElement).selectionEnd).toBe(end);
  });

  it('queues the list, then follows each address as Rust looks at and adds it', async () => {
    const user = userEvent.setup();
    const waiting = listOf([
      item(1, REPO, 'checking'),
      item(2, ADDON, 'waiting'),
      item(3, SERVER, 'waiting'),
    ]);
    commands.sourcesQueueAdd.mockResolvedValue({
      queued: 3,
      duplicates: 1,
      overLimit: 0,
      status: waiting,
    });
    const { sheet, field } = await openSeveral(user);
    await user.click(field);
    await user.paste(`${REPO}\n${ADDON}\n${SERVER}\n${REPO}\n`);
    await user.click(within(sheet).getByRole('button', { name: 'Add all' }));
    expect(commands.sourcesQueueAdd).toHaveBeenCalledWith(
      `${REPO}\n${ADDON}\n${SERVER}\n${REPO}\n`,
    );
    expect(await within(sheet).findByText(/3 addresses queued/)).toHaveTextContent(
      '1 was already in the list',
    );
    expect(field).toHaveValue('');
    const list = within(sheet).getByRole('region', { name: 'Adding several sources' });
    expect(within(list).getByRole('status')).toHaveTextContent('Adding sources · 1 of 3');
    expect(row(list, REPO)).toHaveTextContent('Looking at it…');
    expect(row(list, ADDON)).toHaveTextContent('Waiting');

    emitEvent(
      'sourceQueueEvent',
      listOf([
        item(1, REPO, 'added', { kind: 'repository', format: 'raygun', name: 'Example sources' }),
        item(2, ADDON, 'checking'),
        item(3, SERVER, 'waiting'),
      ]),
    );
    expect(row(list, 'Example sources')).toHaveTextContent('Raygun repository · Added');
    expect(within(list).getByRole('status')).toHaveTextContent('Adding sources · 2 of 3');

    emitEvent(
      'sourceQueueEvent',
      listOf([
        item(1, REPO, 'added', { kind: 'repository', format: 'raygun', name: 'Example sources' }),
        item(2, ADDON, 'failed', {
          error: "Couldn't reach films.example.org\nconnection refused",
        }),
        item(3, SERVER, 'needsSignIn', {
          kind: 'server',
          server: 'jellyfin',
          name: 'Home Media',
          url: 'https://media.example.org',
        }),
      ]),
    );
    expect(within(list).getByRole('status')).toHaveTextContent(
      '1 added, 1 needs a sign-in, 1 failed',
    );
    expect(row(list, ADDON)).toHaveTextContent('Not added');
    expect(within(row(list, ADDON)).getByRole('alert')).toBeInTheDocument();
    expect(within(list).queryByRole('button', { name: 'Cancel all' })).not.toBeInTheDocument();
    commands.sourcesQueueRetry.mockResolvedValue(
      listOf([
        item(1, REPO, 'added', { kind: 'repository', format: 'raygun', name: 'Example sources' }),
        item(2, ADDON, 'waiting'),
      ]),
    );
    await user.click(within(list).getByRole('button', { name: 'Retry failed' }));
    expect(commands.sourcesQueueRetry).toHaveBeenCalled();
    await waitFor(() => expect(row(list, ADDON)).toHaveTextContent('Waiting'));
    await user.click(within(list).getByRole('button', { name: 'Clear' }));
    expect(commands.sourcesQueueDismiss).toHaveBeenCalledWith(null);
  });

  it('carries on in Sources once the sheet closes, with Cancel all', async () => {
    const user = userEvent.setup();
    const running = listOf([item(1, REPO, 'checking'), item(2, ADDON, 'waiting')]);
    commands.sourcesQueueAdd.mockResolvedValue({
      queued: 2,
      duplicates: 0,
      overLimit: 0,
      status: running,
    });
    commands.sourcesQueueCancel
      .mockResolvedValueOnce(listOf([item(1, REPO, 'checking'), item(2, ADDON, 'cancelled')]))
      .mockResolvedValueOnce(listOf([item(1, REPO, 'cancelled'), item(2, ADDON, 'cancelled')]));
    const { page, sheet, field } = await openSeveral(user);
    await user.type(field, `${REPO}{Enter}${ADDON}`);
    await user.click(within(sheet).getByRole('button', { name: 'Add all' }));
    await within(sheet).findByText(/2 addresses queued/);
    await user.click(within(sheet).getByRole('button', { name: 'Close' }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Add a source' })).not.toBeInTheDocument(),
    );

    const list = within(page).getByRole('region', { name: 'Adding several sources' });
    expect(within(list).getByRole('status')).toHaveTextContent('Adding sources · 1 of 2');
    expect(within(list).getByText(`Now: ${REPO}`)).toBeInTheDocument();
    await user.click(within(row(list, ADDON)).getByRole('button', { name: `Cancel ${ADDON}` }));
    expect(commands.sourcesQueueCancel).toHaveBeenCalledWith(2);
    await waitFor(() => expect(row(list, ADDON)).toHaveTextContent('Cancelled'));
    await user.click(within(list).getByRole('button', { name: 'Cancel all' }));
    expect(commands.sourcesQueueCancel).toHaveBeenLastCalledWith(null);
    expect(await within(list).findByRole('status')).toHaveTextContent('2 cancelled');
  });

  it('shows what Rust is adding when Sources opens', async () => {
    commands.sourcesQueueStatus.mockResolvedValue(
      listOf([item(1, REPO, 'added', { name: 'Example sources' }), item(2, ADDON, 'adding')]),
    );
    await AppShellWrapper.mount('/sources');
    const page = await screen.findByTestId('sources-page');
    const list = await within(page).findByRole('region', { name: 'Adding several sources' });
    expect(within(list).getByRole('status')).toHaveTextContent('Adding sources · 2 of 2');
    expect(row(list, ADDON)).toHaveTextContent('Adding…');
    // One being added finishes: it can't be cancelled.
    expect(
      within(row(list, ADDON)).queryByRole('button', { name: `Cancel ${ADDON}` }),
    ).not.toBeInTheDocument();
  });

  it('signs in to a server of the list, and sets up an addon on its website', async () => {
    const user = userEvent.setup();
    commands.sourcesQueueStatus.mockResolvedValue(
      listOf([
        item(1, SERVER, 'needsSignIn', {
          kind: 'server',
          server: 'jellyfin',
          name: 'Home Media',
          url: 'https://media.example.org',
        }),
        item(2, ADDON, 'needsSetup', {
          kind: 'addon',
          name: 'Films',
          configureUrl: 'https://films.example.org/configure',
        }),
      ]),
    );
    commands.sourcesQueueSignedIn.mockResolvedValue(
      listOf([
        item(1, SERVER, 'added', { kind: 'server', server: 'jellyfin', name: 'Home Media' }),
        item(2, ADDON, 'needsSetup', {
          kind: 'addon',
          name: 'Films',
          configureUrl: 'https://films.example.org/configure',
        }),
      ]),
    );
    await AppShellWrapper.mount('/sources');
    const page = await screen.findByTestId('sources-page');
    const list = await within(page).findByRole('region', { name: 'Adding several sources' });
    const server = row(list, 'Home Media');
    expect(server).toHaveTextContent('Jellyfin · Needs a sign-in');
    await user.click(within(server).getByRole('button', { name: 'Sign in' }));
    await user.type(within(server).getByRole('textbox', { name: 'User name' }), 'viewer');
    await user.type(within(server).getByLabelText('Password'), 'secret');
    await user.click(within(server).getByRole('button', { name: 'Connect' }));
    expect(commands.serversAdd).toHaveBeenCalledWith({
      kind: 'jellyfin',
      address: 'https://media.example.org',
      username: 'viewer',
      secret: 'secret',
      apiKey: false,
    });
    await waitFor(() => expect(commands.sourcesQueueSignedIn).toHaveBeenCalledWith(1));
    await waitFor(() => expect(row(list, 'Home Media')).toHaveTextContent('Jellyfin · Added'));

    await user.click(within(row(list, 'Films')).getByRole('button', { name: 'Set up' }));
    expect(commands.stremioConfigure).toHaveBeenCalledWith('https://films.example.org/configure');
  });

  it('takes a list pasted into the one address field, and a list file chosen beside it', async () => {
    const user = userEvent.setup();
    commands.filesPick.mockResolvedValue(ok({ path: '/lists/sources.txt', name: 'sources.txt' }));
    commands.sourcesQueueReadFile.mockResolvedValue(ok(['c.example.org', 'd.example.org']));
    await AppShellWrapper.mount('/sources');
    const page = await screen.findByTestId('sources-page');
    await user.click(within(page).getByRole('button', { name: 'Add a source' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add a source' });
    await user.click(within(sheet).getByRole('textbox', { name: 'Address' }));
    await user.paste(`${REPO}\n${ADDON}`);
    expect(within(sheet).getByRole('tab', { name: 'Several' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    const field = within(sheet).getByRole('textbox', { name: 'Addresses' });
    expect(field).toHaveValue(`${REPO}\n${ADDON}\n`);

    await user.click(within(sheet).getByRole('button', { name: 'Choose file' }));
    expect(commands.sourcesQueueReadFile).toHaveBeenCalledWith('/lists/sources.txt');
    await waitFor(() =>
      expect(field).toHaveValue(`${REPO}\n${ADDON}\nc.example.org\nd.example.org\n`),
    );
  });

  it('asks for local network access first, and adds the list either way', async () => {
    const user = userEvent.setup();
    commands.sourcesQueueLocalAddress.mockResolvedValue(ok('192.168.1.5:4533'));
    const { sheet, field } = await openSeveral(user);
    await user.type(field, `192.168.1.5:4533{Enter}${REPO}`);
    await user.click(within(sheet).getByRole('button', { name: 'Add all' }));
    const ask = await screen.findByRole('dialog', { name: 'Local network access' });
    expect(commands.sourcesQueueAdd).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole('button', { name: 'Not now' }));
    await waitFor(() =>
      expect(commands.sourcesQueueAdd).toHaveBeenCalledWith(`192.168.1.5:4533\n${REPO}`),
    );
  });
});
