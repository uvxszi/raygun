import type {
  CatalogEntry,
  DetectedSource,
  ExtensionInfo,
  ExtensionRepo,
  ServerInfo,
  StremioAddon,
} from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const extension = (
  id: string,
  name: string,
  kind: string,
  overrides: Partial<ExtensionInfo> = {},
): ExtensionInfo => ({
  id,
  name,
  contentRating: 'safe',
  version: '1.0.0',
  description: null,
  origin: 'installed',
  status: 'loaded',
  error: null,
  provides: [kind],
  net: [],
  providers: [{ id: `${id}/main`, extensionId: id, kind, name, adult: false }],
  settings: [],
  compatibility: null,
  icon: null,
  website: null,
  repository: null,
  ...overrides,
});

const entry = (id: string, provides: string[]): CatalogEntry => ({
  repoId: 'repo-1',
  id,
  name: id,
  version: '1.0.0',
  description: null,
  author: null,
  provides,
  net: [],
  contentRating: 'safe',
  adult: false,
  lang: null,
  size: 0,
  state: 'available',
  installedVersion: null,
  nuclear: false,
  icon: null,
});

const REPO: ExtensionRepo = {
  id: 'repo-1',
  url: 'https://example.org/index.json',
  name: 'Example sources',
};

const ADDON: StremioAddon = {
  id: 'cinemeta',
  url: 'https://v3-cinemeta.strem.io/manifest.json',
  name: 'Cinemeta',
  description: 'Movies and series',
  logo: null,
  version: '3.0.0',
  resources: ['catalog', 'meta'],
  types: ['movie', 'series'],
  catalogs: 2,
  adult: false,
  error: null,
  enabled: true,
  provides: [{ resource: 'catalog', kind: 'movie', count: 2 }],
  p2p: false,
  configureUrl: null,
  needsSetup: false,
  failing: false,
};

const SERVER: ServerInfo = {
  id: 'server.subsonic.0a1b2c3d4e5f',
  kind: 'subsonic',
  name: 'Home Music',
  address: 'https://music.example.org',
  username: 'me',
  provides: ['music.metadata', 'music.stream', 'lyrics'],
  error: null,
};

const detected = (overrides: Partial<DetectedSource>): DetectedSource => ({
  kind: 'addon',
  format: null,
  server: null,
  url: '',
  name: '',
  description: null,
  added: false,
  addon: null,
  entries: [],
  note: null,
  ...overrides,
});

const seedSources = () => {
  commands.extensionRepos.mockResolvedValue(ok([REPO]));
  commands.extensionCatalog.mockResolvedValue(
    ok([{ repo: REPO, entries: [entry('dev.example.shelf', ['manga.source'])], error: null }]),
  );
  commands.stremioAddons.mockResolvedValue(ok([ADDON]));
  commands.serversList.mockResolvedValue(ok([SERVER]));
  commands.extensionsList.mockResolvedValue(
    ok([
      extension('dev.example.radio', 'Radio Stations', 'music.metadata'),
      extension('dev.example.comics', 'Comic Shelf', 'manga.source'),
      // The server's own extension is listed as the server.
      extension(SERVER.id, SERVER.name, 'music.metadata'),
    ]),
  );
};

/** The page's sections by name (suggestions inside them are named by their headings). */
const sections = (page: HTMLElement) =>
  within(page)
    .queryAllByRole('region')
    .map((region) => region.getAttribute('aria-label'))
    .filter(Boolean);

const openAddSheet = async (user: ReturnType<typeof userEvent.setup>) => {
  await AppShellWrapper.mount('/sources');
  const page = await screen.findByTestId('sources-page');
  await user.click(within(page).getByRole('button', { name: 'Add a source' }));
  return screen.findByRole('dialog', { name: 'Add a source' });
};

const lookUp = async (
  user: ReturnType<typeof userEvent.setup>,
  sheet: HTMLElement,
  address: string,
) => {
  await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), address);
  await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
};

