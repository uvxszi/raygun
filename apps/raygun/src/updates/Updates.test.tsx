import type { AppUpdate } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent, NO_UPDATE } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** Raygun 1.1.0 is out; this copy is the installed (setup exe) 1.0.0. */
const OUT: AppUpdate = {
  ...NO_UPDATE,
  latest: '1.1.0',
  available: true,
  notes: '## What is new\n- Faster starts',
  installKind: 'setup',
  assetName: 'Raygun_1.1.0_x64-setup.exe',
  assetSize: 9_000_000,
  checkedAt: 1,
};

describe('Updates (M46.47)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('checks for a release, then updates an installed copy from About', async () => {
    const user = userEvent.setup();
    commands.appUpdate.mockImplementation(async (refresh: boolean) =>
      ok(refresh ? OUT : NO_UPDATE),
    );
    commands.appUpdateInstall.mockResolvedValue(
      ok({ ...OUT, state: 'downloading', progress: 0.4 }),
    );
    await AppShellWrapper.mount('/about');

    const card = await screen.findByTestId('app-update');
    expect(within(card).getByTestId('update-status')).toHaveTextContent('This is Raygun 1.0.0.');
    await user.click(within(card).getByRole('button', { name: 'Check for updates' }));
    expect(commands.appUpdate).toHaveBeenCalledWith(true);
    expect(await within(card).findByTestId('update-status')).toHaveTextContent(
      'Raygun 1.1.0 is out (you have 1.0.0).',
    );
    await user.click(within(card).getByRole('button', { name: "What's new in 1.1.0" }));
    const notes = await screen.findByRole('dialog', { name: "What's new in 1.1.0" });
    expect(within(notes).getByRole('heading', { name: 'What is new' })).toBeInTheDocument();
    expect(within(notes).getByRole('listitem')).toHaveTextContent('Faster starts');
    await user.click(within(notes).getByRole('button', { name: 'Close' }));

    await user.click(within(card).getByRole('button', { name: 'Update to 1.1.0' }));
    expect(commands.appUpdateInstall).toHaveBeenCalled();
    expect(
      await within(card).findByRole('progressbar', { name: 'Downloading the update' }),
    ).toHaveAttribute('aria-valuenow', '40');

    await user.click(within(card).getByRole('button', { name: 'Raygun on GitHub' }));
    expect(commands.appOpenPage).toHaveBeenCalledWith('https://github.com/uvxszi/raygun');
  });

  it('sends a portable copy to the release page, and says so on Home', async () => {
    const user = userEvent.setup();
    const portable = { ...OUT, installKind: 'portable' as const, assetName: null };
    commands.appUpdate.mockResolvedValue(ok(portable));
    await AppShellWrapper.mount('/');

    const banner = await screen.findByTestId('update-banner');
    expect(banner).toHaveTextContent('Raygun 1.1.0 is out');
    await user.click(within(banner).getByRole('link'));

    const card = await screen.findByTestId('app-update');
    expect(within(card).getByText(/This is the portable Raygun/)).toBeInTheDocument();
    commands.appUpdateInstall.mockResolvedValue(ok(portable));
    await user.click(within(card).getByRole('button', { name: 'Get 1.1.0 from GitHub' }));
    expect(commands.appUpdateInstall).toHaveBeenCalled();
  });

  it('follows the updater as it reports, and hides the banner until the next release', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    expect(screen.queryByTestId('update-banner')).not.toBeInTheDocument();

    emitEvent('appUpdateEvent', OUT);
    const banner = await screen.findByTestId('update-banner');
    await user.click(within(banner).getByRole('button', { name: "What's new" }));
    const notes = await screen.findByRole('dialog', { name: "What's new in 1.1.0" });
    expect(notes).toHaveTextContent('Faster starts');
    await user.click(within(notes).getByRole('button', { name: 'Close' }));
    await user.click(within(banner).getByRole('button', { name: 'Hide until the next release' }));
    expect(screen.queryByTestId('update-banner')).not.toBeInTheDocument();
  });

  it("shows this version's notes once after an update, and again from About", async () => {
    const user = userEvent.setup();
    const notes = { version: '1.1.0', notes: '## Playback\n- **Gapless** songs' };
    commands.appWhatsNew.mockResolvedValue(ok(notes));
    commands.appReleaseNotes.mockResolvedValue([
      notes,
      { version: '1.0.0', notes: 'The first one' },
    ]);
    await AppShellWrapper.mount('/about');

    const sheet = await screen.findByRole('dialog', { name: "What's new in 1.1.0" });
    expect(within(sheet).getByText('Gapless').tagName).toBe('STRONG');
    await user.click(within(sheet).getByRole('button', { name: 'Close' }));
    expect(commands.appWhatsNewSeen).toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    const card = await screen.findByTestId('app-update');
    await user.click(await within(card).findByRole('button', { name: /^What's new/ }));
    const again = await screen.findByRole('dialog', { name: "What's new in 1.1.0" });
    await user.click(within(again).getByRole('button', { name: '1.0.0' }));
    expect(screen.getByRole('dialog', { name: "What's new in 1.0.0" })).toHaveTextContent(
      'The first one',
    );
  });
});
