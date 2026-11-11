import type { MangaSource, SourceSearchResult, SourceSetup } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { useSourceHealth } from '../search/sourceHealth';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { sourceExtension } from '../test/sourceExtension';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const source = (id: string, name: string): MangaSource => ({
  id,
  name,
  extensionId: id.split('/')[0] ?? id,
  enabled: true,
});
const result = (from: MangaSource, titles: string[], error: string | null = null) =>
  ({
    source: from,
    items: titles.map((title, index) => ({
      sourceId: from.id,
      id: `${from.id}-${index}`,
      title,
      coverUrl: null,
      inLibrary: false,
    })),
    hasNextPage: false,
    error,
  }) satisfies SourceSearchResult;

const ROYAL = source('x.royal/novel', 'Royal Road');
const BROKEN = source('x.broken/novel', 'Broken Site');
const KEYED = source('x.keyed/novel', 'Keyed Books');
const LAZY = source('x.lazy/novel', 'Lazy Books');

const waiting = (
  extensionId: string,
  name: string,
  kinds: string[],
  need: SourceSetup['need'],
  server = false,
): SourceSetup => ({ extensionId, name, kinds, need, server });

const KEYED_SETUP = waiting('x.keyed', 'Keyed Books', ['novel.source'], {
  reason: 'settings',
  missing: ['API key', 'API secret'],
  detail: null,
});
const SERVER_SETUP = waiting(
  'server.subsonic.abc',
  'Home Music',
  ['music.metadata'],
  { reason: 'signIn', missing: [], detail: null },
  true,
);

const searchedIds = () =>
  commands.catalogGlobalSearch.mock.calls.flatMap((call) => call[2] as string[]);

describe('Sources that need a key or sign-in', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    localStorage.clear();
    useSourceHealth.setState({ failures: {} });
  });

  it('are counted in one collapsed line, asked nothing, each leading to its settings', async () => {
    const user = userEvent.setup();
    commands.novelSources.mockResolvedValue(ok([BROKEN, KEYED, ROYAL]));
    commands.extensionsNeedingSetup.mockResolvedValue(ok([KEYED_SETUP, SERVER_SETUP]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources) => {
      const id = sources[0];
      if (id === ROYAL.id) return ok([result(ROYAL, ['Mother of Learning'])]);
      return ok([result(BROKEN, [], 'HTTP 503')]);
    });
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother');

    const note = await screen.findByTestId('needs-setup');
    // The server only answers music: not counted with books.
    const line = within(note).getByRole('button', { name: '1 source needs a key or sign-in' });
    expect(line).toHaveAttribute('aria-expanded', 'false');
    expect(within(note).queryByRole('link')).not.toBeInTheDocument();
    await user.click(line);
    expect(line).toHaveAttribute('aria-expanded', 'true');
    const settings = within(note).getByRole('link', { name: /Keyed Books/ });
    expect(settings).toHaveTextContent('API key, API secret not set');
    expect(settings).toHaveAttribute('href', '/extensions/detail?id=x.keyed');

    // The other failure is still named, without the one waiting for its key.
    const failed = await screen.findByTestId('failed-sources');
    expect(failed).toHaveTextContent('Broken Site is having problems');
    expect(failed).not.toHaveTextContent('Keyed Books');
    expect(searchedIds()).not.toContain(KEYED.id);
    expect(searchedIds()).toContain(ROYAL.id);

    // Once it's set up, the line goes.
    emitEvent('sourcesSetupEvent', [SERVER_SETUP]);
    await vi.waitFor(() => expect(screen.queryByTestId('needs-setup')).not.toBeInTheDocument());
  });

  it("aren't failures when their own answer says so", async () => {
    const user = userEvent.setup();
    commands.novelSources.mockResolvedValue(ok([LAZY, ROYAL]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources) => {
      if (sources[0] === ROYAL.id) return ok([result(ROYAL, ['Mother of Learning'])]);
      return ok([
        result(
          LAZY,
          [],
          'Lazy Books needs a key or sign-in\nneeds setup (x.lazy): the API refused the key',
        ),
      ]);
    });
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother');
    await screen.findAllByTestId('manga-card');
    await vi.waitFor(() => expect(searchedIds()).toContain(LAZY.id));
    // Not listed as failing, nor remembered as failing.
    expect(screen.queryByTestId('failed-sources')).not.toBeInTheDocument();
    expect(useSourceHealth.getState().failures).not.toHaveProperty(LAZY.id);

    // Rust says it waits for its key: counted with the others.
    emitEvent('sourcesSetupEvent', [
      waiting('x.lazy', 'Lazy Books', ['novel.source'], {
        reason: 'refused',
        missing: [],
        detail: 'the API refused the key',
      }),
    ]);
    const note = await screen.findByTestId('needs-setup');
    await user.click(within(note).getByRole('button', { name: '1 source needs a key or sign-in' }));
    expect(within(note).getByRole('link', { name: /Lazy Books/ })).toHaveTextContent(
      'Key or sign-in refused',
    );
  });

  it('send servers to Sources to sign in again', async () => {
    const user = userEvent.setup();
    commands.extensionsNeedingSetup.mockResolvedValue(
      ok([
        waiting(
          'server.jellyfin.abc',
          'Home Media',
          ['novel.source'],
          {
            reason: 'signIn',
            missing: [],
            detail: null,
          },
          true,
        ),
      ]),
    );
    commands.novelSources.mockResolvedValue(ok([ROYAL]));
    commands.catalogGlobalSearch.mockResolvedValue(ok([result(ROYAL, ['Mother of Learning'])]));
    await AppShellWrapper.mount('/browse?tab=search&type=novel&q=mother');
    const note = await screen.findByTestId('needs-setup');
    await user.click(within(note).getByRole('button'));
    const signIn = within(note).getByRole('link', { name: /Home Media/ });
    expect(signIn).toHaveTextContent('Sign in again');
    expect(signIn).toHaveAttribute('href', '/sources?type=servers');
  });

  it('say on their own page what they wait for, marking the settings they need', async () => {
    commands.extensionsList.mockResolvedValue(
      ok([
        {
          ...sourceExtension(['podcast.directory'], 'x.keyed', 'Keyed Books'),
          needsSetup: { reason: 'settings', missing: ['API key'], detail: null },
          settings: [
            {
              key: 'apiKey',
              label: 'API key',
              kind: 'text',
              options: [],
              default: null,
              value: null,
              required: true,
            },
          ],
        },
      ]),
    );
    await AppShellWrapper.mount('/extensions/detail?id=x.keyed');
    const waits = await screen.findByTestId('extension-needs-setup');
    expect(waits).toHaveTextContent('Asked nothing until it’s set up');
    expect(waits).toHaveTextContent('API key not set');
    expect(within(screen.getByTestId('extension-settings')).getByText('Needed')).toBeVisible();
  });

  it("don't count against a source's health", () => {
    const health = useSourceHealth.getState();
    health.failed(
      'x.keyed/novel',
      'Keyed needs a key or sign-in\nneeds setup (x.keyed): API key not set',
    );
    health.failed('x.broken/novel', 'HTTP 503');
    expect(Object.keys(useSourceHealth.getState().failures)).toEqual(['x.broken/novel']);
  });
});
