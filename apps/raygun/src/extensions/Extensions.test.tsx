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
  contentRating: 'safe',
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
  compatibility: null,
  icon: null,
  website: null,
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
  lang: null,
  size: 2219,
  state: 'available',
  installedVersion: null,
  nuclear: false,
  icon: null,
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
    await AppShellWrapper.mount('/settings/browse');

    const card = await screen.findByTestId('extensions-card');
    expect(card).toHaveTextContent('Internet Archive, Broken');
    expect(within(card).getByRole('alert')).toHaveTextContent('1 extension failed to load');

    await userEvent.click(card);
    expect(await screen.findByTestId('browse-page')).toBeInTheDocument();
    expect(screen.getByTestId('browse-tab-extensions')).toHaveAttribute('aria-current', 'page');
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

  it('installs from a repository at once, with what it may access behind the info button', async () => {
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
    await userEvent.click(within(entries[0]!).getByRole('button', { name: 'About Radio Browser' }));
    const info = screen.getByRole('dialog', { name: 'About this extension' });
    expect(within(info).getByTestId('install-permissions')).toHaveTextContent(
      '*.api.radio-browser.info',
    );
    await userEvent.click(screen.getByRole('button', { name: 'Close' }));

    await userEvent.click(within(entries[0]!).getByRole('button', { name: 'Install' }));
    expect(commands.extensionInstall).toHaveBeenCalledWith('repo-1', 'dev.raygun.radio-browser');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('installs a selection, one after another', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry(),
          entry({ id: 'dev.raygun.ia', name: 'Internet Archive' }),
          entry({ id: 'dev.raygun.mb', name: 'MusicBrainz' }),
        ]),
      ),
    );
    commands.extensionInstall.mockResolvedValue(ok([extension()]));
    await AppShellWrapper.mount('/extensions?tab=browse');
    await screen.findAllByTestId('catalog-entry');

    await userEvent.click(screen.getByRole('checkbox', { name: 'Select Radio Browser' }));
    const bar = screen.getByRole('toolbar', { name: 'Selected extensions' });
    expect(bar).toHaveTextContent('1 selected');
    await userEvent.click(within(bar).getByRole('button', { name: 'Select all 3' }));
    await userEvent.click(within(bar).getByRole('button', { name: 'Install 3' }));

    await vi.waitFor(() => expect(commands.extensionInstall).toHaveBeenCalledTimes(3));
    expect(commands.extensionInstall.mock.calls.map(([, id]) => id)).toEqual([
      'dev.raygun.radio-browser',
      'dev.raygun.ia',
      'dev.raygun.mb',
    ]);
    expect(screen.queryByRole('toolbar')).not.toBeInTheDocument();
  });

  it('says a Shosetsu script may reach any public website', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry({
            id: 'shosetsu.en.novelfull',
            name: 'NovelFull',
            provides: ['novel.source'],
            net: ['*'],
          }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    const [novelFull] = await screen.findAllByTestId('catalog-entry');
    await userEvent.click(within(novelFull!).getByRole('button', { name: 'About NovelFull' }));
    const dialog = screen.getByRole('dialog', { name: 'About this extension' });
    expect(within(dialog).getByTestId('install-permissions')).toHaveTextContent(
      'Any public website (never this device or your local network)',
    );
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

    expect(await within(row).findByRole('alert')).toHaveTextContent('checksum');
    expect(within(row).getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('marks Nuclear plugins and says their network access is detected at install', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry({ id: 'nuclear.nuclear-plugin-lrclib', name: 'LRCLIB', net: [], nuclear: true }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    const row = await screen.findByTestId('catalog-entry');
    expect(row).toHaveTextContent('Nuclear');
    await userEvent.click(within(row).getByRole('button', { name: 'About LRCLIB' }));
    expect(screen.getByRole('dialog')).toHaveTextContent('converts it when installing');
  });

  it('offers plugins converted by an older converter again', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry({
            id: 'nuclear.nuclear-plugin-lrclib',
            name: 'LRCLIB',
            nuclear: true,
            state: 'updateAvailable',
            version: '1.0.0',
            installedVersion: '1.0.0',
          }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    const row = await screen.findByTestId('catalog-entry');
    expect(row).toHaveTextContent('1.0.0, converted again with fixes');
    expect(within(row).getByRole('button', { name: 'Update' })).toBeInTheDocument();
  });

  it("shows a converted Nuclear plugin's compatibility report", async () => {
    commands.extensionsList.mockResolvedValue(
      ok([
        extension({
          compatibility: {
            package: 'nuclear-plugin-sample',
            converter: 2,
            supported: ['lyrics'],
            unsupportedCategories: ['dashboard'],
            stubbedApis: ['Queue'],
            uiImports: ['react'],
          },
        }),
      ]),
    );
    await AppShellWrapper.mount('/extensions/detail?id=dev.raygun.radio-browser');

    const report = await screen.findByTestId('extension-compatibility');
    expect(report).toHaveTextContent('Converted from the Nuclear plugin nuclear-plugin-sample.');
    expect(report).toHaveTextContent('Not supported in Raygun: dashboard.');
    expect(report).toHaveTextContent('unavailable, so parts of the plugin may fail: Queue.');
    expect(report).toHaveTextContent("Its screens and widgets don't show in Raygun.");
  });

  it('shows a repository that failed to load', async () => {
    commands.extensionCatalog.mockResolvedValue(ok(catalog([], 'could not reach example.org')));
    await AppShellWrapper.mount('/extensions?tab=browse');

    const issue = await screen.findByRole('alert');
    expect(issue).toHaveTextContent('Couldn’t reach');
    expect(issue).toHaveTextContent('could not reach example.org');
  });

  it('adds and removes repositories', async () => {
    commands.extensionRepos.mockResolvedValue(ok([REPO]));
    commands.extensionRepoAdd.mockResolvedValue({
      status: 'error',
      error: 'invalid repository URL: use https',
    });
    await AppShellWrapper.mount('/extensions?tab=repos');

    expect(await screen.findByTestId('repo-row')).toHaveTextContent(REPO.url);
    // Adding is the top bar's "+", which opens a sheet.
    await userEvent.click(screen.getByRole('button', { name: 'Add repository' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add repository' });
    await userEvent.type(
      within(sheet).getByLabelText('Repository URL'),
      'http://example.org/index.json',
    );
    await userEvent.click(within(sheet).getByRole('button', { name: 'Add repository' }));
    expect(commands.extensionRepoAdd).toHaveBeenCalledWith('http://example.org/index.json');
    expect(await within(sheet).findByRole('alert')).toHaveTextContent('use https');
    await userEvent.click(within(sheet).getByRole('button', { name: 'Close' }));

    await userEvent.click(screen.getByRole('button', { name: 'Remove Raygun Extensions' }));
    expect(commands.extensionRepoRemove).toHaveBeenCalledWith('repo-1');
  });

  it('reloads on request and follows reloads pushed from Rust', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension({ name: 'Before' })]));
    await AppShellWrapper.mount('/extensions');
    expect(await screen.findByTestId('extension-row')).toHaveTextContent('Before');

    await userEvent.click(screen.getByRole('button', { name: 'More options' }));
    await userEvent.click(screen.getByRole('menuitem', { name: 'Reload' }));
    expect(commands.extensionsReload).toHaveBeenCalled();

    emitEvent('extensionsChangedEvent', [extension({ name: 'After' })]);
    expect(await screen.findByTestId('extension-row')).toHaveTextContent('After');
  });

  it('filters the catalog by text, kind, language and repository', async () => {
    const user = userEvent.setup();
    const shosetsu = { id: 'repo-2', url: 'https://example.org/s/index.json', name: 'Shosetsu' };
    commands.extensionCatalog.mockResolvedValue(
      ok([
        ...catalog([
          entry(),
          entry({ id: 'dev.raygun.md', name: 'MangaDex', provides: ['manga.source'] }),
        ]),
        {
          repo: shosetsu,
          error: null,
          entries: [
            entry({
              repoId: 'repo-2',
              id: 's.en',
              name: 'NovelFull',
              provides: ['novel.source'],
              lang: 'en',
            }),
            entry({
              repoId: 'repo-2',
              id: 's.ja',
              name: 'Kakuyomu',
              provides: ['novel.source'],
              lang: 'ja',
            }),
          ],
        },
      ]),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');
    const names = async () =>
      (await screen.findAllByTestId('catalog-entry')).map(
        (row) => within(row).getByText(/^\S/, { selector: '.font-medium' }).textContent,
      );
    expect(await names()).toEqual(['Radio Browser', 'MangaDex', 'NovelFull', 'Kakuyomu']);

    const kinds = screen.getByRole('group', { name: 'Show' });
    await user.click(within(kinds).getByRole('button', { name: 'Books' }));
    expect(await names()).toEqual(['NovelFull', 'Kakuyomu']);
    await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), 'ja');
    expect(await names()).toEqual(['Kakuyomu']);

    await user.click(within(kinds).getByRole('button', { name: 'All' }));
    await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), '');
    await user.selectOptions(screen.getByRole('combobox', { name: 'Repository' }), 'repo-1');
    expect(await names()).toEqual(['Radio Browser', 'MangaDex']);
    await user.type(screen.getByRole('searchbox', { name: 'Search extensions' }), 'manga');
    expect(await names()).toEqual(['MangaDex']);
    await user.type(screen.getByRole('searchbox', { name: 'Search extensions' }), 'zzz');
    expect(await screen.findByText('No extensions match.')).toBeInTheDocument();
  });

  it('adds a suggested repository with one tap', async () => {
    const user = userEvent.setup();
    commands.extensionSuggestedRepos.mockResolvedValue(
      ok([
        { url: REPO.url, name: 'Raygun Extensions', description: 'Official.', added: true },
        {
          url: 'https://example.org/s/index.json',
          name: 'Shosetsu extensions',
          description: 'Novel sources.',
          added: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/extensions?tab=repos');

    const official = await screen.findByRole('listitem', { name: 'Raygun Extensions' });
    expect(within(official).getByText('Added')).toBeInTheDocument();
    const shosetsu = screen.getByRole('listitem', { name: 'Shosetsu extensions' });
    await user.click(within(shosetsu).getByRole('button', { name: 'Add Shosetsu extensions' }));
    expect(commands.extensionRepoAdd).toHaveBeenCalledWith('https://example.org/s/index.json');
    expect(
      screen.getByText(/doesn’t review extensions from other repositories/),
    ).toBeInTheDocument();
  });
});
