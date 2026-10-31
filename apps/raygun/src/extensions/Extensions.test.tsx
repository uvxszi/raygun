import type { CatalogEntry, ExtensionInfo, RepoCatalog } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const extension = (overrides: Partial<ExtensionInfo> = {}): ExtensionInfo => ({
  id: 'dev.raygun.radio-browser',
  name: 'Radio Browser',
  version: '1.0.0',
  description: 'Live internet radio.',
  origin: 'installed',
  status: 'loaded',
  error: null,
  provides: ['music.metadata'],
  net: ['*.api.radio-browser.info'],
  providers: [
    {
      id: 'dev.raygun.radio-browser/stations',
      extensionId: 'dev.raygun.radio-browser',
      kind: 'music.metadata',
      name: 'Radio Browser',
    },
  ],
  settings: [
    {
      key: 'order',
      label: 'Sort stations by',
      kind: 'select',
      options: ['popularity', 'votes'],
      default: 'popularity',
      value: 'popularity',
    },
    { key: 'country', label: 'Country', kind: 'text', options: [], default: '', value: '' },
    { key: 'hd', label: 'HD only', kind: 'boolean', options: [], default: false, value: false },
  ],
  ...overrides,
});

const entry = (overrides: Partial<CatalogEntry> = {}): CatalogEntry => ({
  repoId: 'repo-1',
  id: 'dev.raygun.radio-browser',
  name: 'Radio Browser',
  version: '1.0.0',
  description: 'Live internet radio.',
  author: 'Raygun',
  provides: ['music.metadata'],
  net: ['*.api.radio-browser.info'],
  contentRating: 'safe',
  size: 2219,
  state: 'available',
  installedVersion: null,
  ...overrides,
});

const REPO = {
  id: 'repo-1',
  url: 'https://example.org/repo/index.json',
  name: 'Raygun Extensions',
};

const catalog = (entries: CatalogEntry[], error: string | null = null): RepoCatalog[] => [
  { repo: REPO, entries, error },
];

