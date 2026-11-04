import type { Website } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SITE: Website = {
  id: 'w1',
  name: 'Internet Archive',
  url: 'https://archive.org/',
  host: 'archive.org',
  icon: 'http://127.0.0.1:1/cover/ab',
  lastUrl: 'https://archive.org/details/x',
  blockAds: true,
};

describe('Websites', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('adds a site by its address and opens it in the built-in browser', async () => {
    const user = userEvent.setup();
    commands.websiteAdd.mockResolvedValue(ok({ ...SITE, lastUrl: null }));
    await AppShellWrapper.mount('/browse?tab=sources');
    const section = await screen.findByRole('region', { name: 'Websites' });
    expect(section).toHaveTextContent('No websites yet');

    await user.click(within(section).getByRole('button', { name: 'Add website' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add website' });
    await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), 'archive.org');
    commands.websiteList.mockResolvedValue(ok([SITE]));
    await user.click(within(sheet).getByRole('button', { name: 'Add' }));
    // No name typed: Rust names it after the host.
    expect(commands.websiteAdd).toHaveBeenCalledWith(null, 'archive.org');
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    const open = await within(section).findByRole('button', { name: 'Open Internet Archive' });
    expect(open).toHaveTextContent('archive.org');
    await user.click(open);
    expect(commands.websiteOpen).toHaveBeenCalledWith('w1');
  });

  it('edits, starts over and removes a site', async () => {
    const user = userEvent.setup();
    commands.websiteList.mockResolvedValue(ok([SITE]));
    await AppShellWrapper.mount('/browse?tab=sources');
    const section = await screen.findByRole('region', { name: 'Websites' });
    const menu = await within(section).findByRole('button', { name: 'Internet Archive options' });

    await user.click(menu);
    await user.click(screen.getByRole('menuitem', { name: 'Edit website' }));
    const sheet = await screen.findByRole('dialog', { name: 'Edit website' });
    const name = within(sheet).getByRole('textbox', { name: 'Name' });
    await user.clear(name);
    await user.type(name, 'Archive');
    await user.click(within(sheet).getByRole('button', { name: 'Save' }));
    expect(commands.websiteEdit).toHaveBeenCalledWith('w1', 'Archive', 'https://archive.org/');

    await user.click(menu);
    await user.click(screen.getByRole('menuitem', { name: 'Start from its home page' }));
    expect(commands.websiteReset).toHaveBeenCalledWith('w1');

    // Removing asks first.
    await user.click(menu);
    await user.click(screen.getByRole('menuitem', { name: 'Remove' }));
    const ask = await screen.findByRole('dialog', { name: 'Remove Internet Archive?' });
    expect(commands.websiteRemove).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole('button', { name: 'Remove' }));
    expect(commands.websiteRemove).toHaveBeenCalledWith('w1');
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it("switches a site's own ad blocker and says when it's off", async () => {
    const user = userEvent.setup();
    commands.websiteList.mockResolvedValue(ok([SITE]));
    await AppShellWrapper.mount('/browse?tab=sources');
    const section = await screen.findByRole('region', { name: 'Websites' });
    const menu = await within(section).findByRole('button', { name: 'Internet Archive options' });

    commands.websiteList.mockResolvedValue(ok([{ ...SITE, blockAds: false }]));
    await user.click(menu);
    await user.click(screen.getByRole('menuitem', { name: 'Allow ads on this site' }));
    expect(commands.websiteSetBlockAds).toHaveBeenCalledWith('w1', false);
    const open = await within(section).findByRole('button', { name: 'Open Internet Archive' });
    await vi.waitFor(() => expect(open).toHaveTextContent('archive.org · Ads allowed'));

    await user.click(menu);
    await user.click(screen.getByRole('menuitem', { name: 'Block ads on this site' }));
    expect(commands.websiteSetBlockAds).toHaveBeenCalledWith('w1', true);
  });

  it('says calmly which site did not open', async () => {
    const user = userEvent.setup();
    commands.websiteList.mockResolvedValue(ok([SITE]));
    commands.websiteOpen.mockResolvedValue({ status: 'error', error: 'WebView2 is missing' });
    await AppShellWrapper.mount('/browse?tab=sources');
    const section = await screen.findByRole('region', { name: 'Websites' });
    await user.click(await within(section).findByRole('button', { name: 'Open Internet Archive' }));
    const issue = await within(section).findByTestId('source-issue');
    expect(issue).toHaveTextContent("Couldn't open Internet Archive");
    expect(within(issue).getByText('WebView2 is missing')).not.toBeVisible();
  });

  it('shows why a site could not be added', async () => {
    const user = userEvent.setup();
    commands.websiteAdd.mockResolvedValue({
      status: 'error',
      error: 'Enter a web address, like example.com',
    });
    await AppShellWrapper.mount('/browse?tab=sources');
    const section = await screen.findByRole('region', { name: 'Websites' });
    await user.click(within(section).getByRole('button', { name: 'Add website' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add website' });
    await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), 'nope');
    await user.click(within(sheet).getByRole('button', { name: 'Add' }));
    expect(await within(sheet).findByRole('alert')).toHaveTextContent('Enter a web address');
  });

  it('sets the blocker in Settings → Browse', async () => {
    const user = userEvent.setup();
    commands.websiteBlocklistStatus.mockResolvedValue(
      ok({
        domains: 175760,
        updatedAt: Date.UTC(2026, 10, 1),
        source: 'https://example.org/list.txt',
        error: null,
      }),
    );
    await AppShellWrapper.mount('/settings/browse');
    const group = await screen.findByRole('region', { name: 'Websites' });
    expect(await within(group).findByText(/175,760 sites listed/)).toBeInTheDocument();
    await user.click(within(group).getByRole('switch', { name: 'Block popups and redirects' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'websites',
      value: { ...DEFAULT_SETTINGS.websites, blockPopups: false },
    });

    // A list from an address asks for it first.
    await user.click(within(group).getByRole('button', { name: /^Blocklist/ }));
    await user.click(screen.getByRole('radio', { name: /^A list from an address/ }));
    const sheet = await screen.findByRole('dialog', { name: 'A list from an address' });
    await user.type(
      within(sheet).getByRole('textbox', { name: 'List address' }),
      'https://example.org/hosts',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Save' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'websites',
      value: expect.objectContaining({
        blocklist: 'custom',
        customListUrl: 'https://example.org/hosts',
      }),
    });

    await user.click(within(group).getByRole('button', { name: 'Update blocklist now' }));
    expect(commands.websiteBlocklistUpdate).toHaveBeenCalled();
    await user.click(within(group).getByRole('button', { name: 'Clear website data' }));
    expect(await within(group).findByRole('status')).toHaveTextContent('were cleared');
  });
});
