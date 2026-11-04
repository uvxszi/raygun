import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from './AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

const CHECK =
  'Royal Road wants a browser check, which Raygun can’t do\nwww.royalroad.com answered HTTP 403 with a browser check';
const TARGET = {
  extensionId: 'dev.raygun.royalroad',
  source: 'Royal Road',
  url: 'https://www.royalroad.com/',
};

describe('Browser checks passed by hand', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.musicChart.mockResolvedValue({ status: 'error', error: CHECK });
  });

  it('offers nothing while the setting is off', async () => {
    commands.browserCheckTarget.mockResolvedValue(ok(TARGET));
    await AppShellWrapper.mount('/music/chart?provider=x&id=top');

    await screen.findByTestId('source-issue');
    expect(
      screen.queryByRole('button', { name: 'Open the site to pass the check' }),
    ).not.toBeInTheDocument();
    expect(commands.browserCheckTarget).not.toHaveBeenCalled();
  });

  it('opens the site when on, then asks the failed source again', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, manualBrowserChecks: true }));
    commands.browserCheckTarget.mockResolvedValue(ok(TARGET));
    commands.browserCheckOpen.mockResolvedValue(ok(2));
    await AppShellWrapper.mount('/music/chart?provider=x&id=top');

    const issue = await screen.findByTestId('source-issue');
    const open = await within(issue).findByRole('button', {
      name: 'Open the site to pass the check',
    });
    expect(commands.browserCheckTarget).toHaveBeenCalledWith(CHECK, null);
    const asked = commands.musicChart.mock.calls.length;
    commands.musicChart.mockResolvedValue(ok([]));
    await user.click(open);

    expect(commands.browserCheckOpen).toHaveBeenCalledWith('dev.raygun.royalroad');
    await waitFor(() => expect(commands.musicChart.mock.calls.length).toBeGreaterThan(asked));
  });

  it('says so when the browser closed without the check passed', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(ok({ ...DEFAULT_SETTINGS, manualBrowserChecks: true }));
    commands.browserCheckTarget.mockResolvedValue(ok(TARGET));
    commands.browserCheckOpen.mockResolvedValue(ok(0));
    await AppShellWrapper.mount('/music/chart?provider=x&id=top');

    await user.click(
      await screen.findByRole('button', { name: 'Open the site to pass the check' }),
    );
    expect(
      await screen.findByText('The check wasn’t passed — no cookies came back'),
    ).toBeInTheDocument();
  });

  it('turns on only after the warnings are confirmed', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/browse');

    const toggle = await screen.findByRole('switch', {
      name: 'Solve browser checks myself (experimental)',
    });
    expect(toggle).toHaveAttribute('aria-checked', 'false');

    await user.click(toggle);
    const warning = await screen.findByTestId('browser-checks-warning');
    for (const text of [
      'Sites may see this as automated use and block or ban your IP address or account',
      'It may break those sites’ terms of use',
      'It’s experimental and may not work',
      'Raygun is a tool: you’re responsible for how you use it',
    ]) {
      expect(within(warning).getByText(text)).toBeInTheDocument();
    }
    await user.click(within(warning).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByTestId('browser-checks-warning')).not.toBeInTheDocument();
    expect(commands.settingsUpdate).not.toHaveBeenCalledWith({
      key: 'manualBrowserChecks',
      value: true,
    });
    expect(toggle).toHaveAttribute('aria-checked', 'false');

    await user.click(toggle);
    await user.click(
      within(await screen.findByTestId('browser-checks-warning')).getByRole('button', {
        name: 'Turn on anyway',
      }),
    );
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'manualBrowserChecks',
      value: true,
    });
    await waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'true'));

    // Turning it off asks nothing.
    await user.click(toggle);
    expect(screen.queryByTestId('browser-checks-warning')).not.toBeInTheDocument();
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'manualBrowserChecks',
      value: false,
    });
  });
});