describe('Extensions', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('summarizes extensions in Settings and links to the extensions page', async () => {
    commands.extensionsList.mockResolvedValue(
      ok([
        extension({ name: 'Internet Archive', origin: 'builtIn' }),
        extension({ id: 'dev.example.broken', name: 'Broken', status: 'failed', error: 'x' }),
      ]),
    );
    await AppShellWrapper.mount('/settings');

    const card = await screen.findByTestId('extensions-card');
    expect(card).toHaveTextContent('Internet Archive, Broken');
    expect(within(card).getByRole('alert')).toHaveTextContent('1 extension failed to load');

    await userEvent.click(card);
    expect(await screen.findByTestId('extensions-page')).toBeInTheDocument();
  });

  it('lists installed extensions and opens one with its permissions and settings', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension()]));
    await AppShellWrapper.mount('/extensions');

    await userEvent.click(await screen.findByTestId('extension-row'));

    expect(await screen.findByTestId('extension-detail-page')).toHaveTextContent(
      '1.0.0 · installed',
    );
    expect(screen.getByTestId('extension-permissions')).toHaveTextContent(
      '*.api.radio-browser.info',
    );
    expect(screen.getByLabelText('Sort stations by')).toHaveValue('popularity');
  });

  it('saves each kind of setting', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension()]));
    // Rust answers with the reloaded list.
    commands.extensionSettingSet.mockResolvedValue(ok([extension()]));
    await AppShellWrapper.mount('/extensions/detail?id=dev.raygun.radio-browser');

    await userEvent.selectOptions(await screen.findByLabelText('Sort stations by'), 'votes');
    expect(commands.extensionSettingSet).toHaveBeenLastCalledWith(
      'dev.raygun.radio-browser',
      'order',
      'votes',
    );

    // Text saves when the field is left, not on every keystroke.
    await userEvent.type(screen.getByLabelText('Country'), 'DE');
    expect(commands.extensionSettingSet).toHaveBeenCalledTimes(1);
    await userEvent.tab();
    expect(commands.extensionSettingSet).toHaveBeenLastCalledWith(
      'dev.raygun.radio-browser',
      'country',
      'DE',
    );

    await userEvent.click(screen.getByRole('switch', { name: 'HD only' }));
    expect(commands.extensionSettingSet).toHaveBeenLastCalledWith(
      'dev.raygun.radio-browser',
      'hd',
      true,
    );
  });

  it('uninstalls only on a second tap', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension()]));
    commands.extensionUninstall.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/extensions/detail?id=dev.raygun.radio-browser');

    await userEvent.click(await screen.findByRole('button', { name: 'Uninstall' }));
    expect(commands.extensionUninstall).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Tap again to uninstall' }));

    expect(commands.extensionUninstall).toHaveBeenCalledWith('dev.raygun.radio-browser');
    expect(await screen.findByTestId('installed-tab')).toBeInTheDocument();
  });

  it('offers no uninstall for built-in extensions', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension({ origin: 'builtIn' })]));
    await AppShellWrapper.mount('/extensions/detail?id=dev.raygun.radio-browser');

    expect(await screen.findByTestId('extension-detail-page')).toHaveTextContent('built-in');
    expect(screen.queryByRole('button', { name: 'Uninstall' })).not.toBeInTheDocument();
  });

  it('installs from a repository after showing the network access it asks for', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry(),
          entry({ id: 'dev.raygun.ia', name: 'Internet Archive', state: 'installed' }),
        ]),
      ),
    );
    commands.extensionInstall.mockResolvedValue(ok([extension()]));
    await AppShellWrapper.mount('/extensions?tab=browse');

    const entries = await screen.findAllByTestId('catalog-entry');
    expect(entries[1]).toHaveTextContent('Installed');
    await userEvent.click(within(entries[0]!).getByRole('button', { name: 'Install' }));

    const dialog = screen.getByRole('dialog', { name: 'Install Radio Browser?' });
    expect(within(dialog).getByTestId('install-permissions')).toHaveTextContent(
      '*.api.radio-browser.info',
    );
    await userEvent.click(within(dialog).getByRole('button', { name: 'Install' }));

    expect(commands.extensionInstall).toHaveBeenCalledWith('repo-1', 'dev.raygun.radio-browser');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('shows updates and install failures', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([entry({ state: 'updateAvailable', version: '1.1.0', installedVersion: '1.0.0' })]),
      ),
    );
    commands.extensionInstall.mockResolvedValue({
      status: 'error',
      error: "package checksum doesn't match the repository index",
    });
    await AppShellWrapper.mount('/extensions?tab=browse');

    const row = await screen.findByTestId('catalog-entry');
    expect(row).toHaveTextContent('1.0.0 → 1.1.0');
    await userEvent.click(within(row).getByRole('button', { name: 'Update' }));
    const dialog = screen.getByRole('dialog', { name: 'Update Radio Browser?' });
    await userEvent.click(within(dialog).getByRole('button', { name: 'Update' }));

    expect(await within(dialog).findByRole('alert')).toHaveTextContent('checksum');
  });

  it('shows a repository that failed to load', async () => {
    commands.extensionCatalog.mockResolvedValue(ok(catalog([], 'could not reach example.org')));
    await AppShellWrapper.mount('/extensions?tab=browse');

    expect(await screen.findByRole('alert')).toHaveTextContent(
      "Couldn't load this repository: could not reach example.org",
    );
  });

  it('adds and removes repositories', async () => {
    commands.extensionRepos.mockResolvedValue(ok([REPO]));
    commands.extensionRepoAdd.mockResolvedValue({
      status: 'error',
      error: 'invalid repository URL: use https',
    });
    await AppShellWrapper.mount('/extensions?tab=repos');

    expect(await screen.findByTestId('repo-row')).toHaveTextContent(REPO.url);
    await userEvent.type(screen.getByLabelText('Repository URL'), 'http://example.org/index.json');
    await userEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(commands.extensionRepoAdd).toHaveBeenCalledWith('http://example.org/index.json');
    expect(await screen.findByRole('alert')).toHaveTextContent('use https');

    await userEvent.click(screen.getByRole('button', { name: 'Remove Raygun Extensions' }));
    expect(commands.extensionRepoRemove).toHaveBeenCalledWith('repo-1');
  });

  it('reloads on request and follows reloads pushed from Rust', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension({ name: 'Before' })]));
    await AppShellWrapper.mount('/extensions');
    expect(await screen.findByTestId('extension-row')).toHaveTextContent('Before');

    await userEvent.click(screen.getByRole('button', { name: 'Reload' }));
    expect(commands.extensionsReload).toHaveBeenCalled();

    emitEvent('extensionsChangedEvent', [extension({ name: 'After' })]);
    expect(await screen.findByTestId('extension-row')).toHaveTextContent('After');
  });
});
