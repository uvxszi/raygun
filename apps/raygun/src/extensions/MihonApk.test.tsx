import type { CatalogEntry, RepoCatalog } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

const MIHON: CatalogEntry = {
  repoId: 'repo-1',
  id: 'mihon.en.fixture',
  name: 'Fixture Comics',
  version: '1.6.16',
  description: 'Fixture Comics, a Mihon comic source.',
  author: 'Keiyoushi',
  provides: ['manga.source'],
  net: ['fixture.example'],
  contentRating: 'safe',
  adult: false,
  lang: 'en',
  size: 0,
  state: 'available',
  installedVersion: null,
  nuclear: false,
  icon: null,
  unsupported: 'Raygun can’t convert it: its code uses extractNextJs()',
};

const catalog = (entries: CatalogEntry[]): RepoCatalog[] => [
  {
    repo: { id: 'repo-1', url: 'https://example.org/index.min.json', name: 'Keiyoushi' },
    entries,
    error: null,
  },
];

describe('Allow running source code', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('turns on only after its notes are read', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/browse');

    const toggle = await screen.findByRole('switch', {
      name: 'Allow running source code (experimental)',
    });
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    await user.click(toggle);
    const notes = await screen.findByTestId('source-code-notes');
    expect(notes).toHaveTextContent('Running it isn’t available on this device yet');
    await user.click(within(notes).getByRole('button', { name: 'Allow' }));

    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'runSourceCode', value: true });
  });
});

describe('Mihon APKs', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.extensionCatalog.mockResolvedValue(ok(catalog([MIHON])));
  });

  it('offers no APK while running source code is off', async () => {
    await AppShellWrapper.mount('/extensions?tab=browse');

    expect(await screen.findByText(/its code uses extractNextJs/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Download APK' })).not.toBeInTheDocument();
  });

  it('downloads and checks the APK, then says running it is not available yet', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, runSourceCode: true }));
    commands.extensionMihonApk.mockResolvedValue(
      ok({
        package: 'eu.kanade.tachiyomi.extension.en.fixture',
        versionCode: 16,
        signer: 'ab'.repeat(32),
        size: 64_000,
        runnable: false,
        note: 'Saved and checked.',
      }),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    await user.click(await screen.findByRole('button', { name: 'Download APK' }));

    expect(commands.extensionMihonApk).toHaveBeenCalledWith('repo-1', 'mihon.en.fixture');
    expect(await screen.findByText(/Not available on this device yet/)).toHaveTextContent(
      'APK saved and checked against its store (63 KB). Not available on this device yet.',
    );
  });
});
