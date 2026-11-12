import type { CatalogEntry, ExtensionInfo, RepoCatalog } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const entry = (overrides: Partial<CatalogEntry> = {}): CatalogEntry => ({
  repoId: 'repo-1',
  id: 'dev.example.radio',
  name: 'Radio Stations',
  version: '1.0.0',
  description: 'Live internet radio.',
  author: 'Raygun',
  provides: ['music.metadata'],
  net: ['*.api.radio.example'],
  contentRating: 'safe',
  adult: false,
  lang: null,
  size: 2219,
  state: 'available',
  installedVersion: null,
  nuclear: false,
  icon: null,
  ...overrides,
});

const MINI_PLAYER = entry({
  id: 'nuclear.nuclear-plugin-miniplayer',
  name: 'Mini Player',
  description: 'A small always-on-top window',
  provides: [],
  nuclear: true,
  unsupported: 'Adds a mini player to Nuclear; Raygun has its own',
  unsupportedKind: 'notForRaygun',
});

const catalog = (entries: CatalogEntry[]): RepoCatalog[] => [
  {
    repo: { id: 'repo-1', url: 'https://example.org/plugins.json', name: 'Nuclear plugins' },
    entries,
    error: null,
  },
];

const extension = (overrides: Partial<ExtensionInfo> = {}): ExtensionInfo => ({
  id: 'dev.example.radio',
  name: 'Radio Stations',
  contentRating: 'safe',
  adult: false,
  version: '1.0.0',
  description: 'Live internet radio.',
  origin: 'installed',
  status: 'loaded',
  error: null,
  provides: ['music.metadata'],
  net: [],
  providers: [
    {
      id: 'dev.example.radio/stations',
      extensionId: 'dev.example.radio',
      kind: 'music.metadata',
      name: 'Radio Stations',
      adult: false,
    },
  ],
  settings: [],
  compatibility: null,
  icon: null,
  website: null,
  repository: 'Paraline',
  ...overrides,
});

describe('Not for Raygun', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('says why in the catalogue instead of offering Install, and leaves it out of Not installed', async () => {
    const user = userEvent.setup();
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry(),
          MINI_PLAYER,
          entry({
            id: 'kodi.plugin.video.example',
            name: 'Kodi Films',
            unsupported: 'Kodi add-ons are Python programs, which Raygun can’t run yet',
            unsupportedKind: 'cantRun',
          }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    const [radio, miniPlayer, kodi] = await screen.findAllByTestId('catalog-entry');
    expect(within(radio!).getByRole('button', { name: 'Install' })).toBeInTheDocument();
    expect(miniPlayer).toHaveTextContent('Not for Raygun');
    expect(miniPlayer).toHaveTextContent('Adds a mini player to Nuclear; Raygun has its own');
    expect(within(miniPlayer!).queryByRole('button', { name: 'Install' })).toBeNull();
    expect(within(miniPlayer!).queryByRole('checkbox')).toBeNull();
    // Another app's add-on Raygun can't run yet says so, as before.
    expect(kodi).toHaveTextContent("Can't install");
    expect(kodi).not.toHaveTextContent('Not for Raygun');
    // Select all takes only what can be installed.
    expect(screen.queryByRole('button', { name: /Select all/ })).toBeNull();

    await user.click(within(miniPlayer!).getByRole('button', { name: 'About Mini Player' }));
    expect(screen.getByRole('dialog', { name: 'About this extension' })).toHaveTextContent(
      'Not for Raygun: Adds a mini player to Nuclear; Raygun has its own',
    );
    await user.click(screen.getByRole('button', { name: 'Close' }));

    await user.click(screen.getByTestId('extensions-tab-notInstalled'));
    await vi.waitFor(() => expect(screen.queryByText('Mini Player')).toBeNull());
    expect(screen.getByText('Radio Stations')).toBeInTheDocument();
  });

  it('marks an installed one that does nothing and removes it with Remove', async () => {
    const user = userEvent.setup();
    commands.extensionsList.mockResolvedValue(
      ok([
        extension(),
        extension({
          id: 'dev.example.nothing',
          name: 'Nothing Here',
          description: 'A theme',
          provides: [],
          providers: [],
          notForRaygun: 'Provides nothing Raygun can call',
        }),
      ]),
    );
    commands.extensionUninstall.mockResolvedValue(ok([extension()]));
    await AppShellWrapper.mount('/extensions');

    const rows = await screen.findAllByTestId('extension-row');
    const nothing = rows.find((row) => row.textContent?.includes('Nothing Here'))!;
    const item = nothing.closest('li')!;
    expect(item).toHaveTextContent('Not for Raygun: Provides nothing Raygun can call');
    expect(within(item).queryByRole('switch')).toBeNull();
    const radio = rows.find((row) => row.textContent?.includes('Radio Stations'))!;
    expect(within(radio.closest('li')!).getByRole('switch')).toBeInTheDocument();

    await user.click(within(item).getByRole('button', { name: 'Remove Nothing Here' }));
    expect(commands.extensionUninstall).toHaveBeenCalledWith('dev.example.nothing');
  });
});
