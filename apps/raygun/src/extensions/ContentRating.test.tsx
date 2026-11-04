import type { ExtensionInfo, Settings, StremioAddon } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const extension = (overrides: Partial<ExtensionInfo> = {}): ExtensionInfo => ({
  id: 'dev.test.comics',
  name: 'Comics Source',
  contentRating: 'safe',
  adult: false,
  version: '1.0.0',
  description: null,
  origin: 'installed',
  status: 'loaded',
  error: null,
  provides: ['manga.source'],
  net: [],
  providers: [],
  settings: [],
  compatibility: null,
  icon: null,
  website: null,
  ...overrides,
});

const REPO = { id: 'repo-1', url: 'https://example.org/index.json', name: 'Example Repo' };

const ADDON: StremioAddon = {
  id: 'addon-1',
  url: 'https://addon.example/manifest.json',
  name: 'Late Night',
  description: null,
  logo: null,
  version: '1.0.0',
  resources: ['catalog'],
  types: ['movie'],
  catalogs: 1,
  adult: true,
  error: null,
  enabled: true,
  provides: [],
  p2p: false,
  configureUrl: null,
  needsSetup: false,
  failing: false,
};

const withMarks = (contentOverrides: Settings['contentOverrides']) =>
  commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, contentOverrides }));

/** Opens an installed extension's right-click menu. */
const openMenu = async () =>
  userEvent.pointer({ keys: '[MouseRight]', target: await screen.findByTestId('extension-row') });

/** The last `contentOverrides` change sent to Rust. */
const lastMarks = () =>
  commands.settingsUpdate.mock.calls
    .map(([change]) => change as { key: string; value: Settings['contentOverrides'] })
    .filter((change) => change.key === 'contentOverrides')
    .at(-1)?.value;

describe('18+ marks', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('marks an installed extension 18+ from its menu, shows who set it, and goes back to its own rating', async () => {
    commands.extensionsList.mockResolvedValue(ok([extension()]));
    await AppShellWrapper.mount('/extensions');

    await openMenu();
    const menu = await screen.findByRole('menu');
    // Not rated 18+: only the mark that changes that.
    expect(within(menu).queryByRole('menuitem', { name: 'Not 18+' })).not.toBeInTheDocument();
    await userEvent.click(within(menu).getByRole('menuitem', { name: 'Mark as 18+' }));
    expect(lastMarks()?.extensions).toEqual({ 'dev.test.comics': 'adult' });
    expect(await screen.findByTestId('rating-hint')).toHaveTextContent('Set by you');

    await openMenu();
    await userEvent.click(await screen.findByRole('menuitem', { name: "Use the source's rating" }));
    expect(lastMarks()?.extensions).toEqual({});
  });

  it('offers Not 18+ for an extension rated 18+', async () => {
    commands.extensionsList.mockResolvedValue(
      ok([extension({ contentRating: 'nsfw', adult: true })]),
    );
    await AppShellWrapper.mount('/extensions');

    await openMenu();
    await screen.findByRole('menu');
    expect(screen.queryByRole('menuitem', { name: 'Mark as 18+' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('menuitem', { name: 'Not 18+' }));
    expect(lastMarks()?.extensions).toEqual({ 'dev.test.comics': 'notAdult' });
    expect(await screen.findByTestId('rating-hint')).toHaveTextContent('Not 18+ · set by you');
  });

  it('marks a whole repository either way', async () => {
    commands.extensionRepos.mockResolvedValue(ok([REPO]));
    await AppShellWrapper.mount('/extensions?tab=repos');

    await userEvent.click(await screen.findByRole('button', { name: 'More for Example Repo' }));
    expect(screen.getByRole('menuitem', { name: 'Mark as 18+' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('menuitem', { name: 'Not 18+' }));
    expect(lastMarks()?.repositories).toEqual({ 'repo-1': 'notAdult' });
    const row = screen.getByTestId('repo-row');
    expect(await within(row).findByTestId('rating-hint')).toHaveTextContent('Not 18+');
  });

  it('marks a Stremio addon from its row', async () => {
    commands.stremioAddons.mockResolvedValue(ok([ADDON]));
    await AppShellWrapper.mount('/watch/addons');

    await userEvent.click(await screen.findByRole('button', { name: 'More for Late Night' }));
    await userEvent.click(screen.getByRole('menuitem', { name: 'Not 18+' }));
    expect(lastMarks()?.addons).toEqual({ 'addon-1': 'notAdult' });
  });

  it('lists every mark in Settings and clears one', async () => {
    withMarks({
      extensions: { 'dev.test.comics': 'adult' },
      repositories: { 'repo-1': 'notAdult' },
      addons: {},
    });
    commands.extensionsList.mockResolvedValue(ok([extension()]));
    commands.extensionRepos.mockResolvedValue(ok([REPO]));
    await AppShellWrapper.mount('/settings/browse');

    const list = await screen.findByTestId('content-marks');
    const rows = within(list).getAllByRole('listitem');
    expect(rows.map((row) => row.textContent)).toEqual([
      'Comics SourceExtension · 18+',
      'Example RepoRepository · Not 18+',
    ]);
    await userEvent.click(
      within(list).getByRole('button', { name: "Use Example Repo's own rating" }),
    );
    expect(lastMarks()).toEqual({
      extensions: { 'dev.test.comics': 'adult' },
      repositories: {},
      addons: {},
    });
  });
});
