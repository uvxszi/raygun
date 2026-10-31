import { commands } from '@raygun/bindings';
import type { AppInfo, StorageInfo } from '@raygun/bindings';
import { screen } from '@testing-library/react';

import { App } from './App';
import { renderWithProviders } from './test/renderWithProviders';

vi.mock('@raygun/bindings', () => ({
  commands: { appInfo: vi.fn(), storageInfo: vi.fn() },
}));

const ANDROID_INFO: AppInfo = {
  name: 'Raygun',
  version: '0.1.0',
  platform: 'android',
  arch: 'x86_64',
  debug: true,
};

const MOBILE_STORAGE: StorageInfo = {
  mode: 'mobile',
  root: '/data/user/0/app.raygun.player',
  logs: '/data/user/0/app.raygun.player/logs',
  webview: '/data/user/0/app.raygun.player/webview',
};

describe('App', () => {
  beforeEach(() => {
    vi.mocked(commands.appInfo).mockResolvedValue(ANDROID_INFO);
    vi.mocked(commands.storageInfo).mockResolvedValue(MOBILE_STORAGE);
  });

  it('shows the app info reported by the core', async () => {
    renderWithProviders(<App />);

    expect(await screen.findByTestId('platform')).toHaveTextContent('android');
    expect(screen.getByTestId('app-info')).toHaveTextContent('0.1.0');
    expect(screen.getByTestId('app-info')).toHaveTextContent('debug');
  });

  it('shows where data is stored', async () => {
    renderWithProviders(<App />);

    expect(await screen.findByTestId('storage-mode')).toHaveTextContent('App sandbox');
    expect(screen.getByTestId('storage-root')).toHaveTextContent('/data/user/0/app.raygun.player');
  });

  it('labels portable storage', async () => {
    vi.mocked(commands.storageInfo).mockResolvedValue({ ...MOBILE_STORAGE, mode: 'portable' });

    renderWithProviders(<App />);

    expect(await screen.findByTestId('storage-mode')).toHaveTextContent('Portable');
  });

  it('shows an error when the core is unreachable', async () => {
    vi.mocked(commands.appInfo).mockRejectedValue(new Error('no ipc'));

    renderWithProviders(<App />);

    expect(await screen.findByRole('alert')).toHaveTextContent('no ipc');
  });
});
