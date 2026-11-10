import type { CatalogEntry, RepoCatalog } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
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
    commands.mihonRuntimeState.mockResolvedValue(
      ok({ supported: false, ready: false, settingUp: false, note: '' }),
    );
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

  const RUNTIME_OFF = { supported: false, ready: false, settingUp: false, note: 'Not supported' };
  const RUNTIME_READY = { supported: true, ready: true, settingUp: false, note: '' };

  it('offers no APK while running source code is off', async () => {
    commands.mihonRuntimeState.mockResolvedValue(ok(RUNTIME_OFF));
    await AppShellWrapper.mount('/extensions?tab=browse');

    expect(await screen.findByText(/its code uses extractNextJs/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Download APK' })).not.toBeInTheDocument();
  });

  const savedApk = () =>
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

  it('downloads and checks the APK, then says running it is not available yet', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, runSourceCode: true }));
    commands.mihonRuntimeState.mockResolvedValue(ok(RUNTIME_OFF));
    savedApk();
    await AppShellWrapper.mount('/extensions?tab=browse');

    await user.click(await screen.findByRole('button', { name: 'Download APK' }));

    expect(commands.extensionMihonApk).toHaveBeenCalledWith('repo-1', 'mihon.en.fixture');
    expect(await screen.findByText(/isn.t available on this device yet/)).toBeInTheDocument();
  });

  it('runs the extension’s own code when the runtime is ready', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, runSourceCode: true }));
    commands.mihonRuntimeState.mockResolvedValue(ok(RUNTIME_READY));
    savedApk();
    commands.mihonRun.mockImplementation((_pkg: string, method: string) =>
      Promise.resolve(
        ok(
          method === 'sources'
            ? JSON.stringify([{ id: '42', name: 'Fixture', lang: 'en', supportsLatest: true }])
            : JSON.stringify({
                items: [
                  { url: '/a', title: 'One' },
                  { url: '/b', title: 'Two' },
                ],
                hasNextPage: false,
              }),
        ),
      ),
    );
    await AppShellWrapper.mount('/extensions?tab=browse');

    await user.click(await screen.findByRole('button', { name: 'Download APK' }));
    await user.click(await screen.findByRole('button', { name: 'Browse with its own code' }));

    expect(commands.mihonRun).toHaveBeenCalledWith(
      'eu.kanade.tachiyomi.extension.en.fixture',
      'sources',
      '{}',
    );
    expect(await screen.findByText('Its own code answered with 2 titles.')).toBeInTheDocument();
  });

  it('cancels setting up the runtime while its tools download', async () => {
    const user = userEvent.setup();
    const RUNTIME_MISSING = { supported: true, ready: false, settingUp: false, note: '' };
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, runSourceCode: true }));
    commands.mihonRuntimeState.mockResolvedValue(ok(RUNTIME_MISSING));
    commands.mihonRuntimeCancel.mockResolvedValue(ok(null));
    let finish: (value: { status: 'ok'; data: typeof RUNTIME_MISSING }) => void = () => {};
    commands.mihonRuntimeSetup.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    savedApk();
    await AppShellWrapper.mount('/extensions?tab=browse');

    await user.click(await screen.findByRole('button', { name: 'Download APK' }));
    expect(screen.queryByRole('button', { name: 'Cancel setting up the runtime' })).toBeNull();
    await user.click(await screen.findByRole('button', { name: 'Set up the runtime' }));
    await user.click(await screen.findByRole('button', { name: 'Cancel setting up the runtime' }));
    expect(commands.mihonRuntimeCancel).toHaveBeenCalled();

    // Rust answers once it stopped: the runtime is as it was, with nothing said to have failed.
    finish(ok(RUNTIME_MISSING));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Set up the runtime' })).toBeEnabled(),
    );
    expect(screen.queryByRole('button', { name: 'Cancel setting up the runtime' })).toBeNull();
    expect(screen.queryByText('Couldn’t set up the runtime')).toBeNull();
  });
});
