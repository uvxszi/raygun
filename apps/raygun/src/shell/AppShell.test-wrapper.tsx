import { commands } from '@raygun/bindings';
import type { AppInfo, Settings, StorageInfo } from '@raygun/bindings';
import { RouterProvider, createMemoryHistory } from '@tanstack/react-router';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { usePlayerUiStore } from '../player/playerUiStore';
import { createAppRouter } from '../router';
import { renderWithProviders } from '../test/renderWithProviders';

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

const DEFAULT_SETTINGS: Settings = { theme: 'system', language: 'en' };

export const AppShellWrapper = {
  seedCore() {
    vi.mocked(commands.appInfo).mockResolvedValue(ANDROID_INFO);
    vi.mocked(commands.storageInfo).mockResolvedValue(MOBILE_STORAGE);
    vi.mocked(commands.settingsGet).mockResolvedValue({ status: 'ok', data: DEFAULT_SETTINGS });
    usePlayerUiStore.setState({ sheetOpen: false });
  },

  async mount(path = '/') {
    const router = createAppRouter(createMemoryHistory({ initialEntries: [path] }));
    renderWithProviders(<RouterProvider router={router} />);
    await screen.findByTestId('page-content');
  },

  page(name: 'home' | 'library' | 'search' | 'settings') {
    return screen.findByTestId(`${name}-page`);
  },

  async tapTab(name: 'home' | 'library' | 'search' | 'settings') {
    await userEvent.click(screen.getByTestId(`nav-${name}-tab`));
  },

  get activeTab() {
    return within(screen.getByTestId('bottom-tabs')).getByRole('link', { current: 'page' });
  },

  miniPlayer: {
    async open() {
      await userEvent.click(screen.getByRole('button', { name: 'Open player' }));
    },
  },

  playerSheet: {
    get element() {
      return screen.findByRole('dialog', { name: 'Now playing' });
    },
    async close() {
      await userEvent.click(screen.getByRole('button', { name: 'Close player' }));
    },
    async waitUntilClosed() {
      await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    },
  },
};