describe('Sources', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    seedSources();
  });

  it('lists repositories, addons, servers and extensions, narrowed by kind and by what they are for', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/sources');
    const page = await screen.findByTestId('sources-page');
    await within(page).findByRole('listitem', { name: 'Home Music' });
    expect(sections(page)).toEqual(['Repositories', 'Addons', 'Servers', 'Extensions']);
    const extensions = within(page).getByRole('region', { name: 'Extensions' });
    expect(within(extensions).queryByText('Home Music')).not.toBeInTheDocument();

    const kinds = within(page).getByRole('navigation', { name: 'Kinds of sources' });
    await user.click(within(kinds).getByRole('link', { name: 'Addons · 1' }));
    expect(within(kinds).getByRole('link', { name: 'Addons · 1' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(sections(page)).toEqual(['Addons']);

    // Audio: music sources and the servers that bring music, whatever their kind.
    await user.click(within(kinds).getByRole('link', { name: 'All' }));
    const media = within(page).getByRole('group', { name: 'What sources are for' });
    await user.click(within(media).getByRole('button', { name: 'Audio' }));
    expect(sections(page)).toEqual(['Servers', 'Extensions']);
    expect(within(page).getByRole('listitem', { name: 'Radio Stations' })).toBeInTheDocument();
    expect(within(page).queryByRole('listitem', { name: 'Comic Shelf' })).not.toBeInTheDocument();

    await user.click(within(media).getByRole('button', { name: 'Comics' }));
    expect(sections(page)).toEqual(['Repositories', 'Extensions']);
    expect(within(page).getByRole('listitem', { name: 'Comic Shelf' })).toBeInTheDocument();
  });

  it('suggests Paraline while no repository is added', async () => {
    const user = userEvent.setup();
    commands.extensionRepos.mockResolvedValue(ok([]));
    commands.extensionCatalog.mockResolvedValue(ok([]));
    commands.extensionsList.mockResolvedValue(ok([]));
    commands.serversList.mockResolvedValue(ok([]));
    commands.extensionSuggestedRepos.mockResolvedValue(
      ok([
        {
          url: 'https://example.org/paraline/index.json',
          name: 'Paraline',
          description: 'Public-domain and openly licensed sources.',
          added: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/sources');
    const page = await screen.findByTestId('sources-page');
    const repos = await within(page).findByRole('region', { name: 'Repositories' });
    const paraline = await within(repos).findByRole('listitem', { name: 'Paraline' });
    await user.click(within(paraline).getByRole('button', { name: 'Add Paraline' }));
    expect(commands.extensionRepoAdd).toHaveBeenCalledWith(
      'https://example.org/paraline/index.json',
    );
  });

  it('turns addons, servers and extensions off together', async () => {
    const user = userEvent.setup();
    commands.settingsUpdate.mockClear();
    await AppShellWrapper.mount('/sources');
    const page = await screen.findByTestId('sources-page');
    await within(page).findByRole('listitem', { name: 'Home Music' });

    await user.click(within(page).getByRole('button', { name: 'Select' }));
    const bar = screen.getByRole('toolbar', { name: 'Selected' });
    await user.click(within(bar).getByRole('checkbox', { name: 'Select all' }));
    expect(bar).toHaveTextContent('4 selected');
    await user.click(within(bar).getByRole('button', { name: 'Turn off' }));
    expect(commands.stremioSetEnabled).toHaveBeenCalledWith('cinemeta', false);
    const last = commands.settingsUpdate.mock.lastCall?.[0] as { value: string[] };
    expect([...last.value].sort()).toEqual(['dev.example.comics', 'dev.example.radio', SERVER.id]);
  });

  it('looks at an address before adding it, then adds what it is', async () => {
    const user = userEvent.setup();
    const addon = detected({
      kind: 'addon',
      url: ADDON.url,
      name: 'Cinemeta',
      description: 'Movies and series',
      addon: { ...ADDON, id: '' },
    });
    commands.sourcesDetect.mockResolvedValue(ok(addon));
    commands.sourcesAdd.mockResolvedValue(ok({ ...addon, added: true }));
    const sheet = await openAddSheet(user);
    await lookUp(user, sheet, 'v3-cinemeta.strem.io');
    expect(commands.sourcesDetect).toHaveBeenCalledWith('v3-cinemeta.strem.io');

    const card = await within(sheet).findByRole('region', {
      name: 'Looks like a Stremio addon: Cinemeta',
    });
    expect(within(card).getByText('Movie catalogs · 2')).toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'Add' }));
    expect(commands.sourcesAdd).toHaveBeenCalledWith('v3-cinemeta.strem.io');
    await vi.waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Add a source' })).not.toBeInTheDocument(),
    );
  });

  it('shows what a repository holds, and why its extensions may not run here', async () => {
    const user = userEvent.setup();
    commands.sourcesDetect.mockResolvedValue(
      ok(
        detected({
          kind: 'repository',
          format: 'mihon',
          url: 'https://raw.githubusercontent.com/keiyoushi/extensions/repo/index.min.json',
          name: 'keiyoushi',
          entries: [entry('mihon.en.example', ['manga.source'])],
          note: "Mihon extensions aren't supported on this device yet",
        }),
      ),
    );
    const sheet = await openAddSheet(user);
    await lookUp(user, sheet, 'https://github.com/keiyoushi/extensions');
    const card = await within(sheet).findByRole('region', {
      name: 'Looks like a Mihon repository: keiyoushi',
    });
    expect(
      within(within(card).getByRole('list', { name: 'What keiyoushi holds' }))
        .getAllByRole('listitem')
        .map((chip) => chip.textContent),
    ).toEqual(['Comics sources · 1', 'Mihon extensions · 1']);
    expect(card).toHaveTextContent("Mihon extensions aren't supported on this device yet");
    // Typing again asks again: what was shown was about the old address.
    await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), '/');
    expect(within(sheet).queryByRole('region')).not.toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: 'Look up' })).toBeEnabled();
  });

  it('signs in to a server it found', async () => {
    const user = userEvent.setup();
    commands.sourcesDetect.mockResolvedValue(
      ok(
        detected({
          kind: 'server',
          server: 'jellyfin',
          url: 'https://media.example.org',
          name: 'Home Media',
        }),
      ),
    );
    const sheet = await openAddSheet(user);
    await lookUp(user, sheet, 'media.example.org');
    const card = await within(sheet).findByRole('region', {
      name: 'Looks like a Jellyfin server: Home Media',
    });
    await user.type(within(card).getByRole('textbox', { name: 'User name' }), 'viewer');
    await user.type(within(card).getByLabelText('Password'), 'secret');
    await user.click(within(card).getByRole('button', { name: 'Connect' }));
    expect(commands.serversAdd).toHaveBeenCalledWith({
      kind: 'jellyfin',
      address: 'https://media.example.org',
      username: 'viewer',
      secret: 'secret',
      apiKey: false,
    });
    expect(commands.sourcesAdd).not.toHaveBeenCalled();
  });

  it('says when nothing it can add is there, and connects a server by hand', async () => {
    const user = userEvent.setup();
    commands.sourcesDetect.mockResolvedValue({
      status: 'error',
      error: 'Raygun found no repository, addon, server or extension at blog.example',
    });
    const sheet = await openAddSheet(user);
    await lookUp(user, sheet, 'blog.example');
    expect(await within(sheet).findByRole('alert')).toHaveTextContent(
      'Raygun found no repository, addon, server or extension at blog.example',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Connect a server by hand' }));
    expect(await screen.findByRole('dialog', { name: 'Add server' })).toBeInTheDocument();
  });

  it('looks at a picked folder at once', async () => {
    const user = userEvent.setup();
    commands.filesPick.mockResolvedValue(ok({ path: 'C:\\repos\\mine', name: 'mine' }));
    commands.sourcesDetect.mockResolvedValue(
      ok(
        detected({
          kind: 'repository',
          format: 'raygun',
          url: 'file:///C:/repos/mine/index.json',
          name: 'mine',
        }),
      ),
    );
    const sheet = await openAddSheet(user);
    await user.click(within(sheet).getByRole('button', { name: 'Choose a file or folder' }));
    await user.click(screen.getByRole('menuitem', { name: 'Choose folder' }));
    expect(commands.filesPick).toHaveBeenCalledWith('folder');
    expect(within(sheet).getByRole('textbox', { name: 'Address' })).toHaveValue('C:\\repos\\mine');
    expect(commands.sourcesDetect).toHaveBeenCalledWith('C:\\repos\\mine');
    expect(
      await within(sheet).findByRole('region', { name: 'Looks like a Raygun repository: mine' }),
    ).toBeInTheDocument();
  });

  it('gives Browse → Sources an Audio pill: music, podcasts, audiobooks and audio servers', async () => {
    const user = userEvent.setup();
    commands.settingsUpdate.mockClear();
    commands.extensionsList.mockResolvedValue(
      ok([
        extension('dev.example.radio', 'Radio Stations', 'music.metadata'),
        extension('dev.example.books', 'Free Audiobooks', 'audiobook.directory'),
        extension('dev.example.casts', 'Podcast Index', 'podcast.directory'),
        extension('dev.example.comics', 'Comic Shelf', 'manga.source'),
        extension(SERVER.id, SERVER.name, 'music.metadata', {
          providers: [
            {
              id: `${SERVER.id}/music`,
              extensionId: SERVER.id,
              kind: 'music.metadata',
              name: 'Music',
              adult: false,
            },
            {
              id: `${SERVER.id}/lyrics`,
              extensionId: SERVER.id,
              kind: 'lyrics',
              name: 'Lyrics',
              adult: false,
            },
          ],
        }),
      ]),
    );
    await AppShellWrapper.mount('/browse?tab=sources');
    const kinds = await screen.findByRole('group', { name: 'Kinds of sources' });
    await user.click(await within(kinds).findByRole('button', { name: 'Audio · 4' }));
    const audio = await screen.findByRole('region', { name: 'Audio' });
    expect(
      within(audio)
        .getAllByRole('link')
        .map((link) => link.getAttribute('aria-label')),
    ).toEqual(['Radio Stations', 'Free Audiobooks', 'Podcast Index', SERVER.name]);
    const detail = (name: string) => within(audio).getByRole('link', { name });
    expect(detail('Radio Stations')).toHaveTextContent(/Music$/);
    expect(detail('Free Audiobooks')).toHaveTextContent(/Audiobooks$/);
    expect(detail('Podcast Index')).toHaveTextContent(/Podcasts$/);
    // A server brings its music too.
    expect(detail(SERVER.name)).toHaveTextContent(/Music · Lyrics$/);
    expect(screen.queryByRole('region', { name: 'Comics' })).not.toBeInTheDocument();

    // A row's switch turns all of its audio sources off.
    await user.click(within(audio).getByRole('switch', { name: `Use ${SERVER.name}` }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'disabledSources',
      value: [`${SERVER.id}/music`, `${SERVER.id}/lyrics`],
    });
  });

  it('is where the addons page used to be', async () => {
    await AppShellWrapper.mount('/watch/addons');
    const page = await screen.findByTestId('sources-page');
    expect(await within(page).findByRole('link', { name: 'Addons · 1' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  it('is where the repositories page used to be', async () => {
    await AppShellWrapper.mount('/extensions/repos');
    const page = await screen.findByTestId('sources-page');
    expect(await within(page).findByRole('link', { name: 'Repositories · 1' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });
});
