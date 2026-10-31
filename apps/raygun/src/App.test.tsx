import { commands } from '@raygun/bindings';
import type { AppInfo } from '@raygun/bindings';
import { screen } from '@testing-library/react';

import { App } from './App';
import { renderWithProviders } from './test/renderWithProviders';

vi.mock('@raygun/bindings', () => ({
  commands: { appInfo: vi.fn() },
}));

const ANDROID_INFO: AppInfo = {
  name: 'Raygun',
  version: '0.1.0',
  platform: 'android',
  arch: 'x86_64',
  debug: true,
};

describe('App', () => {
  it('shows the app info reported by the core', async () => {
    vi.mocked(commands.appInfo).mockResolvedValue(ANDROID_INFO);

    renderWithProviders(<App />);

    expect(await screen.findByTestId('platform')).toHaveTextContent('android');
    expect(screen.getByTestId('app-info')).toHaveTextContent('0.1.0');
    expect(screen.getByTestId('app-info')).toHaveTextContent('debug');
  });

  it('shows an error when the core is unreachable', async () => {
    vi.mocked(commands.appInfo).mockRejectedValue(new Error('no ipc'));

    renderWithProviders(<App />);

    expect(await screen.findByRole('alert')).toHaveTextContent('no ipc');
  });
});
