import type {
  CatalogEntry,
  ExtensionInfo,
  ExtensionInstall,
  ExtensionInstallStatus,
  RepoCatalog,
} from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { NO_INSTALLS, bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const extension = (overrides: Partial<ExtensionInfo> = {}): ExtensionInfo => ({
  id: 'dev.example.radio',
  name: 'Radio Stations',
  contentRating: 'safe',
  version: '1.0.0',
  description: 'Live internet radio.',
  origin: 'installed',
  status: 'loaded',
  error: null,
  provides: ['music.metadata'],
  net: ['*.api.radio.example'],
  providers: [
    {
      id: 'dev.example.radio/stations',
      extensionId: 'dev.example.radio',
      kind: 'music.metadata',
      name: 'Radio Stations',
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
  repository: null,
  ...overrides,
});

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
  lang: null,
  size: 2219,
  state: 'available',
  installedVersion: null,
  nuclear: false,
  icon: null,
  ...overrides,
});

const install = (overrides: Partial<ExtensionInstall> = {}): ExtensionInstall => ({
  repoId: 'repo-1',
  extensionId: 'dev.example.radio',
  name: 'Radio Stations',
  state: 'queued',
  phase: null,
  error: null,
  finishedAt: null,
  batch: 1,
  ...overrides,
});

const installs = (overrides: Partial<ExtensionInstallStatus> = {}): ExtensionInstallStatus => ({
  ...NO_INSTALLS,
  batch: 1,
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
        extension({ name: 'Internet Archive', repository: 'Paraline' }),
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
      '1.0.0 · added by you',
    );
    expect(screen.getByTestId('extension-permissions')).toHaveTextContent('*.api.radio.example');
    expect(screen.getByLabelText('Sort stations by')).toHaveValue('popularity');
  });

  it('saves each kind of setting', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension()]));
    // Rust answers with the reloaded list.
    commands.extensionSettingSet.mockResolvedValue(ok([extension()]));
    await AppShellWrapper.mount('/extensions/detail?id=dev.example.radio');

    await userEvent.selectOptions(await screen.findByLabelText('Sort stations by'), 'votes');
    expect(commands.extensionSettingSet).toHaveBeenLastCalledWith(
      'dev.example.radio',
      'order',
      'votes',
    );

    // Text saves when the field is left, not on every keystroke.
    await userEvent.type(screen.getByLabelText('Country'), 'DE');
    expect(commands.extensionSettingSet).toHaveBeenCalledTimes(1);
    await userEvent.tab();
    expect(commands.extensionSettingSet).toHaveBeenLastCalledWith(
      'dev.example.radio',
      'country',
      'DE',
    );

    await userEvent.click(screen.getByRole('switch', { name: 'HD only' }));
    expect(commands.extensionSettingSet).toHaveBeenLastCalledWith('dev.example.radio', 'hd', true);
  });

  it('turns an extension off with its switch, and many at once with Select all', async () => {
    commands.extensionsList.mockResolvedValue(
      ok([extension(), extension({ id: 'dev.example.books', name: 'Free Books' })]),
    );
    commands.settingsUpdate.mockClear();
    await AppShellWrapper.mount('/extensions');

    await userEvent.click(await screen.findByRole('switch', { name: 'Use Radio Stations' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'disabledSources',
      value: ['dev.example.radio'],
    });

    await userEvent.click(screen.getByRole('button', { name: 'Select' }));
    const bar = screen.getByRole('toolbar', { name: 'Selected' });
    await userEvent.click(within(bar).getByRole('checkbox', { name: 'Select all' }));
    expect(bar).toHaveTextContent('2 selected');
    await userEvent.click(within(bar).getByRole('button', { name: 'Turn off' }));
    const last = commands.settingsUpdate.mock.lastCall?.[0] as { value: string[] };
    expect([...last.value].sort()).toEqual(['dev.example.books', 'dev.example.radio']);
  });

  it('uninstalls only on a second tap', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension()]));
    commands.extensionUninstall.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/extensions/detail?id=dev.example.radio');

    await userEvent.click(await screen.findByRole('button', { name: 'Uninstall' }));
    expect(commands.extensionUninstall).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Tap again to uninstall' }));

    expect(commands.extensionUninstall).toHaveBeenCalledWith('dev.example.radio');
    expect(await screen.findByTestId('installed-tab')).toBeInTheDocument();
  });

  it('names the repository an extension came from, never "built-in"', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension({ repository: 'Paraline' })]));
    await AppShellWrapper.mount('/extensions/detail?id=dev.example.radio');

    const page = await screen.findByTestId('extension-detail-page');
    expect(page).toHaveTextContent('Paraline');
    expect(page).not.toHaveTextContent(/built-in/i);
    expect(screen.getByRole('button', { name: 'Uninstall' })).toBeInTheDocument();
  });

  it('says an extension added on its own was added by the user', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension({ repository: null })]));
    await AppShellWrapper.mount('/extensions/detail?id=dev.example.radio');

    expect(await screen.findByTestId('extension-detail-page')).toHaveTextContent('added by you');
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
    await AppShellWrapper.mount('/extensions?tab=browse');

    const entries = await screen.findAllByTestId('catalog-entry');
    expect(entries[1]).toHaveTextContent('Installed');
    await userEvent.click(
      within(entries[0]!).getByRole('button', { name: 'About Radio Stations' }),
    );
    const info = screen.getByRole('dialog', { name: 'About this extension' });
    expect(within(info).getByTestId('install-permissions')).toHaveTextContent(
      '*.api.radio.example',
    );
    await userEvent.click(screen.getByRole('button', { name: 'Close' }));

    await userEvent.click(within(entries[0]!).getByRole('button', { name: 'Install' }));
    expect(commands.extensionInstallsEnqueue).toHaveBeenCalledWith([
      { repoId: 'repo-1', extensionId: 'dev.example.radio', name: 'Radio Stations' },
    ]);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('queues a selection in Rust, in order', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry(),
          entry({ id: 'dev.raygun.ia', name: 'Internet Archive' }),
          entry({ id: 'dev.raygun.mb', name: 'MusicBrainz' }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');
    await screen.findAllByTestId('catalog-entry');

    await userEvent.click(screen.getByRole('checkbox', { name: 'Select Radio Stations' }));
    const bar = screen.getByRole('toolbar', { name: 'Selected extensions' });
    expect(bar).toHaveTextContent('1 selected');
    await userEvent.click(within(bar).getByRole('button', { name: 'Select all 3' }));
    await userEvent.click(within(bar).getByRole('button', { name: 'Install 3' }));

    expect(commands.extensionInstallsEnqueue).toHaveBeenCalledTimes(1);
    expect(
      commands.extensionInstallsEnqueue.mock.calls[0]![0].map(
        (item: { extensionId: string }) => item.extensionId,
      ),
    ).toEqual(['dev.example.radio', 'dev.raygun.ia', 'dev.raygun.mb']);
    expect(screen.queryByRole('toolbar')).not.toBeInTheDocument();
  });

  it('shows the queue with its progress and stops the rest on Cancel', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(catalog([entry(), entry({ id: 'dev.raygun.ia', name: 'Internet Archive' })])),
    );
    // The queue was already running (the screen reloaded meanwhile).
    const running = installs({
      running: true,
      total: 12,
      done: 2,
      current: 'Radio Stations',
      phase: 'downloading',
      items: [
        install({ state: 'installing', phase: 'downloading' }),
        install({ extensionId: 'dev.raygun.ia', name: 'Internet Archive' }),
      ],
    });
    commands.extensionInstallsStatus.mockResolvedValue(running);
    commands.extensionInstallsCancel.mockResolvedValue(running);
    commands.extensionInstallsCancelAll.mockResolvedValue(running);
    await AppShellWrapper.mount('/extensions?tab=browse');

    const progress = await screen.findByTestId('install-progress');
    expect(progress).toHaveTextContent('Installing extensions · 3 of 12');
    expect(progress).toHaveTextContent('Radio Stations · Downloading');
    expect(within(progress).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '2');
    const [radio, archive] = screen.getAllByTestId('catalog-entry');
    expect(radio).toHaveTextContent('Installing');
    expect(archive).toHaveTextContent('Waiting');

    await userEvent.click(
      within(archive!).getByRole('button', { name: "Don't install Internet Archive" }),
    );
    expect(commands.extensionInstallsCancel).toHaveBeenCalledWith('repo-1', 'dev.raygun.ia');
    await userEvent.click(within(progress).getByRole('button', { name: 'Cancel' }));
    expect(commands.extensionInstallsCancelAll).toHaveBeenCalled();

    const ended = installs({
      total: 12,
      done: 11,
      failed: 1,
      items: [install({ state: 'failed', error: 'the repository is offline' })],
    });
    commands.extensionInstallsRetry.mockResolvedValue(ended);
    emitEvent('extensionInstallsEvent', ended);
    expect(await screen.findByTestId('install-progress')).toHaveTextContent(
      '11 installed · 1 failed',
    );
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(commands.extensionInstallsRetry).toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(commands.extensionInstallsDismiss).toHaveBeenCalledWith(null, null);
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
    commands.extensionInstallsEnqueue.mockResolvedValue(
      installs({
        total: 1,
        failed: 1,
        items: [
          install({
            state: 'failed',
            error: "package checksum doesn't match the repository index",
          }),
        ],
      }),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    const row = await screen.findByTestId('catalog-entry');
    expect(row).toHaveTextContent('1.0.0 → 1.1.0');
    await userEvent.click(within(row).getByRole('button', { name: 'Update' }));

    const issue = await within(row).findByRole('alert');
    expect(issue).toHaveTextContent("Didn't install");
    expect(issue).toHaveTextContent('checksum');
    expect(within(row).getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('marks Nuclear plugins and says their network access is detected at install', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry({
            id: 'nuclear.nuclear-plugin-lyrics',
            name: 'Lyrics Base',
            net: [],
            nuclear: true,
          }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    const row = await screen.findByTestId('catalog-entry');
    expect(row).toHaveTextContent('Nuclear');
    await userEvent.click(within(row).getByRole('button', { name: 'About Lyrics Base' }));
    expect(screen.getByRole('dialog')).toHaveTextContent('converts it when installing');
  });

  it('tags 18+ entries in the list and in their info, and no others', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry({ id: 'dev.test.adult', name: 'After Dark', contentRating: 'nsfw', adult: true }),
          entry({ id: 'dev.test.safe', name: 'Daylight', contentRating: 'safe' }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    const [adult, safe] = await screen.findAllByTestId('catalog-entry');
    expect(within(adult!).getByRole('img', { name: 'For adults (18+)' })).toHaveTextContent('18+');
    expect(within(safe!).queryByRole('img', { name: 'For adults (18+)' })).not.toBeInTheDocument();
    await userEvent.click(within(adult!).getByRole('button', { name: 'About After Dark' }));
    expect(
      within(screen.getByTestId('entry-info')).getByRole('img', { name: 'For adults (18+)' }),
    ).toBeInTheDocument();
  });

  it('says where site scripts run before they are installed', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry({
            id: 'dev.raygun.noutube',
            name: 'YouTube enhancements',
            provides: ['site.scripts'],
            net: [],
            sites: [
              'https://m.youtube.com/*',
              'https://www.youtube.com/*',
              'https://*.youtube.com/*',
            ],
          }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    const row = await screen.findByTestId('catalog-entry');
    await userEvent.click(within(row).getByRole('button', { name: 'About YouTube enhancements' }));
    const runsOn = within(screen.getByTestId('entry-info')).getByTestId('runs-on');
    expect(runsOn).toHaveTextContent('Runs on websites');
    // Each site once, as people know it.
    expect(
      within(runsOn)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['m.youtube.com', 'youtube.com']);
  });

  it('offers plugins converted by an older converter again', async () => {
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry({
            id: 'nuclear.nuclear-plugin-lyrics',
            name: 'Lyrics Base',
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
    await AppShellWrapper.mount('/extensions/detail?id=dev.example.radio');

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

  it('reloads every repository, or one from its heading, saying what failed', async () => {
    const user = userEvent.setup();
    commands.extensionCatalog.mockImplementation(async (refresh) =>
      refresh
        ? ok(catalog([], 'could not reach example.org'))
        : ok(catalog([entry({ state: 'installed' })])),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');
    await screen.findAllByTestId('catalog-entry');

    await user.click(screen.getByRole('button', { name: 'Reload all' }));
    expect(commands.extensionCatalog).toHaveBeenCalledWith(true);
    expect(await screen.findByText("1 repository couldn't be read")).toBeInTheDocument();
    expect(await screen.findByRole('alert')).toHaveTextContent('could not reach example.org');

    // One repository: its index alone is fetched again.
    commands.extensionCatalogRefreshRepo.mockResolvedValueOnce(
      ok(
        catalog([entry({ state: 'updateAvailable', version: '1.1.0', installedVersion: '1.0.0' })]),
      ),
    );
    await user.click(screen.getByRole('button', { name: 'Reload Raygun Extensions' }));
    expect(commands.extensionCatalogRefreshRepo).toHaveBeenCalledWith('repo-1');
    expect(await screen.findByText('1.0.0 → 1.1.0')).toBeInTheDocument();

    commands.extensionCatalogRefreshRepo.mockResolvedValueOnce({
      status: 'error',
      error: 'This repository is no longer added',
    });
    await user.click(screen.getByRole('button', { name: 'Reload Raygun Extensions' }));
    expect(await screen.findByText('This repository is no longer added')).toBeInTheDocument();
  });

  it("checks one extension for an update from its info, updating it when there's one", async () => {
    const user = userEvent.setup();
    commands.extensionCatalog.mockResolvedValue(ok(catalog([entry({ state: 'installed' })])));
    await AppShellWrapper.mount('/extensions?tab=browse');
    const [row] = await screen.findAllByTestId('catalog-entry');
    await user.click(within(row!).getByRole('button', { name: 'About Radio Stations' }));
    const info = screen.getByRole('dialog', { name: 'About this extension' });

    commands.extensionCatalogRefreshRepo.mockResolvedValueOnce(
      ok(catalog([entry({ state: 'installed' })])),
    );
    await user.click(within(info).getByRole('button', { name: 'Check for an update' }));
    expect(await within(info).findByText('Up to date')).toBeInTheDocument();
    expect(commands.extensionInstallsEnqueue).not.toHaveBeenCalled();

    commands.extensionCatalogRefreshRepo.mockResolvedValueOnce(
      ok(
        catalog([entry({ state: 'updateAvailable', version: '2.0.0', installedVersion: '1.0.0' })]),
      ),
    );
    await user.click(within(info).getByRole('button', { name: 'Check for an update' }));
    expect(await within(info).findByText('Updating to 2.0.0')).toBeInTheDocument();
    expect(commands.extensionInstallsEnqueue).toHaveBeenCalledWith([
      { repoId: 'repo-1', extensionId: 'dev.example.radio', name: 'Radio Stations' },
    ]);

    commands.extensionCatalogRefreshRepo.mockResolvedValueOnce(
      ok(catalog([], 'could not reach example.org')),
    );
    await user.click(within(info).getByRole('button', { name: 'Check for an update' }));
    expect(await within(info).findByRole('alert')).toHaveTextContent('could not reach example.org');
  });

  it('adds and removes repositories', async () => {
    commands.extensionRepos.mockResolvedValue(ok([REPO]));
    commands.sourcesDetect.mockResolvedValue({
      status: 'error',
      error: 'invalid repository URL: use https',
    });
    // Repositories live in Sources now.
    await AppShellWrapper.mount('/extensions?tab=repos');

    expect(await screen.findByTestId('repo-row')).toHaveTextContent(REPO.url);
    // Adding is the top bar's "+": Sources' one Add, which looks at the address first.
    await userEvent.click(screen.getByRole('button', { name: 'Add a source' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add a source' });
    await userEvent.type(
      within(sheet).getByRole('textbox', { name: 'Address' }),
      'http://example.org/index.json',
    );
    await userEvent.click(within(sheet).getByRole('button', { name: 'Look up' }));
    expect(commands.sourcesDetect).toHaveBeenCalledWith('http://example.org/index.json');
    expect(await within(sheet).findByRole('alert')).toHaveTextContent('use https');
    await userEvent.click(within(sheet).getByRole('button', { name: 'Close' }));

    // Removing takes a second tap; moving away cancels it.
    await userEvent.click(screen.getByRole('button', { name: 'Remove Raygun Extensions' }));
    expect(commands.extensionRepoRemove).not.toHaveBeenCalled();
    await userEvent.tab();
    await userEvent.click(screen.getByRole('button', { name: 'Remove Raygun Extensions' }));
    await userEvent.click(screen.getByRole('button', { name: 'Tap again to remove' }));
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
          entry({ id: 'dev.raygun.md', name: 'Comic Shelf', provides: ['manga.source'] }),
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
    expect(await names()).toEqual(['Radio Stations', 'Comic Shelf', 'NovelFull', 'Kakuyomu']);

    const kinds = screen.getByRole('group', { name: 'Show' });
    await user.click(within(kinds).getByRole('button', { name: 'Books' }));
    expect(await names()).toEqual(['NovelFull', 'Kakuyomu']);
    await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), 'ja');
    expect(await names()).toEqual(['Kakuyomu']);

    // A kind pressed again shows every kind.
    await user.click(within(kinds).getByRole('button', { name: 'Books' }));
    expect(within(kinds).getByRole('button', { name: 'Books' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
    await user.selectOptions(screen.getByRole('combobox', { name: 'Language' }), '');
    await user.selectOptions(screen.getByRole('combobox', { name: 'Repository' }), 'repo-1');
    expect(await names()).toEqual(['Radio Stations', 'Comic Shelf']);
    await user.type(screen.getByRole('searchbox', { name: 'Search extensions' }), 'comic');
    expect(await names()).toEqual(['Comic Shelf']);
    await user.type(screen.getByRole('searchbox', { name: 'Search extensions' }), 'zzz');
    expect(await screen.findByText('No extensions match.')).toBeInTheDocument();
  });

  it('lists All, Installed, Not installed and Updates, in that order', async () => {
    const user = userEvent.setup();
    commands.extensionsList.mockResolvedValue(ok([extension()]));
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry({ state: 'installed' }),
          entry({ id: 'dev.raygun.md', name: 'Comic Shelf', provides: ['manga.source'] }),
          entry({
            id: 'dev.raygun.ia',
            name: 'Internet Archive',
            state: 'updateAvailable',
            installedVersion: '1.0.0',
            version: '1.1.0',
          }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/browse?tab=extensions');
    const pills = await screen.findByRole('navigation', { name: 'Extension views' });
    await within(pills).findByRole('link', { name: 'Updates · 1' });
    expect(
      within(pills)
        .getAllByRole('link')
        .map((link) => link.textContent),
    ).toEqual(['All', 'Installed', 'Not installed', 'Updates · 1']);
    expect(within(pills).getByRole('link', { name: 'Installed' })).toHaveAttribute(
      'aria-current',
      'page',
    );

    await user.click(within(pills).getByRole('link', { name: 'Not installed' }));
    await vi.waitFor(() =>
      expect(screen.getAllByTestId('catalog-entry').map((row) => row.textContent)).toEqual([
        expect.stringContaining('Comic Shelf'),
      ]),
    );

    await user.click(within(pills).getByRole('link', { name: 'Updates · 1' }));
    await vi.waitFor(() => expect(screen.getAllByTestId('catalog-entry')).toHaveLength(1));
    expect(screen.getByTestId('catalog-entry')).toHaveTextContent('Internet Archive');
    expect(screen.getByRole('button', { name: 'Update all (1)' })).toBeInTheDocument();

    await user.click(within(pills).getByRole('link', { name: 'All' }));
    await vi.waitFor(() => expect(screen.getAllByTestId('catalog-entry')).toHaveLength(3));
  });

  it('finds installed extensions by name and kind', async () => {
    const user = userEvent.setup();
    commands.extensionsList.mockResolvedValue(
      ok(
        Array.from({ length: 8 }, (_, index) =>
          extension({
            id: `dev.example.${index}`,
            name: index === 3 ? 'Comic Shelf' : `Radio ${index}`,
            provides: index === 3 ? ['manga.source'] : ['music.metadata'],
          }),
        ),
      ),
    );
    await AppShellWrapper.mount('/browse?tab=extensions');
    await screen.findAllByTestId('extension-row');
    await user.type(
      screen.getByRole('searchbox', { name: 'Search installed extensions' }),
      'comic',
    );
    expect(screen.getAllByTestId('extension-row').map((row) => row.textContent)).toEqual([
      expect.stringContaining('Comic Shelf'),
    ]);
    await user.clear(screen.getByRole('searchbox', { name: 'Search installed extensions' }));
    const kinds = screen.getByRole('group', { name: 'Show' });
    await user.click(within(kinds).getByRole('button', { name: 'Audio' }));
    expect(screen.getAllByTestId('extension-row')).toHaveLength(7);
  });

  it('shows what each repository holds', async () => {
    commands.extensionRepos.mockResolvedValue(ok([REPO]));
    commands.extensionCatalog.mockResolvedValue(
      ok(
        catalog([
          entry(),
          entry({ id: 'dev.raygun.md', name: 'Comic Shelf', provides: ['manga.source'] }),
          entry({ id: 'dev.raygun.ck', name: 'Comick', provides: ['manga.source'] }),
          entry({ id: 'shosetsu.en.x', name: 'NovelFull', provides: ['novel.source'] }),
          entry({ id: 'miru.de.ccc.media', name: 'media.ccc.de', provides: ['video.source'] }),
        ]),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=repos');
    const row = await screen.findByTestId('repo-row');
    const contents = await within(row).findByRole('list', { name: 'What Raygun Extensions holds' });
    expect(
      within(contents)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual([
      'Music sources · 1',
      'Comics sources · 2',
      'Book sources · 1',
      'Video sources · 1',
      'Data sources · 1',
      'Shosetsu scripts · 1',
      'Miru extensions · 1',
    ]);
  });

  it("shows what a repository holds before it's added", async () => {
    const user = userEvent.setup();
    commands.sourcesDetect.mockResolvedValue(
      ok({
        kind: 'repository',
        format: 'shosetsu',
        server: null,
        url: 'https://example.org/s/index.json',
        name: 'Shosetsu',
        description: null,
        added: false,
        addon: null,
        entries: [
          entry({ id: 'shosetsu.en.a', name: 'A', provides: ['novel.source'] }),
          entry({ id: 'shosetsu.en.b', name: 'B', provides: ['novel.source'] }),
        ],
        note: null,
      }),
    );
    await AppShellWrapper.mount('/extensions?tab=repos');
    await user.click(await screen.findByRole('button', { name: 'Add a source' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add a source' });
    const field = within(sheet).getByRole('textbox', { name: 'Address' });
    await user.type(field, 'https://example.org/s/index.json');
    await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
    const preview = await within(sheet).findByRole('region', {
      name: 'Looks like a Shosetsu repository: Shosetsu',
    });
    expect(within(preview).getByRole('list', { name: 'What Shosetsu holds' })).toHaveTextContent(
      'Book sources · 2',
    );
    expect(commands.sourcesDetect).toHaveBeenCalledWith('https://example.org/s/index.json');
    expect(commands.sourcesDetect).toHaveBeenCalledTimes(1);

    // A long address goes with one tap.
    await user.click(within(sheet).getByRole('button', { name: 'Clear Address' }));
    expect(field).toHaveValue('');
    expect(field).toHaveFocus();
  });

  it('adds a suggested repository with one tap', async () => {
    const user = userEvent.setup();
    commands.extensionSuggestedRepos.mockResolvedValue(
      ok([
        { url: REPO.url, name: 'Paraline', description: 'Official.', added: true },
        {
          url: 'https://example.org/s/index.json',
          name: 'Novel sources',
          description: 'Novel sources.',
          added: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/extensions?tab=repos');

    expect(await screen.findByText(/^Nothing here yet/)).toBeInTheDocument();
    const official = await screen.findByRole('listitem', { name: 'Paraline' });
    expect(within(official).getByText('Added')).toBeInTheDocument();
    const shosetsu = screen.getByRole('listitem', { name: 'Novel sources' });
    await user.click(within(shosetsu).getByRole('button', { name: 'Add Novel sources' }));
    expect(commands.extensionRepoAdd).toHaveBeenCalledWith('https://example.org/s/index.json');
    expect(
      screen.getByText(/doesn’t review extensions from other repositories/),
    ).toBeInTheDocument();
  });
});
