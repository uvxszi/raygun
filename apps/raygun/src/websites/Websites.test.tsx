import type { SettingChange, Settings, Website } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

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

  it('says which site scripts run on a site added', async () => {
    const user = userEvent.setup();
    const youtube = {
      ...SITE,
      id: 'w2',
      name: 'YouTube',
      url: 'https://m.youtube.com/',
      host: 'm.youtube.com',
      lastUrl: null,
      scripts: ['YouTube enhancements'],
    };
    commands.websiteAdd.mockResolvedValue(ok(youtube));
    await AppShellWrapper.mount('/browse?tab=sources');
    const section = await screen.findByRole('region', { name: 'Websites' });
    await user.click(within(section).getByRole('button', { name: 'Add website' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add website' });
    await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), 'm.youtube.com');
    commands.websiteList.mockResolvedValue(ok([youtube]));
    await user.click(within(sheet).getByRole('button', { name: 'Add' }));
    expect(await screen.findByText('YouTube enhancements runs on this site')).toBeInTheDocument();
    const open = await within(section).findByRole('button', { name: 'Open YouTube' });
    expect(open).toHaveTextContent('m.youtube.com · YouTube enhancements');
  });

  it('switches site scripts per site and background play in Settings → Browse', async () => {
    const user = userEvent.setup();
    commands.websiteSiteScripts.mockResolvedValue(
      ok([
        {
          extensionId: 'dev.raygun.noutube',
          name: 'YouTube enhancements',
          sites: ['https://m.youtube.com/*', 'https://music.youtube.com/*'],
          websites: [{ id: 'w2', name: 'YouTube', on: true }],
        },
      ]),
    );
    await AppShellWrapper.mount('/settings/browse');
    const scripts = await screen.findByRole('region', { name: 'Site scripts' });
    expect(
      await within(scripts).findByText('Runs on m.youtube.com, music.youtube.com'),
    ).toBeInTheDocument();
    const youtube = within(scripts).getByRole('switch', { name: 'YouTube' });
    expect(youtube).toHaveAttribute('aria-checked', 'true');
    await user.click(youtube);
    expect(commands.websiteSetSiteScripts).toHaveBeenCalledWith('w2', 'dev.raygun.noutube', false);

    const group = screen.getByRole('region', { name: 'Websites' });
    await user.click(within(group).getByRole('switch', { name: 'Keep playing in the background' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'websites',
      value: { ...DEFAULT_SETTINGS.websites, backgroundPlay: false },
    });
  });

  it('says when no site scripts are installed', async () => {
    await AppShellWrapper.mount('/settings/browse');
    const scripts = await screen.findByRole('region', { name: 'Site scripts' });
    expect(await within(scripts).findByText('No site scripts installed')).toBeInTheDocument();
    expect(
      within(scripts).getByRole('link', { name: /Find site scripts in Sources/ }),
    ).toHaveAttribute('href', '/sources');
  });

  it('lists saved sites in Library → Websites to open, edit and remove', async () => {
    const user = userEvent.setup();
    commands.websiteList.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/library?media=websites');
    const page = await screen.findByTestId('library-websites');
    expect(await within(page).findByText('No websites yet')).toBeInTheDocument();
    expect(screen.getByTestId('library-media-websites')).toHaveAttribute('aria-current', 'page');

    commands.websiteList.mockResolvedValue(ok([SITE]));
    commands.websiteAdd.mockResolvedValue(ok(SITE));
    await user.click(within(page).getByRole('button', { name: 'Add website' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add website' });
    await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), 'archive.org');
    await user.click(within(sheet).getByRole('button', { name: 'Add' }));
    expect(commands.websiteAdd).toHaveBeenCalledWith(null, 'archive.org');

    await user.click(await within(page).findByRole('button', { name: 'Open Internet Archive' }));
    expect(commands.websiteOpen).toHaveBeenCalledWith('w1');
    await user.click(within(page).getByRole('button', { name: 'Internet Archive options' }));
    await user.click(screen.getByRole('menuitem', { name: 'Remove' }));
    const ask = await screen.findByRole('dialog', { name: 'Remove Internet Archive?' });
    await user.click(within(ask).getByRole('button', { name: 'Remove' }));
    expect(commands.websiteRemove).toHaveBeenCalledWith('w1');
  });

  it('shows saved sites in Library → All, each opening the site', async () => {
    const user = userEvent.setup();
    commands.websiteList.mockResolvedValue(ok([SITE]));
    await AppShellWrapper.mount('/library?media=all');
    const row = await screen.findByRole('region', { name: 'Websites' });
    await user.click(within(row).getByRole('button', { name: 'Open Internet Archive' }));
    expect(commands.websiteOpen).toHaveBeenCalledWith('w1');
  });

  it('opens the site a quick access pin names', async () => {
    commands.websiteList.mockResolvedValue(ok([SITE]));
    await AppShellWrapper.mount('/library?media=websites&site=w1');
    await vi.waitFor(() => expect(commands.websiteOpen).toHaveBeenCalledWith('w1'));
    expect(commands.websiteOpen).toHaveBeenCalledTimes(1);
  });

  describe('on a computer', () => {
    afterEach(() => {
      delete (window as { matchMedia?: unknown }).matchMedia;
    });

    beforeEach(() => {
      window.matchMedia = vi.fn().mockImplementation((query: string) => ({
        matches: true,
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      }));
      commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    });

    it('pins a site to quick access, and the pin opens it', async () => {
      const user = userEvent.setup();
      let current: Settings = {
        ...DEFAULT_SETTINGS,
        rail: { ...DEFAULT_SETTINGS.rail, seeded: true },
      };
      commands.settingsGet.mockImplementation(async () => ok(current));
      commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
        current = { ...current, [change.key]: change.value };
        return ok(current);
      });
      commands.websiteList.mockResolvedValue(ok([SITE]));
      await AppShellWrapper.mount('/library?media=websites');
      const page = await screen.findByTestId('library-websites');
      await user.click(
        await within(page).findByRole('button', { name: 'Internet Archive options' }),
      );
      await user.click(screen.getByRole('menuitem', { name: 'Pin to Quick access' }));
      await vi.waitFor(() =>
        expect(current.rail.pins).toEqual([
          expect.objectContaining({
            kind: 'website',
            to: '/library',
            search: { media: 'websites', site: 'w1' },
          }),
        ]),
      );

      const pin = await within(screen.getByTestId('rail-pins')).findByRole('button', {
        name: /^Internet Archive/,
      });
      commands.websiteOpen.mockClear();
      await user.click(pin);
      expect(commands.websiteOpen).toHaveBeenCalledWith('w1');
    });
  });
});
